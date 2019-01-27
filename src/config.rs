use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::path;
use std::process;

use serde_derive::{Deserialize, Serialize};

type ConfigToml = HashMap<String, ConfigElementToml>;

const DEFAULT_CONFIG: &str = include_str!("../assets/default_config.toml");

#[derive(Serialize, Deserialize, Debug)]
struct SnippetSetJson(HashMap<String, SnippetJson>);

#[derive(Serialize, Deserialize, Debug)]
struct SnippetJson {
    prefix: String,
    body: Vec<String>,
}

pub type Snippets = BTreeMap<String, String>;

#[derive(Deserialize, Debug)]
struct ConfigElementToml {
    snippets: Option<Vec<String>>,
    indent_width: Option<usize>,
    lsp: Option<Vec<String>>,
    formatter: Option<Vec<String>>,
    syntax_extension: Option<String>,
}

pub struct LanguageConfig {
    snippets: Snippets,
    indent_width: Option<usize>,
    lsp: Option<Command>,
    formatter: Option<Command>,
    syntax_extension: Option<String>,
}

pub struct Command {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl Command {
    pub fn command(&self) -> process::Command {
        let mut res = process::Command::new(&self.program);
        res.args(self.args.iter());
        res
    }
}

#[derive(Default)]
struct Config(HashMap<OsString, LanguageConfig>);
pub struct ConfigWithDefault {
    default: Config,
    config: Config,
}

fn load_snippet<P: AsRef<path::Path>>(path: P) -> Result<Snippets, failure::Error> {
    let snippet_set: SnippetSetJson = serde_json::from_reader(fs::File::open(path)?)?;
    let mut snippets = Snippets::new();

    for (_, snippet) in snippet_set.0 {
        let mut buf = String::new();
        for line in &snippet.body {
            buf.push_str(line);
            buf.push('\n');
        }
        snippets.insert(snippet.prefix, buf);
    }

    Ok(snippets)
}

fn to_command(args: &[String]) -> Option<Command> {
    args.split_first().map(|(fst, rest)| Command {
        program: OsString::from(fst),
        args: rest.iter().map(OsString::from).collect(),
    })
}

fn to_language_config(toml: ConfigElementToml) -> LanguageConfig {
    let snippets = toml
        .snippets
        .unwrap_or_default()
        .iter()
        .map(|osstr| path::PathBuf::from(osstr))
        .filter_map(|p| load_snippet(p).ok())
        .fold(Snippets::new(), |mut a, mut b| {
            a.append(&mut b);
            a
        });

    LanguageConfig {
        snippets,
        indent_width: toml.indent_width,
        lsp: toml.lsp.as_ref().map(Vec::as_slice).and_then(to_command),
        formatter: toml
            .formatter
            .as_ref()
            .map(Vec::as_slice)
            .and_then(to_command),
        syntax_extension: toml.syntax_extension,
    }
}

fn parse_config(s: &str) -> Result<Config, failure::Error> {
    let config_toml: ConfigToml = toml::from_str(&s)?;

    Ok(Config(
        config_toml
            .into_iter()
            .map(|(k, v)| (OsString::from(k), to_language_config(v)))
            .collect(),
    ))
}

pub fn parse_config_with_default(s: &str) -> Result<ConfigWithDefault, failure::Error> {
    let default = toml::from_str(DEFAULT_CONFIG)
        .map(|config_toml: ConfigToml| {
            Config(
                config_toml
                    .into_iter()
                    .map(|(k, v)| (OsString::from(k), to_language_config(v)))
                    .collect(),
            )
        })
        .unwrap();

    let config = parse_config(s)?;

    Ok(ConfigWithDefault { default, config })
}

impl Default for ConfigWithDefault {
    fn default() -> Self {
        let default = toml::from_str(DEFAULT_CONFIG)
            .map(|config_toml: ConfigToml| {
                Config(
                    config_toml
                        .into_iter()
                        .map(|(k, v)| (OsString::from(k), to_language_config(v)))
                        .collect(),
                )
            })
            .unwrap();

        Self {
            default,
            config: Config::default(),
        }
    }
}

impl Config {
    const DEFAULT_KEY: &'static str = "default";

    fn indent_width(&self, extension: Option<&OsStr>) -> Option<usize> {
        if let Some(extension) = extension {
            self.0.get(extension).and_then(|c| c.indent_width).or(self
                .0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.indent_width))
        } else {
            self.0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.indent_width)
        }
    }

    fn lsp(&self, extension: Option<&OsStr>) -> Option<&Command> {
        if let Some(extension) = extension {
            self.0.get(extension).and_then(|c| c.lsp.as_ref()).or(self
                .0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.lsp.as_ref()))
        } else {
            self.0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.lsp.as_ref())
        }
    }

    fn formatter(&self, extension: Option<&OsStr>) -> Option<&Command> {
        if let Some(extension) = extension {
            self.0
                .get(extension)
                .and_then(|c| c.formatter.as_ref())
                .or(self
                    .0
                    .get(&OsString::from(Self::DEFAULT_KEY))
                    .and_then(|c| c.formatter.as_ref()))
        } else {
            self.0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.formatter.as_ref())
        }
    }

    fn snippets(&self, extension: Option<&OsStr>) -> Snippets {
        if let Some(extension) = extension {
            let mut snippets = self
                .0
                .get(extension)
                .map(|c| c.snippets.clone())
                .unwrap_or_default();
            let mut default_snippets = self
                .0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .map(|c| c.snippets.clone())
                .unwrap_or_default();

            snippets.append(&mut default_snippets);
            snippets
        } else {
            self.0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .map(|c| c.snippets.clone())
                .unwrap_or_default()
        }
    }

    pub fn syntax_extension(&self, extension: Option<&OsStr>) -> Option<&str> {
        if let Some(extension) = extension {
            self.0
                .get(extension)
                .and_then(|c| c.syntax_extension.as_ref().map(|s| s.as_str()))
                .or(self
                    .0
                    .get(&OsString::from(Self::DEFAULT_KEY))
                    .and_then(|c| c.syntax_extension.as_ref().map(|s| s.as_str())))
        } else {
            self.0
                .get(&OsString::from(Self::DEFAULT_KEY))
                .and_then(|c| c.syntax_extension.as_ref().map(|s| s.as_str()))
        }
    }
}

impl ConfigWithDefault {
    // Always provide index_width
    pub fn indent_width(&self, extension: Option<&OsStr>) -> usize {
        self.config
            .indent_width(extension)
            .unwrap_or_else(|| self.default.indent_width(extension).unwrap())
    }

    pub fn lsp(&self, extension: Option<&OsStr>) -> Option<&Command> {
        self.config
            .lsp(extension)
            .or_else(|| self.default.lsp(extension))
    }

    pub fn formatter(&self, extension: Option<&OsStr>) -> Option<&Command> {
        self.config
            .formatter(extension)
            .or_else(|| self.default.formatter(extension))
    }

    pub fn snippets(&self, extension: Option<&OsStr>) -> Snippets {
        self.config.snippets(extension)
    }

    pub fn syntax_extension(&self, extension: Option<&OsStr>) -> Option<&str> {
        self.config
            .syntax_extension(extension)
            .or_else(|| self.default.syntax_extension(extension))
    }
}
