use serde_derive::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path;

mod snippet;
pub mod types;

use crate::config::snippet::load_snippet;
use crate::config::types::Command;
use crate::config::types::CompilerConfig;

const DEFAULT_CONFIG: &str = include_str!("../../assets/default_config.toml");

#[derive(Deserialize, Debug)]
struct ConfigToml {
    file: HashMap<String, LanguageConfigToml>,
    file_default: Option<LanguageConfigToml>,
}

#[derive(Deserialize, Debug)]
struct LanguageConfigToml {
    ansi_color: Option<bool>,
    snippets: Option<Vec<String>>,
    indent_width: Option<usize>,
    lsp: Option<Vec<String>>,
    formatter: Option<Vec<String>>,
    syntax: Option<String>,
    compiler: Option<CompilerConfig>,
}

pub struct LanguageConfig(typemap::TypeMap);

impl Default for LanguageConfig {
    fn default() -> Self {
        Self(typemap::TypeMap::new())
    }
}

impl LanguageConfig {
    fn insert_option<A>(&mut self, value: Option<A>)
    {
        if let Some(value) = value {
            self.0.insert(value);
        }
    }
}

#[derive(Default)]
struct Config {
    file: HashMap<OsString, LanguageConfig>,
    file_default: Option<LanguageConfig>,
}

pub struct ConfigWithDefault {
    default: Config,
    config: Config,
}

impl Into<LanguageConfig> for LanguageConfigToml {
    fn into(self) -> LanguageConfig {
        let snippets = self
            .snippets
            .unwrap_or_default()
            .iter()
            .map(path::PathBuf::from)
            .filter_map(|p| load_snippet(p).ok())
            .fold(BTreeMap::new(), |mut a, mut b| {
                a.append(&mut b);
                a
            });

        let mut language_config = LanguageConfig::default();

        language_config.insert_option(self.ansi_color.map(types::ANSIColor));
        language_config.0.insert(types::Snippets(snippets));
        language_config.insert_option(self.indent_width.map(types::IndentWidth));
        language_config.insert_option(
            self.lsp
                .as_ref()
                .map(Vec::as_slice)
                .and_then(Command::new)
                .map(types::LSP),
        );
        language_config.insert_option(
            self.formatter
                .as_ref()
                .map(Vec::as_slice)
                .and_then(Command::new)
                .map(types::Formatter),
        );
        language_config.insert_option(self.syntax.map(types::SyntaxExtension));
        language_config.0.insert(self.compiler);

        language_config
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
    fn get<A>(&mut self, extension: Option<&OsStr>) -> Option<&A>
    {
        if let Some(extension) = extension {
            self.file
                .get(extension)
                .and_then(|config| config.0.get())
                .or_else(|| self.file_default.as_ref().and_then(|config| config.0.get()))
        } else {
            self.file_default.as_ref().and_then(|config| config.0.get())
        }
    }
}

impl ConfigWithDefault {
    fn get<A>(&mut self, extension: Option<&OsStr>) -> Option<&A>
    {
        self.config
            .get(extension)
            .or_else(|| self.default.get(extension))
    }
}
