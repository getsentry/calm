use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::collections::HashMap;
use std::borrow::Cow;

use serde::{Deserialize, de};
use regex::Regex;
use glob;

use utils::cmd::expand_vars;

lazy_static! {
    static ref REGEX_RE: Regex = Regex::new(
        r#"(?sm)^\s*/(.*)/([a-z]*)\s*$"#).unwrap();
    static ref LINK_RE: Regex = Regex::new(
        r#"^(.+?)(?:\s+->\s+(.+?))?$"#).unwrap();
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Regex(Regex),
    Glob(glob::Pattern),
}

#[derive(Debug, Clone)]
pub struct LinkSpec {
    pub src: PathBuf,
    pub dst: Option<PathBuf>,
}

impl LinkSpec {
    pub fn src<'a>(&'a self, vars: Option<&HashMap<String, String>>) -> Cow<'a, Path> {
        self.expand_path(&self.src, vars)
    }

    pub fn dst<'a>(&'a self, vars: Option<&HashMap<String, String>>) -> Cow<'a, Path> {
        if let Some(ref dst) = self.dst {
            self.expand_path(dst, vars)
        } else {
            self.src(vars)
        }
    }

    fn expand_path<'a>(&'a self, path: &'a Path, vars: Option<&HashMap<String, String>>) -> Cow<'a, Path> {
        path.to_str().map(|path| {
            match expand_vars(path, |key| {
                if_chain! {
                    if let Some(ref vars) = vars;
                    if let Some(val) = vars.get(key);
                    then {
                        val.to_string()
                    } else {
                        env::var(key).unwrap_or("".to_string())
                    }
                }
            }) {
                Cow::Borrowed(path) => Cow::Borrowed(Path::new(path)),
                Cow::Owned(path) => Cow::Owned(PathBuf::from(path)),
            }
        }).unwrap_or(Cow::Borrowed(path))
    }
}

impl<'a> Deserialize<'a> for Pattern {
    fn deserialize<D>(deserializer: D) -> StdResult<Pattern, D::Error>
        where D: de::Deserializer<'a> {
        struct PatternVisitor;

        impl<'b> de::Visitor<'b> for PatternVisitor {
            type Value = Pattern;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a pattern")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> StdResult<Pattern, E> {
                if value.is_empty() {
                    Err(E::custom("Empty pattern"))

                // regex
                } else if let Some(caps) = REGEX_RE.captures(value) {
                    let mut regex = String::new();
                    if &caps[2] != "" {
                        regex.push_str(&format!("(?{})", &caps[2]));
                    }
                    regex.push_str(&caps[1]);
                    Regex::new(&regex)
                        .map_err(|e| E::custom(e.to_string()))
                        .map(|re| Pattern::Regex(re))

                // glob
                } else {
                    glob::Pattern::new(value)
                        .map_err(|e| E::custom(e.to_string()))
                        .map(|pat| Pattern::Glob(pat))
                }

            }
        }

        deserializer.deserialize_str(PatternVisitor)
    }
}

impl Pattern {
    pub fn match_path<P: AsRef<Path>>(&self, p: P) -> bool {
        match *self {
            Pattern::Glob(ref pattern) => {
                pattern.matches_path(p.as_ref())
            }
            Pattern::Regex(ref pattern) => {
                if let Some(s) = p.as_ref().to_str() {
                    pattern.is_match(s)
                } else {
                    false
                }
            }
        }
    }

    pub fn match_str<'a, 'b>(&'a self, s: &'b str)
        -> Option<HashMap<Cow<'a, str>, Cow<'b, str>>>
    {
        match *self {
            Pattern::Glob(ref pattern) => {
                if pattern.matches(s) {
                    Some(HashMap::new())
                } else {
                    None
                }
            }
            Pattern::Regex(ref regex) => {
                regex.captures(s).map(|caps| {
                    let mut rv = HashMap::new();
                    for name in regex.capture_names() {
                        if let Some(name) = name {
                            rv.insert(
                                Cow::Borrowed(name),
                                Cow::Borrowed(
                                    caps.name(name).map(|x| x.as_str()).unwrap_or("")));
                        }
                    }
                    rv
                })
            }
        }
    }
}

impl<'a> Deserialize<'a> for LinkSpec {
    fn deserialize<D>(deserializer: D) -> StdResult<LinkSpec, D::Error>
        where D: de::Deserializer<'a> {
        struct LinkSpecVisitor;

        impl<'b> de::Visitor<'b> for LinkSpecVisitor {
            type Value = LinkSpec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a link specification")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> StdResult<LinkSpec, E> {
                if value.is_empty() {
                    Err(E::custom("Empty link specification"))

                // regex
                } else if let Some(caps) = LINK_RE.captures(value) {
                    Ok(LinkSpec {
                        src: PathBuf::from(&caps[1]),
                        dst: caps.get(2).map(|m| PathBuf::from(m.as_str())),
                    })
                } else {
                    Err(E::custom("Bad link specification"))
                }
            }
        }

        deserializer.deserialize_str(LinkSpecVisitor)
    }
}
