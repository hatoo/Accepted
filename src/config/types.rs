use std::fmt;
use std::process;

use serde_derive::Deserialize;

#[derive(Debug, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

impl Command {
    pub fn new(args: &[String]) -> Option<Self> {
        args.split_first().map(|(fst, rest)| Self {
            program: fst.clone(),
            args: rest.to_vec(),
        })
    }
    pub fn command(&self) -> process::Command {
        let mut res = process::Command::new(&self.program);
        res.args(self.args.iter());
        res
    }
    pub fn summary<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<String, shellexpand::LookupError<std::env::VarError>> {
        crate::env::set_env(path);
        let prog = shellexpand::full(&self.program)?;
        let args = self
            .args
            .iter()
            .map(shellexpand::full)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(format!("{} {}", prog, args.join(" ")))
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.program)?;
        for a in &self.args {
            write!(f, " {}", a)?;
        }
        Ok(())
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

pub mod keys {
    use std::collections::BTreeMap;

    use typemap::Key;

    use crate::config::types::Command;
    use crate::config::types::CompilerConfig;

    // TODO Those generate impls from macro

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

    pub struct TestCommand;
    impl Key for TestCommand {
        type Value = Command;
    }

    pub struct TabNineCommand;
    impl Key for TabNineCommand {
        type Value = Command;
    }

    pub struct Theme;
    impl Key for Theme {
        type Value = String;
    }

    pub struct HardTab;
    impl Key for HardTab {
        type Value = bool;
    }
}
