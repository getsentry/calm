use std::io::Write;
use std::env;
use std::path::{Path, PathBuf};

use sha1::Sha1;

use prelude::*;
use config::{Config, RuntimeConfig};
use tools::Tool;
use report::Report;
use rt;
use rt::common::Runtime;

use console::{style, Term};
use parking_lot::Mutex;

#[derive(Debug)]
struct Log {
    lines: usize,
}

#[derive(Debug)]
pub struct Context {
    config: Config,
    cache_dir: PathBuf,
    base_dir: PathBuf,
    log: Mutex<Log>,
}

impl Context {
    pub fn new(config: Config) -> Result<Context> {
        let mut sha = Sha1::new();
        sha.update(config.filename().to_string_lossy().as_bytes());

        let mut cache_dir = env::home_dir().ok_or(
            Error::from("could not find home folder"))?;
        cache_dir.push(".calm");
        cache_dir.push("rt");
        cache_dir.push(sha.digest().to_string());

        Ok(Context {
            base_dir: config.config_dir().parent().unwrap().to_path_buf(),
            config: config,
            cache_dir: cache_dir,
            log: Mutex::new(Log {
                lines: 0,
            }),
        })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn log_step(&self, text: &str) {
        let mut log = self.log.lock();
        write!(&mut ::std::io::stdout(), "{} {}\n",
               style(">").dim().bold(),
               text).unwrap();
        log.lines += 1;
    }

    pub fn clear_log(&self) {
        let mut log = self.log.lock();
        Term::stdout().clear_last_lines(log.lines).unwrap();
        log.lines = 0;
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

    pub fn update(&self) -> Result<()> {
        self.log_step("Updating toolchains");
        for tool_id in self.config.iter_tools() {
            let tool = self.create_tool(tool_id)?;
            tool.update()?;
        }
        self.log_step("Updated");
        Ok(())
    }

    pub fn lint(&self, files: Option<&[&Path]>) -> Result<bool> {
        let mut report = Report::new(self);
        let mut failed = false;

        for tool_id in self.config.iter_tools() {
            let tool = self.create_tool(tool_id)?;
            if !tool.lint(&mut report, files)? {
                failed = true;
            }
        }

        report.sort();

        self.clear_log();
        report.print();

        Ok(!failed)
    }
}
