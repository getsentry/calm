use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;

use git2;
use regex::{Captures, Regex};

use prelude::*;


lazy_static! {
    static ref HOOK_RE: Regex = Regex::new(
        r#"(?m)^calm\s+hook\s+--exec-([\w-]+)\s+\|\|\s+exit\s+1\s*?\r?\n?"#).unwrap();
}


pub struct HookManager {
    repo: git2::Repository,
}

pub struct HookStatus {
    pub pre_commit_installed: bool,
}


impl HookManager {
    pub fn new() -> Result<HookManager> {
        Ok(HookManager {
            repo: git2::Repository::open_from_env()?,
        })
    }

    fn get_hook_file(&self, hook: &str) -> PathBuf {
        self.repo.path().join("hooks").join(hook)
    }

    fn is_hook_installed(&self, hook: &str) -> Result<bool> {
        if let Ok(mut f) = fs::File::open(self.get_hook_file(hook)) {
            let mut contents = String::new();
            f.read_to_string(&mut contents)?;
            for m in HOOK_RE.captures(&contents) {
                if &m[1] == hook {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn add_hook(&self, hook: &str) -> Result<()> {
        let filename = self.get_hook_file(hook);
        let mut contents = String::new();
        if let Ok(mut f) = fs::File::open(&filename) {
            f.read_to_string(&mut contents)?;
        } else {
            contents.push_str("#!/bin/sh\n");
        }
        use std::fmt::Write;
        write!(&mut contents, "calm hook --exec-{} || exit 1\n", hook).unwrap();
        let mut f = fs::File::create(&filename)?;
        f.write_all(contents.as_bytes())?;

        // make sure it's executable
        let mut perm = fs::metadata(&filename)?.permissions();
        let old_mode = perm.mode();
        perm.set_mode(old_mode | 0o111);
        fs::set_permissions(&filename, perm)?;

        Ok(())
    }

    fn remove_hook(&self, hook: &str) -> Result<()> {
        let mut contents = String::new();
        if let Ok(mut f) = fs::File::open(self.get_hook_file(hook)) {
            f.read_to_string(&mut contents)?;
        } else {
            return Ok(());
        }
        let mut f = fs::File::create(self.get_hook_file(hook))?;
        f.write_all(HOOK_RE.replace_all(&contents, |caps: &Captures| {
            if &caps[1] == hook {
                "".to_string()
            } else {
                caps[0].to_string()
            }
        }).as_bytes())?;
        Ok(())
    }

    pub fn status(&self) -> Result<HookStatus> {
        Ok(HookStatus {
            pre_commit_installed: self.is_hook_installed("pre-commit")?,
        })
    }

    pub fn install_hooks(&self) -> Result<()> {
        if !self.is_hook_installed("pre-commit")? {
            self.add_hook("pre-commit")?;
        }
        Ok(())
    }

    pub fn uninstall_hooks(&self) -> Result<()> {
        if self.is_hook_installed("pre-commit")? {
            self.remove_hook("pre-commit")?;
        }
        Ok(())
    }
}
