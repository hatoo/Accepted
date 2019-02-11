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

//

pub mod keys {
    use crate::config::types::Command;
    use crate::config::types::CompilerConfig;
    use std::collections::BTreeMap;
    use typemap::Key;

    pub struct ANSIColor;
    impl Key for ANSIColor {
        type Value = bool;
    }

    pub struct Snippets;
    impl Key for Snippets {
        type Value = BTreeMap<String, String>;
    }

    pub struct IndentWidth;
    impl Key for IndentWidth {
        type Value = usize;
    }

    pub struct LSP;
    impl Key for LSP {
        type Value = Command;
    }

    pub struct Formatter;
    impl Key for Formatter {
        type Value = Command;
    }

    pub struct SyntaxExtension;
    impl Key for SyntaxExtension {
        type Value = String;
    }

    pub struct Compiler;
    impl Key for Compiler {
        type Value = CompilerConfig;
    }

}
