use std::io;
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
use elementtree::Element;

use regex::Regex;
use console::{Style, style};

lazy_static! {
    static ref IDENT_RE: Regex = Regex::new(
        r#"(?x)
            [\d\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}$_]
            [\d\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}\p{Mn}\p{Mc}\p{Nd}\p{Pc}$_]*
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    Human,
    Simple,
    Checkstyle
}

impl str::FromStr for Format {
    type Err = Error;

    fn from_str(s: &str) -> Result<Format> {
        match s {
            "human" => Ok(Format::Human),
            "simple" => Ok(Format::Simple),
            "checkstyle" => Ok(Format::Checkstyle),
            other => Err(Error::from(format!("Unknown format '{}'", other))),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct LintResult {
    pub filename: Option<PathBuf>,
    pub line: u64,
    pub column: u64,
    pub code: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub level: Level,
}

pub struct LintResultSimpleFormat<'a> {
    lr: &'a LintResult,
}

impl fmt::Display for LintResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref filename) = self.filename {
            if_chain! {
                if let Ok(p) = env::current_dir();
                if let Ok(prefix) = filename.strip_prefix(&p);
                then {
                    write!(f, "{}", style(prefix.display()).cyan())?;
                }
                else {
                    write!(f, "{}", style(filename.display()).cyan())?;
                }
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
            if let Some(ref filename) = self.filename;
            if let Ok(sf) = fs::File::open(filename);
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

impl<'a> fmt::Display for LintResultSimpleFormat<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref filename) = self.lr.filename {
            if_chain! {
                if let Ok(p) = env::current_dir();
                if let Ok(prefix) = filename.strip_prefix(&p);
                then {
                    write!(f, "{}", prefix.display())?;
                }
                else {
                    write!(f, "{}", filename.display())?;
                }
            }
        }

        write!(f,
               ":{}:{}:{} [{}]",
               self.lr.line,
               self.lr.column,
               self.lr.message.as_ref().map(|x| x.as_str()).unwrap_or("no info"),
               self.lr.code.as_ref().map(|x| x.as_str()).unwrap_or("E"))?;

        Ok(())
    }
}

impl LintResult {
    pub fn simple_format<'a>(&'a self) -> LintResultSimpleFormat<'a> {
        LintResultSimpleFormat {
            lr: self,
        }
    }
}

#[derive(Debug)]
pub struct Report<'a> {
    lint_results: Vec<LintResult>,
    ctx: &'a Context,
    errors: u64,
    warnings: u64,
}

impl<'a> Report<'a> {
    pub fn new(ctx: &'a Context) -> Report<'a> {
        Report {
            lint_results: vec![],
            ctx: ctx,
            errors: 0,
            warnings: 0,
        }
    }

    pub fn add_match_lint_result(&mut self, tool: &Tool, matches: &HashMap<Cow<str>, Cow<str>>)
        -> Result<&LintResult>
    {
        let f = match matches.get("filename") {
            Some(f) => Some(self.ctx.base_dir().join(&f as &str).canonicalize()?),
            None => None,
        };
        self.push_result(LintResult {
            filename: f,
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
        })
    }

    pub fn add_lint_result(&mut self, tool: &Tool, mut res: LintResult)
        -> Result<&LintResult>
    {
        if let Some(filename) = res.filename {
            res.filename = Some(self.ctx.base_dir().join(&filename).canonicalize()?);
        }
        res.code = res.code.map(|code| format!("{}:{}", tool.id(), code));
        self.push_result(res)
    }

    pub fn get_checkstyle_doc(&self) -> Element {
        let mut rv = Element::new("checkstyle");
        rv.set_attr("version", "4.3");

        let mut files = HashMap::new();
        for res in &self.lint_results {
            files.entry(&res.filename)
                .or_insert_with(|| vec![])
                .push(res);
        }

        for (file, results) in files {
            let name = match *file {
                Some(ref filename) => filename.display().to_string(),
                None => "<no file>".to_string(),
            };
            let element = rv.append_new_child("file")
                .set_attr("name", name);
            for res in results {
                element.append_new_child("error")
                    .set_attr("severity", match res.level {
                        Level::Error => "error",
                        Level::Warning => "warning",
                        Level::Info => "info",
                    })
                    .set_attr("line", res.line.to_string())
                    .set_attr("column", res.column.to_string())
                    .set_attr("source", res.code
                        .as_ref()
                        .map(|x| x.as_str())
                        .unwrap_or("unknown")
                        .replace(":", "."))
                    .set_attr("message", res.message
                        .as_ref()
                        .map(|x| x.as_str())
                        .unwrap_or(""));
            }
        }

        rv
    }

    fn push_result(&mut self, res: LintResult) -> Result<&LintResult> {
        let idx = self.lint_results.len();
        match res.level {
            Level::Error => { self.errors += 1; }
            Level::Warning => { self.warnings += 1; }
            _ => {}
        }
        self.lint_results.push(res);
        Ok(&self.lint_results[idx])
    }

    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    pub fn error_count(&self) -> u64 {
        self.errors
    }

    pub fn warnings_count(&self) -> u64 {
        self.warnings
    }

    pub fn sort(&mut self) {
        self.lint_results.sort();
    }

    pub fn print(&self, format: Format) -> Result<()> {
        if self.lint_results.is_empty() {
            return Ok(());
        }

        match format {
            Format::Human => {
                for res in &self.lint_results {
                    println!("{:#}", res);
                }

                let style = if self.has_errors() {
                    Style::new().bold().red()
                } else {
                    Style::new().bold().yellow()
                };

                println!("");
                println!("{}", style.apply_to(format!(
                    "Lint finished with {} error{} and {} warning{}.",
                    self.error_count(),
                    if self.error_count() != 1 { "s" } else { "" },
                    self.warnings_count(),
                    if self.warnings_count() != 1 { "s" } else { "" }
                )));
            }
            Format::Simple => {
                for res in &self.lint_results {
                    println!("{}", res.simple_format());
                }
            }
            Format::Checkstyle => {
                let doc = self.get_checkstyle_doc();
                doc.to_writer(&mut io::stdout())?;
            }
        }
        Ok(())
    }
}
