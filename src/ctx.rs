use std::fs;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};

use prelude::*;
use config::{Config, RuntimeConfig, RemoteToolInclude};
use tools::Tool;
use utils::cmd::CommandBuilder;
use report::Report;
use formatting::FormatResult;
use rt;
use rt::common::Runtime;

use console::{style, Term, user_attended};
use parking_lot::Mutex;
use walkdir::WalkDir;
use indicatif::ProgressBar;
use which::which_in;

#[derive(Debug)]
struct Log {
    lines: usize,
}

#[derive(Debug)]
pub struct Context {
    base_dir: PathBuf,
    config: Config,
    log: Mutex<Log>,
}

fn update_remote_tool(path: &Path, rti: &RemoteToolInclude) -> Result<()> {
    match *rti {
        RemoteToolInclude::Git { ref git, ref rev, .. } => {
            let mut cmd;
            if fs::metadata(&path).is_err() {
                fs::create_dir_all(&path)?;
                cmd = CommandBuilder::new("git");
                cmd
                    .arg("clone")
                    .arg(git)
                    .arg(".")
                    .current_dir(&path);

                if let &Some(ref rev) = rev {
                    cmd.arg("-b").arg(rev);
                }
            } else if rev.is_none() {
                fs::create_dir_all(&path)?;
                cmd = CommandBuilder::new("git");
                cmd
                    .arg("pull");
            } else {
                return Ok(());
            }

            cmd
                .spawn()?
                .wait()?;
        }
        RemoteToolInclude::Path { .. } => {}
    }
    Ok(())
}

impl Context {
    pub fn new(config: Config) -> Result<Context> {
        Ok(Context {
            base_dir: config.config_dir().parent().unwrap().to_path_buf(),
            config: config,
            log: Mutex::new(Log {
                lines: 0,
            }),
        })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.config.cache_dir()
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn log_step(&self, text: &str) {
        let mut log = self.log.lock();
        write!(&mut ::std::io::stderr(), "{} {}\n",
               style(">").dim().bold(),
               text).unwrap();
        log.lines += 1;
    }

    pub fn clear_log(&self) {
        if user_attended() {
            let mut log = self.log.lock();
            Term::stderr().clear_last_lines(log.lines).unwrap();
            log.lines = 0;
        }
    }

    pub fn create_tool<'a>(&'a self, id: &str) -> Result<Tool<'a>> {
        if let Some(spec) = self.config.get_tool_spec(id) {
            Tool::new(self, id, spec)
        } else {
            Err(Error::from(format!("Could not find tool '{}'", id)))
        }
    }

    pub fn create_runtime<'a>(&'a self, id: &str, cfg: &'a RuntimeConfig)
        -> Result<Box<Runtime<'a> + 'a>>
    {
        match id {
            "python" => Ok(Box::new(rt::python::PythonRuntime::create(self, cfg))),
            "javascript" => Ok(Box::new(rt::js::JsRuntime::create(self, cfg))),
            _ => Err(Error::from(format!("Could not find runtime '{}'", id)))
        }
    }

    pub fn clear_cache(&self) -> Result<()> {
        let wd = WalkDir::new(self.cache_dir());
        let count = wd.into_iter().count();
        let pb = ProgressBar::new(count as u64);

        for entry in WalkDir::new(self.cache_dir()) {
            if_chain! {
                if let Ok(entry) = entry;
                if let Ok(md) = entry.metadata();
                if !md.is_dir();
                then {
                    fs::remove_file(entry.path())?;
                }
            }
            pb.inc(1);
        }

        if fs::metadata(self.cache_dir()).is_ok() {
            fs::remove_dir_all(self.cache_dir())?;
        }

        pb.finish_and_clear();

        Ok(())
    }

    pub fn pull_dependencies(&mut self) -> Result<()> {
        let mut changed = false;
        for tool_id in self.config.iter_tools() {
            let tool = self.config.get_tool_spec(tool_id).unwrap();
            if_chain! {
                if let Some(ref rti) = tool.include;
                if let Some(ref tool_dir_base) = tool.tool_dir_base;
                then {
                    self.log_step(&format!("Pulling dependencies for '{}'", tool_id));
                    update_remote_tool(&tool_dir_base, &rti)?;
                    changed = true;
                }
            }
        }

        if changed {
            self.config = Config::from_env()?;
        }

        Ok(())
    }

    pub fn update(&self) -> Result<()> {
        self.log_step("Updating toolchains");
        for tool_id in self.config.iter_tools() {
            let tool = self.create_tool(tool_id)?;
            tool.update()?;
        }
        self.log_step("Updated");
        Ok(())
    }

    pub fn lint(&self, files: Option<&[&Path]>) -> Result<Report> {
        let mut report = Report::new(self);

        for tool_id in self.config.iter_tools() {
            let tool = self.create_tool(tool_id)?;
            tool.lint(&mut report, files)?;
        }

        report.sort();
        Ok(report)
    }

    pub fn format(&self, files: &[&Path]) -> Result<FormatResult> {
        let mut rv = FormatResult::new();

        for file in files {
            rv.register_file(file)?;
        }

        for tool_id in self.config.iter_tools() {
            let tool = self.create_tool(tool_id)?;
            if !tool.format(&mut rv, files)? {
                fail!("formatter '{}' failed", tool_id);
            }
        }

        Ok(rv)
    }

    pub fn is_lintable_file<P: AsRef<Path>>(&self, p: P) -> Result<bool> {
        for tool_id in self.config.iter_tools() {
            let t = self.create_tool(tool_id)?;
            if t.does_lint_file(p.as_ref())? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn find_command(&self, cmd_name: &str) -> Result<Option<PathBuf>> {
        let mut pathstr = String::new();
        let mut first = true;
        for tool_id in self.config.iter_tools() {
            let t = self.create_tool(tool_id)?;
            let mut paths = vec![];
            t.add_search_paths(&mut paths)?;
            for path in paths {
                if first {
                    first = false;
                } else {
                    pathstr.push(':');
                }
                pathstr.push_str(&path.display().to_string());
            }
        }
        let here = env::current_dir()?;
        if let Ok(rv) = which_in(cmd_name, Some(pathstr), here) {
            Ok(Some(rv))
        } else {
            Ok(None)
        }
    }
}
