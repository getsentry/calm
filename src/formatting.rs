use std::io;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use tempfile::{NamedTempFile, NamedTempFileOptions};

use prelude::*;

pub struct FormatResult {
    files: HashMap<PathBuf, NamedTempFile>,
}

impl FormatResult {
    pub fn new() -> FormatResult {
        FormatResult {
            files: HashMap::new(),
        }
    }

    pub fn register_file<P: AsRef<Path>>(&mut self, filename: P) -> Result<()> {
        let mut sf = fs::File::open(filename.as_ref())?;
        let dft = NamedTempFileOptions::new()
            .prefix(".calm-format-")
            .suffix(&format!("-{}", filename.as_ref().file_name().and_then(|x| x.to_str()).unwrap()))
            .rand_bytes(14)
            .create()?;
        let mut df = dft.reopen()?;
        println!("{}", dft.path().display());
        io::copy(&mut sf, &mut df)?;
        self.files.insert(filename.as_ref().to_path_buf(), dft);
        Ok(())
    }

    pub fn print_diff(&self) -> Result<()> {
        Ok(())
    }
}
