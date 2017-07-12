use std::fs;
use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::os::unix::fs::symlink;
use std::sync::Arc;
use std::collections::HashMap;

use prelude::*;
use ctx::Context;
use rt::common::Runtime;
use utils::cmd::{CommandBuilder, CommandHandlers};
use config::{ToolSpec, ToolStep, ToolCommand, ReportPatternMatch};
use report::Report;
use formatting::FormatResult;

use parking_lot::Mutex;
use serde_json;


#[derive(Debug)]
pub struct Tool<'a> {
    spec: &'a ToolSpec,
    id: String,
    ctx: &'a Context,
    runtimes: Vec<Box<Runtime<'a> + 'a>>,
}

#[derive(Default, Debug)]
pub struct RunStepOptions<'a, 'b: 'a, 'c> {
    report: Option<&'a mut Report<'b>>,
    file_args: Vec<&'c Path>,
}

impl<'a> Tool<'a> {
    pub fn new(ctx: &'a Context, id: &str, spec: &'a ToolSpec) -> Result<Tool<'a>> {
        let mut runtimes = vec![];
        for (id, cfg) in spec.runtimes.iter() {
            runtimes.push(ctx.create_runtime(id, cfg)?);
        }

        Ok(Tool {
            ctx: ctx,
            id: id.to_string(),
            spec: spec,
            runtimes: runtimes,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn tool_dir<'b>(&'b self) -> Cow<'b, Path> {
        if let Some(rv) = self.spec.tool_dir_prefix() {
            rv
        } else {
            Cow::Borrowed(self.ctx.config().config_dir())
        }
    }

    pub fn add_search_paths(&self, path: &mut Vec<PathBuf>) -> Result<()> {
        for rt in &self.runtimes {
            rt.add_search_paths(path)?;
        }
        Ok(())
    }

    pub fn run_step(&self, step: &ToolStep,
                    opts: Option<&mut RunStepOptions>) -> Result<bool> {
        let mut path = vec![];
        for rt in &self.runtimes {
            rt.add_search_paths(&mut path)?;
        }
        self.ctx.log_step(&step.description());

        // build the updated environment variables
        let mut env = HashMap::new();
        for rt in &self.runtimes {
            rt.update_env(&mut |key, value| {
                env.insert(key.to_string_lossy().to_string(),
                           value.to_string_lossy().to_string());
            })?;
        }

        env.insert("CALM_TOOL_PATH".to_string(),
                   self.tool_dir().display().to_string());

        // link resources
        if let Some(res) = step.link() {
            let from_path = self.tool_dir().join(&res.src(Some(&env)));
            let target_path = self.ctx.base_dir().join(&res.dst(Some(&env)));
            fs::remove_file(&target_path).ok();
            symlink(from_path, target_path)?;
            Ok(true)
        }

        // execute commands
        else if let Some(tool_cmd) = step.cmd() {
            let mut cmd;
            match tool_cmd {
                &ToolCommand::Shell(ref cmdline) => {
                    cmd = CommandBuilder::new_shell(cmdline);
                }
                &ToolCommand::Exec(ref args) => {
                    if args.is_empty() {
                        return Err(Error::from("empty arguments for tool step"));
                    }
                    cmd = CommandBuilder::new(&args[0]);
                    for arg in &args[1..] {
                        cmd.arg(arg);
                    }
                }
            }

            // configure process
            cmd.search_path(&path);
            cmd.current_dir(self.ctx.base_dir());
            for (ref key, ref value) in &env {
                cmd.env(key, value);
            }
            for rt in &self.runtimes {
                rt.configure_run_step(&mut cmd)?;
            }

            // add all file arguments as extra arguments to the script
            if let Some(file_args) = opts.as_ref().map(|x| &x.file_args[..]) {
                for file_arg in file_args {
                    cmd.arg(file_arg);
                }
            }

            let process = cmd.spawn()?;
            let mut handlers: CommandHandlers = Default::default();

            if let Some(report) = opts.and_then(|mut x| x.report.as_mut()) {
                let report = Arc::new(Mutex::new(report));

                macro_rules! configure_actions {
                    ($actions:expr, $target_field:ident) => {
                        if let Some(actions) = $actions {
                            if let Some(ref parse_lines) = actions.parse_lines {
                                let report = report.clone();
                                handlers.expect = false;
                                handlers.$target_field = Some(Box::new(move |line| {
                                    if let Some(m) = parse_lines.pattern.match_str(line) {
                                        if parse_lines.report_match == ReportPatternMatch::LintResult {
                                            let mut rep = report.lock();
                                            let res = rep.add_match_lint_result(self, &m)?;
                                            return Ok(match res.filename {
                                                Some(ref filename) => {
                                                    Cow::Owned(format!(
                                                        "Found issue in {}", filename.display()))
                                                },
                                                None => {
                                                    Cow::Borrowed("Found new general issue")
                                                }
                                            });
                                        }
                                    }
                                    Ok(Cow::Borrowed("Linting ..."))
                                }));
                            }
                            if actions.parse_lint_json {
                                let report = report.clone();
                                handlers.expect = false;
                                handlers.$target_field = Some(Box::new(move |line| {
                                    let res = serde_json::from_str(&line)?;
                                    let mut rep = report.lock();
                                    let _res = rep.add_lint_result(self, res)?;
                                    Ok(Cow::Borrowed(""))
                                }));
                            }
                        }
                    }
                }

                configure_actions!(step.stdout_actions(), on_stdout);
                configure_actions!(step.stderr_actions(), on_stderr);
            }

            process.wait_with_handlers(handlers)
        } else {
            Err(Error::from("Empty tool step"))
        }
    }

