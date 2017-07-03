use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::ffi::OsStr;

use prelude::*;
use config::RuntimeConfig;
use ctx::Context;
use rt::common::Runtime;
use utils::cmd::CommandBuilder;

use sha1::Sha1;

#[derive(Debug)]
pub struct JsRuntime<'a> {
    ctx: &'a Context,
    config_hash: String,
    config: &'a RuntimeConfig,
}

impl<'a> JsRuntime<'a> {
    pub fn create(ctx: &'a Context, config: &'a RuntimeConfig)
        -> JsRuntime<'a>
    {
        let mut sha1 = Sha1::new();
        sha1.update(b"javascript\x00");

        JsRuntime {
            ctx: ctx,
            config_hash: sha1.digest().to_string(),
            config: config,
        }
    }
}

impl<'a> Runtime<'a> for JsRuntime<'a> {
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
        "javascript"
    }

    fn add_search_paths(&self, paths: &mut Vec<PathBuf>) -> Result<()> {
        paths.push(self.get_path().join("node_modules/.bin"));
        Ok(())
    }

    fn update_env(&self, f: &mut FnMut(&OsStr, &OsStr)) -> Result<()> {
        f(OsStr::new("NODE_PATH"),
          self.get_path().join("node_modules").as_os_str());
        f(OsStr::new("CALM_JAVASCRIPT_BIN"),
          self.get_path().join("node_modules/.bin").as_os_str());
        f(OsStr::new("CALM_JAVASCRIPT_BASE"),
          self.get_path().as_os_str());
        f(OsStr::new("CALM_JAVASCRIPT_PACKAGE_JSON"),
          self.get_path().join("package.json").as_os_str());
        Ok(())
    }

    fn update(&self) -> Result<()> {
        let path = self.get_path();

        fs::create_dir_all(&path)?;

        // dump an empty package.json if one is missing
        if !fs::metadata(path.join("package.json")).is_ok() {
            self.ctx.log_step("Bootstrapping environment");
            let mut f = fs::File::create(path.join("package.json"))?;
            f.write_all(r#"
                {
                  "name": "calm-js-scratchpad",
                  "version": "0.0.1",
                  "description": "",
                  "author": "",
                  "license": "ISC",
                  "dependencies": {
                      "yarn": "*"
                  }
                }
            "#.as_bytes())?;
        }

        // install yarn if missing
        if !fs::metadata(path.join("node_modules/.bin/yarn")).is_ok() {
            self.ctx.log_step("Installing yarn");
            let mut cmd = CommandBuilder::new("npm");
            cmd
                .current_dir(&path)
                .arg("install")
                .arg("-d");
            self.configure_run_step(&mut cmd)?;
            cmd.spawn()?.wait()?;
        }

        // install yarn dependencies
        if !self.config.packages().is_empty() {
            self.ctx.log_step("Installing javascript packages");
            let mut cmd = CommandBuilder::new("yarn");
            cmd
                .current_dir(&path)
                .arg("add");

            for (ref pkg_name, ref version) in self.config.packages() {
                cmd.arg(format!("{}@{}", pkg_name, version));
            }

            self.configure_run_step(&mut cmd)?;
            cmd.spawn()?.wait()?;
        }

        Ok(())
    }
}
