use std::path::PathBuf;
use std::collections::HashSet;

use git2;

use prelude::*;


pub fn get_changed_files() -> Result<Vec<PathBuf>> {
    let repo = git2::Repository::open_from_env()?;
    let base = repo.workdir().ok_or_else(|| Error::from("No working directory found"))?;
    let diff = repo.diff_index_to_workdir(None, None)?;
    let mut sets = HashSet::new();
    
    for delta in diff.deltas() {
        if let Some(path) = delta.old_file().path() {
            sets.insert(base.join(path));
        }
        if let Some(path) = delta.new_file().path() {
            sets.insert(base.join(path));
        }
    }

    let mut rv = sets.into_iter().collect::<Vec<_>>();
    rv.sort();
    Ok(rv)
}
