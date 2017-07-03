use std::fs;
use std::path::PathBuf;
use std::ffi::OsStr;

use prelude::*;
use config::RuntimeConfig;
use ctx::Context;
use rt::common::Runtime;
use utils::cmd::CommandBuilder;

use sha1::Sha1;

const DEFAULT_FLAVOR: &'static str = "python3";

#[derive(Debug)]
pub struct PythonRuntime<'a> {
    ctx: &'a Context,
    config_hash: String,
    config: &'a RuntimeConfig,
}

impl<'a> PythonRuntime<'a> {
    pub fn create(ctx: &'a Context, config: &'a RuntimeConfig)
        -> PythonRuntime<'a>
    {
        let mut sha1 = Sha1::new();
        sha1.update(b"python\x00");
        if let Some(flavor) = config.flavor() {
            sha1.update(flavor.as_bytes());
        } else {
            sha1.update(DEFAULT_FLAVOR.as_bytes());
        }

        PythonRuntime {
            ctx: ctx,
            config_hash: sha1.digest().to_string(),
            config: config,
        }
    }
}

impl<'a> Runtime<'a> for PythonRuntime<'a> {
    fn ctx(&self) -> &Context {
        self.ctx
    }

    fn config(&self) -> &RuntimeConfig {
        self.config
    }

    fn id(&self) -> &str {
        &self.config_hash
    }

    fn type_name(&self) -> &str {
        "python"
    }

    fn add_search_paths(&self, paths: &mut Vec<PathBuf>) -> Result<()> {
        paths.push(self.get_path().join("bin"));
        Ok(())
    }

    fn update_env(&self, f: &mut FnMut(&OsStr, &OsStr)) -> Result<()> {
        let env_prefix = self.config.flavor().unwrap_or("python").to_uppercase();
        f(OsStr::new(&format!("CALM_{}_VENV", &env_prefix)),
          self.get_path().as_os_str());
        f(OsStr::new(&format!("CALM_{}_BIN", &env_prefix)),
          self.get_path().join("bin").as_os_str());
        f(OsStr::new(&format!("CALM_{}_LIB", &env_prefix)),
          self.get_path().join("lib").as_os_str());
        Ok(())
    }

    fn update(&self) -> Result<()> {
        let path = self.get_path();

        fs::create_dir_all(&path)?;

        // only make a venv if there is none yet
        if !fs::metadata(path.join("bin").join("python")).is_ok() {
            self.ctx.log_step(&format!("Bootstrapping virtualenv ({})",
                                       self.config.flavor().unwrap_or("default python")));
            let mut cmd = CommandBuilder::new("virtualenv");
            cmd.arg(&path);
            if let Some(flavor) = self.config.flavor() {
                cmd.arg("-p").arg(flavor);
            }
            cmd.spawn()?.wait()?;
        }

        // Ensure we have a recent pip
        self.ctx.log_step("Updating pip");
        CommandBuilder::new("bin/pip")
            .current_dir(&path)
            .arg("install")
            .arg("--upgrade")
            .arg("pip")
            .spawn()?
            .wait()?;

        // install dependencies
        if !self.config.packages().is_empty() {
            self.ctx.log_step("Installing python packages");
            let mut cmd = CommandBuilder::new("bin/pip");
            cmd
                .current_dir(&path)
                .arg("install");

            for (ref pkg_name, ref version) in self.config.packages() {
                cmd.arg(format!("{}=={}", pkg_name, version));
            }

            cmd.spawn()?.wait()?;
        }

        Ok(())
    }
}
