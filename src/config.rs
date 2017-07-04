use std::fs;
use std::env;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::collections::hash_map::Keys as HashMapKeys;

use prelude::*;
use utils::serde::{Pattern, LinkSpec};

use sha1::Sha1;
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
#[serde(untagged)]
pub enum RemoteToolInclude {
    Git {
        git: String,
        rev: Option<String>,
        path: Option<String>,
    },
    Path {
        path: PathBuf,
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolSpec {
    #[serde(skip)]
    pub tool_dir_base: Option<PathBuf>,
    pub include: Option<RemoteToolInclude>,
    pub description: Option<String>,
    #[serde(default)]
    pub runtimes: HashMap<String, RuntimeConfig>,
    #[serde(rename="install", default)]
    pub install_steps: Vec<ToolStep>,
    pub lint: Option<LintSpec>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StandaloneToolConfig {
    tool: ToolSpec,
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
    cache_dir: PathBuf,
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

fn merge_tool_config(tool: &mut ToolSpec, config_dir: &Path, cache_dir: &Path) -> Result<()> {
    tool.tool_dir_base = Some(tool.include
        .as_ref().unwrap().local_path_reference(config_dir, cache_dir));
    let mut tool_config = tool.tool_dir_base.as_ref().unwrap().to_path_buf();
    if let Some(prefix) = tool.tool_dir_prefix() {
        tool_config.push(prefix);
    }
    tool_config.push("calmtool.yml");

    if fs::metadata(&tool_config).is_ok() {
        let mut f = fs::File::open(&tool_config)?;
        let rt: StandaloneToolConfig = serde_yaml::from_reader(&mut f)
            .chain_err(|| "Failed to parse calmtool.yml")?;

        if let Some(val) = rt.tool.description {
            tool.description = Some(val);
        }
        for (id, rtc) in rt.tool.runtimes.into_iter() {
            tool.runtimes.insert(id, rtc);
        }
        for ts in rt.tool.install_steps.into_iter() {
            tool.install_steps.push(ts);
        }
        if let Some(val) = rt.tool.lint {
            tool.lint = Some(val);
        }
    }

    Ok(())
}

impl Config {
    pub fn from_env() -> Result<Config> {
        let filename = find_config_file()?;
        let config_dir = filename.parent().unwrap().to_path_buf();

        let mut f = fs::File::open(&filename)
            .chain_err(|| "Could not open .calm/calm.yml")?;
        let mut rv: ConfigValues = serde_yaml::from_reader(&mut f)
            .chain_err(|| "Failed to parse .calm/calm.yml")?;

        let mut sha = Sha1::new();
        sha.update(filename.to_string_lossy().as_bytes());
        let mut cache_dir = env::home_dir().ok_or(
            Error::from("could not find home folder"))?;
        cache_dir.push(".calm");
        cache_dir.push("env-cache");
        cache_dir.push(sha.digest().to_string());

        // resolve includes and fail silently
        for mut tool in rv.tools.values_mut() {
            if tool.include.is_some() {
                merge_tool_config(&mut tool, &config_dir, &cache_dir)?;
            }
        }

        Ok(Config {
            filename: filename,
            config_dir: config_dir,
            cache_dir: cache_dir,
            values: rv,
        })
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn iter_tools(&self) -> HashMapKeys<String, ToolSpec> {
        self.values.tools.keys()
    }

    pub fn get_tool_spec(&self, id: &str) -> Option<&ToolSpec> {
        self.values.tools.get(id)
    }
}

impl ToolSpec {
    pub fn tool_dir_prefix<'a>(&'a self) -> Option<Cow<'a, Path>> {
        if let Some(ref tool_dir) = self.tool_dir_base {
            Some(if_chain! {
                if let Some(ref include) = self.include;
                if let Some(ref prefix) = include.path_prefix();
                then {
                    Cow::Owned(tool_dir.join(prefix))
                } else {
                    Cow::Borrowed(tool_dir)
                }
            })
        } else {
            None
        }
    }
}

impl RemoteToolInclude {

    pub fn path_prefix(&self) -> Option<&Path> {
        match *self {
            RemoteToolInclude::Git { ref path, .. } => {
                if let &Some(ref path) = path {
                    let path = Path::new(path);
                    if let Ok(rest) = path.strip_prefix("/") {
                        return Some(rest);
                    } else {
                        return Some(path);
                    }
                }
            }
            RemoteToolInclude::Path { .. } => {
                return None;
            }
        }
        None
    }

    pub fn local_path_reference<'a>(&'a self, config_dir: &Path,
                                    cache_dir: &Path) -> PathBuf {
        match *self {
            RemoteToolInclude::Git { .. } => {
                cache_dir.join("tools").join(self.checksum())
            }
            RemoteToolInclude::Path { ref path } => {
                config_dir.join(path)
            }
        }
    }

    pub fn checksum(&self) -> String {
        let mut m = Sha1::new();
        match *self {
            RemoteToolInclude::Git { ref git, ref rev, .. } => {
                m.update(git.as_bytes());
                m.update(b"\x00");
                if let &Some(ref rev) = rev {
                    m.update(rev.as_bytes());
                    m.update(b"\x00");
                }
            }
            RemoteToolInclude::Path { ref path } => {
                m.update(path.display().to_string().as_bytes());
                m.update(b"\x00");
            }
        }
        m.digest().to_string()
    }
}
