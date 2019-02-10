use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io::BufReader;
use std::path;
use std::process;

use serde_derive::{Deserialize, Serialize};

const DEFAULT_CONFIG: &str = include_str!("../assets/default_config.toml");

#[derive(Deserialize, Debug)]
struct ConfigToml {
    file: HashMap<String, LanguageConfigToml>,
    #[serde(rename = "file-default")]
    file_default: Option<LanguageConfigToml>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SnippetSetJson(HashMap<String, SnippetJson>);

#[derive(Serialize, Deserialize, Debug)]
struct SnippetJson {
    prefix: String,
    body: Vec<String>,
}

pub type Snippets = BTreeMap<String, String>;

#[derive(Deserialize, Debug, Clone)]
pub enum CompilerType {
    #[serde(rename = "rustc")]
    Rustc,
    #[serde(rename = "gcc")]
    Gcc,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CompilerConfig {
    pub command: Vec<String>,
    pub optimize_option: Vec<String>,
    #[serde(rename = "type")]
    pub output_type: Option<CompilerType>,
}

#[derive(Deserialize, Debug)]
struct LanguageConfigToml {
    snippets: Option<Vec<String>>,
    indent_width: Option<usize>,
    lsp: Option<Vec<String>>,
    formatter: Option<Vec<String>>,
    syntax: Option<String>,
    compiler: Option<CompilerConfig>,
}

#[derive(Debug)]
pub struct LanguageConfig {
    snippets: Snippets,
    indent_width: Option<usize>,
    lsp: Option<Command>,
    formatter: Option<Command>,
    syntax_extension: Option<String>,
    compiler: Option<CompilerConfig>,
}

#[derive(Debug)]
pub struct Command {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl Command {
    pub fn new(args: &[String]) -> Option<Self> {
        args.split_first().map(|(fst, rest)| Self {
            program: OsString::from(fst),
            args: rest.iter().map(OsString::from).collect(),
        })
    }
    pub fn command(&self) -> process::Command {
        let mut res = process::Command::new(&self.program);
        res.args(self.args.iter());
        res
    }
}

#[derive(Default, Debug)]
struct Config {
    file: HashMap<OsString, LanguageConfig>,
    file_default: Option<LanguageConfig>,
}
#[derive(Debug)]
pub struct ConfigWithDefault {
    default: Config,
    config: Config,
}

fn load_snippet<P: AsRef<path::Path>>(path: P) -> Result<Snippets, failure::Error> {
    let snippet_set: SnippetSetJson =
        serde_json::from_reader(BufReader::new(fs::File::open(path)?))?;
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

impl Into<LanguageConfig> for LanguageConfigToml {
    fn into(self) -> LanguageConfig {
        let snippets = self
            .snippets
            .unwrap_or_default()
            .iter()
            .map(path::PathBuf::from)
            .filter_map(|p| load_snippet(p).ok())
            .fold(Snippets::new(), |mut a, mut b| {
                a.append(&mut b);
                a
            });

        LanguageConfig {
            snippets,
            indent_width: self.indent_width,
            lsp: self.lsp.as_ref().map(Vec::as_slice).and_then(Command::new),
            formatter: self
                .formatter
                .as_ref()
                .map(Vec::as_slice)
                .and_then(Command::new),
            syntax_extension: self.syntax,
            compiler: self.compiler,
        }
    }
}

impl Into<Config> for ConfigToml {
    fn into(self) -> Config {
        Config {
            file: self
                .file
                .into_iter()
                .map(|(k, v)| (OsString::from(k), v.into()))
                .collect(),
            file_default: self.file_default.map(Into::into),
        }
    }
}

fn parse_config(s: &str) -> Result<Config, failure::Error> {
    let config_toml: ConfigToml = toml::from_str(&s)?;
    Ok(config_toml.into())
}

pub fn parse_config_with_default(s: &str) -> Result<ConfigWithDefault, failure::Error> {
    let default = toml::from_str::<ConfigToml>(DEFAULT_CONFIG)
        .map(Into::into)
        .unwrap();

    let config = parse_config(s)?;

    Ok(ConfigWithDefault { default, config })
}

impl Default for ConfigWithDefault {
    fn default() -> Self {
        let default = toml::from_str::<ConfigToml>(DEFAULT_CONFIG)
            .map(Into::into)
            .unwrap();

        Self {
            default,
            config: Config::default(),
        }
    }
}

impl Config {
    fn get<'a, T, F: Fn(&'a LanguageConfig) -> Option<T>>(
        &'a self,
        extension: Option<&OsStr>,
        f: F,
    ) -> Option<T> {
        if let Some(extension) = extension {
            self.file
                .get(extension)
                .and_then(&f)
                .or_else(|| self.file_default.as_ref().and_then(&f))
        } else {
            self.file_default.as_ref().and_then(&f)
        }
    }

    fn indent_width(&self, extension: Option<&OsStr>) -> Option<usize> {
        self.get(extension, |l| l.indent_width)
    }

    fn lsp(&self, extension: Option<&OsStr>) -> Option<&Command> {
        self.get(extension, |l| l.lsp.as_ref())
    }

    fn formatter(&self, extension: Option<&OsStr>) -> Option<&Command> {
        self.get(extension, |l| l.formatter.as_ref())
    }

    fn compiler(&self, extension: Option<&OsStr>) -> Option<&CompilerConfig> {
        self.get(extension, |l| l.compiler.as_ref())
    }

    fn snippets(&self, extension: Option<&OsStr>) -> Snippets {
        if let Some(extension) = extension {
            let mut snippets = self
                .file
                .get(extension)
                .map(|c| c.snippets.clone())
                .unwrap_or_default();
            let mut default_snippets = self
                .file_default
                .as_ref()
                .map(|c| c.snippets.clone())
                .unwrap_or_default();

            snippets.append(&mut default_snippets);
            snippets
        } else {
            self.file_default
                .as_ref()
                .map(|c| c.snippets.clone())
                .unwrap_or_default()
        }
    }

    pub fn syntax_extension(&self, extension: Option<&OsStr>) -> Option<&str> {
        self.get(extension, |l| {
            l.syntax_extension.as_ref().map(String::as_str)
        })
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

    pub fn compiler(&self, extension: Option<&OsStr>) -> Option<&CompilerConfig> {
        self.config
            .compiler(extension)
            .or_else(|| self.default.compiler(extension))
    }

    pub fn snippets(&self, extension: Option<&OsStr>) -> Snippets {
        self.config.snippets(extension)
    }

    pub fn syntax_extension<'a>(&'a self, extension: Option<&'a OsStr>) -> Option<&'a str> {
        self.config
            .syntax_extension(extension)
            .or_else(|| self.default.syntax_extension(extension))
            .or_else(|| extension.and_then(|s| s.to_str()))
    }
}