    pub fn update(&self) -> Result<()> {
        for rt in &self.runtimes {
            rt.update()?;
        }

        for step in &self.spec.install_steps {
            self.run_step(step, None)?;
        }

        Ok(())
    }

    pub fn does_lint_file(&self, path: &Path) -> Result<bool> {
        if let Some(ref lint_spec) = self.spec.lint {
            for pat in &lint_spec.patterns {
                if pat.match_path(path) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn lint(&self, report: &mut Report, files: Option<&[&Path]>) -> Result<bool> {
        if let Some(ref lint_spec) = self.spec.lint {
            let base = self.ctx.base_dir();
            let mut failed = false;
            let mut opts = RunStepOptions {
                report: Some(report),
                file_args: files.map(|x| x.iter().filter_map(|&x| {
                    for pat in &lint_spec.patterns {
                        if pat.match_path(x) {
                            return Some(x.strip_prefix(base).unwrap_or(x));
                        }
                    }
                    None
                }).collect()).unwrap_or(vec![]),
            };

            // if no files are passed to the runner but an explicit file
            // list was given to the lint function, we bail without
            // running as no files would mean all files.
            if opts.file_args.is_empty() && files.is_some() {
                return Ok(true);
            }

            for step in &lint_spec.run {
                if !self.run_step(step, Some(&mut opts))? {
                    failed = true;
                }
            }
            Ok(!failed)
        } else {
            // no lint configured, success!
            Ok(true)
        }
    }

    pub fn format(&self, fr: &mut FormatResult, files: &[&Path]) -> Result<bool> {
        if let Some(ref format_spec) = self.spec.format {
            let mut failed = false;
            let mut file_args = vec![];
            for file in files.iter() {
                for pat in &format_spec.patterns {
                    if pat.match_path(file) {
                        file_args.push(fr.get_scratch_file(file)?);
                        break;
                    }
                }
            }

            if file_args.is_empty() {
                return Ok(true);
            }

            let mut opts = RunStepOptions {
                report: None,
                file_args: file_args,
            };
            for step in &format_spec.run {
                if !self.run_step(step, Some(&mut opts))? {
                    failed = true;
                }
            }
            Ok(!failed)
        } else {
            Ok(true)
        }
    }
}
