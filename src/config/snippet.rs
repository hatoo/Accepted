use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::BufReader;
use std::path;

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct SnippetSetJson(HashMap<String, SnippetJson>);

#[derive(Serialize, Deserialize, Debug)]
pub struct SnippetJson {
    prefix: String,
    body: Vec<String>,
}

pub fn load_snippet<P: AsRef<path::Path>>(path: P) -> anyhow::Result<BTreeMap<String, String>> {
    let snippet_set: SnippetSetJson =
        serde_json::from_reader(BufReader::new(fs::File::open(path)?))?;
    let mut snippets = BTreeMap::new();

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
