use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;

use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
struct ConfigureElementToml {
    snippets: Vec<OsString>,
    indent_width: usize,
    lsp: Vec<OsString>,
    formatter: Option<OsString>,
}

#[derive(Deserialize, Debug)]
struct ConfigureToml(HashMap<String, ConfigureElementToml>);

#[derive(Serialize, Deserialize, Debug)]
struct SnippetSet(HashMap<String, Snippet>);

#[derive(Serialize, Deserialize, Debug)]
struct Snippet {
    prefix: String,
    body: Vec<String>,
}

struct Command {
    program: OsString,
    args: Vec<OsString>,
}

struct Configure {
    snippets: BTreeMap<String, String>,
    indent_width: usize,
    lsp: Option<Command>,
    formatter: Option<Command>,
}
