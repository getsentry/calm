use std::io;
use std::io::{Read, Write, BufRead, BufReader};
use std::fs;
use std::env;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use tempfile::{NamedTempFile, NamedTempFileOptions};
use difflib::unified_diff;
use console::style;

use prelude::*;

pub struct FormatResult {
    files: HashMap<PathBuf, NamedTempFile>,
}

fn read_lines<R: Read>(r: R) -> Result<Vec<String>> {
    let mut rv = vec![];
    let mut r = BufReader::new(r);
    loop {
        let mut buf = String::new();
        let read = r.read_line(&mut buf)?;
        if read == 0 {
            break;
        }
        rv.push(buf);
    }
    Ok(rv)
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
        io::copy(&mut sf, &mut df)?;
        self.files.insert(filename.as_ref().canonicalize()?, dft);
        Ok(())
    }

    pub fn get_scratch_file<P: AsRef<Path>>(&self, filename: P) -> Result<&Path> {
        self.files.get(&filename.as_ref().canonicalize()?).map(|x| x.path())
            .ok_or_else(|| Error::from("tried to get unregistered scratch file"))
    }

    pub fn print_diff(&self) -> Result<()> {
        let here = env::current_dir()?;
        for (file_path, tf) in &self.files {
            let old_lines = read_lines(fs::File::open(&file_path)?)?;
            let new_lines = read_lines(tf.reopen()?)?;

            if old_lines == new_lines {
                continue;
            }

            let rel_path = file_path.strip_prefix(&here).ok().unwrap_or_else(|| &file_path);
            let diff = unified_diff(&old_lines, &new_lines,
                                    &format!("a/{}", rel_path.display()),
                                    &format!("b/{}", rel_path.display()),
                                    "", "", 5);
            for line in diff {
                write!(&mut io::stdout(), "{}", line)?;
            }
        }

        Ok(())
    }

    pub fn apply(&self) -> Result<()> {
        let here = env::current_dir()?;
        for (file_path, tf) in &self.files {
            let mut old = String::new();
            fs::File::open(&file_path)?.read_to_string(&mut old)?;
            let mut new = String::new();
            tf.reopen()?.read_to_string(&mut new)?;
            if old == new {
                continue;
            }

            let mut f = fs::File::create(&file_path)?;
            f.write_all(new.as_bytes())?;

            let path = file_path.strip_prefix(&here).unwrap_or(&file_path);
            println_stderr!("Formatted {}", style(path.display()).cyan());
        }
        Ok(())
    }
}
