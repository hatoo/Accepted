use serde_derive::Deserialize;
use std::ffi::OsString;
use std::process;

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
