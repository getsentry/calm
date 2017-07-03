use std::fs;
use std::str;
use std::env;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::collections::HashMap;
use std::borrow::Cow;

use prelude::*;
use ctx::Context;
use tools::Tool;

use regex::Regex;
use console::style;

lazy_static! {
    static ref IDENT_RE: Regex = Regex::new(
        r#"(?x)
            [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}$_]
            [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}\p{Mn}\p{Mc}\p{Nd}\p{Pc}$_]*
        "#).unwrap();
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Level {
    #[serde(rename="error")]
    Error,
    #[serde(rename="warning")]
    Warning,
    #[serde(rename="info")]
    Info,
}

impl Default for Level {
    fn default() -> Level {
        Level::Error
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct LintResult {
    pub filename: PathBuf,
    pub line: u64,
    pub column: u64,
    pub code: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub level: Level,
}

impl fmt::Display for LintResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if_chain! {
            if let Ok(p) = env::current_dir();
            if let Ok(prefix) = self.filename.strip_prefix(&p);
            then {
                write!(f, "{}", style(prefix.display()).cyan())?;
            }
            else {
                write!(f, "{}", style(self.filename.display()).cyan())?;
            }
        }

        write!(f,
               ":{}:{} {} {}",
               style(self.line).yellow(),
               style(self.column).yellow(),
               style(self.code.as_ref().map(|x| x.as_str()).unwrap_or("E")).magenta().italic(),
               self.message.as_ref().map(|x| x.as_str()).unwrap_or("no info"))?;

        if_chain! {
            if f.alternate() && self.line > 0;
            if let Ok(sf) = fs::File::open(&self.filename);
            if let Some(Ok(line)) = BufReader::new(sf).lines().skip(self.line as usize - 1).next();
            then {
                let stripped_line = line.trim_left();
                let tok_start = self.column as usize - 1 - (line.len() - stripped_line.len());
                let mut tok_len = 1;
                if let Some(m) = IDENT_RE.captures(&stripped_line[tok_start..]) {
                    let g = m.get(0).unwrap();
                    tok_len = g.end() - g.start();
                }

                write!(f, "\n  {}", style(stripped_line.trim_right()).dim())?;
                if self.column > 0 {
                    write!(f, "\n  {}{}",
                           str::repeat(" ", tok_start),
                           style(str::repeat("^", tok_len)).red().dim())?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Report<'a> {
    lint_results: Vec<LintResult>,
    ctx: &'a Context,
}

impl<'a> Report<'a> {
    pub fn new(ctx: &'a Context) -> Report<'a> {
        Report {
            lint_results: vec![],
            ctx: ctx,
        }
    }

    pub fn add_match_lint_result(&mut self, tool: &Tool, matches: &HashMap<Cow<str>, Cow<str>>)
        -> Result<&LintResult>
    {
        let idx = self.lint_results.len();
        let f: &str = &matches.get("filename").ok_or_else(||
            Error::from("No filename in lint result pattern"))?;
        self.lint_results.push(LintResult {
            filename: self.ctx.base_dir().join(f).canonicalize()?,
            line: matches.get("line").and_then(|x| x.parse().ok()).unwrap_or(0),
            column: matches.get("column").and_then(|x| x.parse().ok()).unwrap_or(0),
            code: matches.get("code").map(|x| format!("{}:{}", tool.id(), x)),
            message: matches.get("message").map(|x| x.to_string()),
            level: matches.get("level").map(|x| {
                match x.to_lowercase().as_str() {
                    "error" | "e" | "err" => Level::Error,
                    "warning" | "w" | "warn" => Level::Warning,
                    "info" => Level::Info,
                    _ => Level::Error,
                }
            }).unwrap_or(Level::Error)
        });
        Ok(&self.lint_results[idx])
    }

    pub fn add_lint_result(&mut self, tool: &Tool, mut res: LintResult)
        -> Result<&LintResult>
    {
        let idx = self.lint_results.len();
        res.filename = self.ctx.base_dir().join(&res.filename).canonicalize()?;
        res.code = res.code.map(|code| format!("{}:{}", tool.id(), code));
        self.lint_results.push(res);
        Ok(&self.lint_results[idx])
    }

    pub fn sort(&mut self) {
        self.lint_results.sort();
    }

    pub fn print(&self) {
        if self.lint_results.is_empty() {
            return;
        }
        for res in &self.lint_results {
            println!("{:#}", res);
        }
    }
}
