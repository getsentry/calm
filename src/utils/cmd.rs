use std::io::{Read, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::env;
use std::process;
use std::borrow::Cow;

use indicatif::{ProgressBar, ProgressStyle};
use crossbeam;
use console::style;
use regex::{Regex, Captures};

use prelude::*;


pub struct CommandBuilder {
    cmd_name: String,
    cmd: process::Command,
    cmdline: Option<String>,
    args: Vec<OsString>,
}

pub struct Command {
    cmd_name: String,
    bar: ProgressBar,
    child: process::Child,
}

pub struct CommandHandlers<'a> {
    pub on_stdout: Option<Box<FnMut(&str) -> Result<Cow<'static, str>> + Send + Sync + 'a>>,
    pub on_stderr: Option<Box<FnMut(&str) -> Result<Cow<'static, str>> + Send + Sync + 'a>>,
    pub expect: bool,
}

impl<'a> Default for CommandHandlers<'a> {
    fn default() -> CommandHandlers<'a> {
        CommandHandlers {
            on_stdout: None,
            on_stderr: None,
            expect: true,
        }
    }
}

fn process<'a, R: Read>(r: R, prefix: &str, bar: &ProgressBar,
                        mut f: Option<&mut Box<FnMut(&str)
                            -> Result<Cow<'static, str>> + Send + Sync + 'a>>)
    -> Result<()>
{
    bar.set_message(&format!("{}: Running ...", style(prefix).cyan()));
    for line in BufReader::new(r).lines() {
        let line = line?;
        let text = if let Some(ref mut f) = f {
            f(&line)?
        } else {
            Cow::Borrowed(line.trim())
        };
        if !text.is_empty() {
            bar.set_message(&format!("{}: {}",
                                     style(prefix).cyan(),
                                     text));
        }
    }
    Ok(())
}

impl Command {
    fn new(child: process::Child, cmd_name: String) -> Command {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .tick_chars("⢄⢂⢁⡁⡈⡐⡠ ")
            .template("{prefix:.cyan} {spinner:.green} {wide_msg}"));
        pb.set_prefix(">");
        pb.enable_steady_tick(100);
        Command {
            cmd_name: cmd_name,
            bar: pb,
            child: child,
        }
    }

    pub fn name(&self) -> &str {
        &self.cmd_name
    }

    pub fn wait_with_handlers(mut self, mut handlers: CommandHandlers) -> Result<bool> {
        let stdout = self.child.stdout.take().unwrap();
        let mut on_stdout = handlers.on_stdout.take();
        let on_stdout_mut = on_stdout.as_mut();
        let stderr = self.child.stderr.take().unwrap();
        let mut on_stderr = handlers.on_stderr.take();
        let on_stderr_mut = on_stderr.as_mut();

        let bar = &self.bar;
        {
            let prefix = self.name();
            crossbeam::scope(|scope| {
                scope.spawn(move || {
                    process(stdout, prefix, bar, on_stdout_mut).unwrap();
                });
                scope.spawn(move || {
                    process(stderr, prefix, bar, on_stderr_mut).unwrap();
                });
            });
        }

        let output = self.child.wait_with_output()?;
        self.bar.finish_and_clear();

        if handlers.expect && !output.status.success() {
            return Err(Error::from(format!("{} failed with {}", &self.cmd_name, output.status)));
        }

        Ok(output.status.success())
    }

    pub fn wait(self) -> Result<bool> {
        self.wait_with_handlers(Default::default())
    }
}

impl CommandBuilder {
    pub fn new(cmd: &str) -> CommandBuilder {
        CommandBuilder {
            cmd_name: Path::new(cmd)
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or(cmd)
                .to_string(),
            cmd: process::Command::new(cmd),
            args: vec![],
            cmdline: None,
        }
    }

    pub fn new_shell(cmdline: &str) -> CommandBuilder {
        let mut cmd = process::Command::new("sh");
        cmd.arg("-c");
        CommandBuilder {
            cmd_name: Path::new(cmdline.split_whitespace().next().unwrap_or(""))
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or(cmdline)
                .to_string(),
            cmd: cmd,
            args: vec![],
            cmdline: Some(cmdline.to_string()),
        }
    }

    pub fn current_dir<S: AsRef<Path>>(&mut self, arg: S) -> &mut CommandBuilder {
        self.cmd.current_dir(arg);
        self
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut CommandBuilder {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub fn search_path(&mut self, paths: &[PathBuf]) -> &mut CommandBuilder {
        let mut path = String::new();
        for item in paths {
            path.push_str(&format!("{}:", item.display()));
        }
        if let Ok(default_path) = env::var("PATH") {
            path.push_str(&default_path);
        }
        self.cmd.env("PATH", path);
        self
    }

    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, key: K, value: V) -> &mut CommandBuilder {
        self.cmd.env(key, value);
        self
    }

    pub fn spawn(&mut self) -> Result<Command> {
        self.cmd.stdout(process::Stdio::piped());
        self.cmd.stderr(process::Stdio::piped());

        if let Some(ref cmdline) = self.cmdline {
            let mut cmdline = cmdline.to_string();
            for arg in &self.args {
                cmdline.push_str(&format!(" \"{}\"", arg.to_string_lossy()));
            }
            self.cmd.arg(cmdline);
        } else {
            for arg in &self.args {
                self.cmd.arg(&arg);
            }
        }

        Ok(Command::new(self.cmd.spawn()?, self.cmd_name.clone()))
    }
}

/// Expands variables in a string
pub fn expand_vars<'a, F: Fn(&str) -> String>(s: &'a str, f: F) -> Cow<'a, str> {
    lazy_static! {
        static ref VAR_RE: Regex = Regex::new(
            r"\$(\$|[a-zA-Z0-9_]+|\([^)]+\)|\{[^}]+\})").unwrap();
    }
    VAR_RE.replace_all(s, |caps: &Captures| {
        let key = &caps[1];
        if key == "$" {
            "$".into()
        } else if &key[..1] == "(" || &key[..1] == "{" {
            f(&key[1..key.len() - 1])
        } else {
            f(key)
        }
    })
}
