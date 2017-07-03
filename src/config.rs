use std::fs;
use std::env;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::collections::hash_map::Keys as HashMapKeys;

use prelude::*;
use utils::serde::{Pattern, LinkSpec};

use serde_yaml;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ReportPatternMatch {
    #[serde(rename="lint-result")]
    LintResult,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ParseLinesAction {
    pub pattern: Pattern,
    #[serde(rename="report-match")]
    pub report_match: ReportPatternMatch,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StreamActions {
    #[serde(rename="parse-lines")]
    pub parse_lines: Option<ParseLinesAction>,
    #[serde(rename="parse-lint-json", default)]
    pub parse_lint_json: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ToolCommand {
    Shell(String),
    Exec(Vec<String>),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ToolStep {
    Command {
        description: Option<String>,
        cmd: ToolCommand,
        stdout: Option<StreamActions>,
        stderr: Option<StreamActions>,
    },
    Link {
        description: Option<String>,
        link: LinkSpec,
    }
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct LintSpec {
    pub patterns: Vec<Pattern>,
    pub run: Vec<ToolStep>,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct RuntimeConfig {
    /// some runtimes have different flavors that can be selected.
    flavor: Option<String>,
    /// packages to install.
    #[serde(default)]
    packages: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolSpec {
    #[serde(skip)]
    pub tool_dir: Option<PathBuf>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub runtimes: HashMap<String, RuntimeConfig>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(rename="install", default)]
    pub install_steps: Vec<ToolStep>,
    pub lint: Option<LintSpec>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Rule {
    patterns: Vec<String>,
    run: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigValues {
    #[serde(default)]
    tools: HashMap<String, ToolSpec>,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    filename: PathBuf,
    config_dir: PathBuf,
    values: ConfigValues,
}

fn find_config_file() -> Result<PathBuf> {
    if let Ok(mut path) = env::current_dir() {
        loop {
            path.push(".calm/calm.yml");
            if path.exists() {
                return Ok(path);
            }
            path.pop();
            path.pop();
            if !path.pop() {
                break;
            }
        }
    }
    Err(Error::from("Could not find .calm.yml"))
}

impl RuntimeConfig {
    pub fn flavor(&self) -> Option<&str> {
        self.flavor.as_ref().map(|x| x.as_str())
    }

    pub fn packages(&self) -> &HashMap<String, String> {
        &self.packages
    }
}

impl ToolStep {
    pub fn description(&self) -> String {
        match *self {
            ToolStep::Command { ref description, .. } => {
                if let &Some(ref desc) = description {
                    desc.to_string()
                } else {
                    format!("Running {}", self.cmd_name())
                }
            }
            ToolStep::Link { ref description, ref link, .. } => {
                if let &Some(ref desc) = description {
                    desc.to_string()
                } else {
                    format!("Linking {}", link.src.display())
                }
            }
        }
    }

    pub fn cmd_name(&self) -> &str {
        match *self {
            ToolStep::Command { ref cmd, .. } => {
                match cmd {
                    &ToolCommand::Shell(ref cmd) => cmd.split_whitespace().next(),
                    &ToolCommand::Exec(ref cmd) => cmd.get(0).map(|x| x.as_str()),
                }.unwrap_or("command")
            }
            _ => ""
        }
    }

    pub fn cmd(&self) -> Option<&ToolCommand> {
        match *self {
            ToolStep::Command { ref cmd, .. } => Some(cmd),
            _ => None,
        }
    }

    pub fn link(&self) -> Option<&LinkSpec> {
        match *self {
            ToolStep::Link { ref link, .. } => Some(link),
            _ => None,
        }
    }

    pub fn stdout_actions(&self) -> Option<&StreamActions> {
        match *self {
            ToolStep::Command { ref stdout, .. } => stdout.as_ref(),
            _ => None,
        }
    }

    pub fn stderr_actions(&self) -> Option<&StreamActions> {
        match *self {
            ToolStep::Command { ref stderr, .. } => stderr.as_ref(),
            _ => None,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Config> {
        let filename = find_config_file()?;
        let mut f = fs::File::open(&filename)
            .chain_err(|| "Could not open .calm/calm.yml")?;
        let mut rv: ConfigValues = serde_yaml::from_reader(&mut f)
            .chain_err(|| "Failed to parse .calm/calm.yml")?;

        // currently all tools come from the main .calm folder so this
        // is what we set as tool directory.  Once we permit tools to
        // be coming from external sources this will change.
        let config_dir = filename.parent().unwrap().to_path_buf();
        for mut tool in rv.tools.values_mut() {
            tool.tool_dir = Some(config_dir.clone());
        }

        Ok(Config {
            filename: filename,
            config_dir: config_dir,
            values: rv,
        })
    }

    pub fn filename(&self) -> &Path {
        &self.filename
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn iter_tools(&self) -> HashMapKeys<String, ToolSpec> {
        self.values.tools.keys()
    }

    pub fn get_tool_spec(&self, id: &str) -> Option<&ToolSpec> {
        self.values.tools.get(id)
    }
}
