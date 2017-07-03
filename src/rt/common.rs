use std::fmt::Debug;
use std::path::PathBuf;
use std::ffi::OsStr;

use prelude::*;
use config::RuntimeConfig;
use ctx::Context;
use utils::cmd::CommandBuilder;

pub trait Runtime<'a>: Debug + Sync {
    /// Return the context this runtime was created from.
    fn ctx(&'a self) -> &'a Context;

    /// Return the context this runtime was created from.
    fn config(&'a self) -> &'a RuntimeConfig;

    /// Returns the type name of the runtime.
    fn type_name(&self) -> &str;

    /// Returns the unique id of the runtime.
    fn id(&self) -> &str;

    /// Initializes or updates the runtime in the context.
    fn update(&self) -> Result<()>;

    /// Adds all search paths to a vector
    fn add_search_paths(&self, _paths: &mut Vec<PathBuf>) -> Result<()> {
        Ok(())
    }

    /// Provides extra environment variables.
    fn update_env(&self, _f: &mut FnMut(&OsStr, &OsStr)) -> Result<()> {
        Ok(())
    }

    /// Adds arbitrary configuration to a command before launching
    fn configure_run_step(&self, _builder: &mut CommandBuilder) -> Result<()> {
        Ok(())
    }

    /// Returns the path to where the runtime lives in the runtime
    /// context.  This will also return a path in case the runtime
    /// has not been created in the context yet.
    fn get_path(&'a self) -> PathBuf {
        self.ctx().cache_dir().join("rt").join(self.id().to_string())
    }
}
