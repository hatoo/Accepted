use crate::core::CoreBuffer;
use anyhow::Context;
use lsp_types::{CompletionItemKind, Documentation};
use serde_derive::{Deserialize, Serialize};
use std::process;
use tokio::prelude::*;
use tokio::stream::StreamExt;

pub struct TabNineClient {
    _proc: tokio::process::Child,
    results_receiver: tokio::sync::mpsc::UnboundedReceiver<AutocompleteResponse>,
    args_sender: tokio::sync::mpsc::UnboundedSender<AutocompleteArgs>,
}

pub struct TabNineCompletion {
    pub keyword: String,
    pub doc: String,
    pub old_prefix: String,
}

impl TabNineClient {
    pub fn new(command: std::process::Command) -> anyhow::Result<Self> {
        let mut proc: tokio::process::Child = tokio::process::Command::from(command)
            .stdin(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = proc.stdout.take().context("get stdout")?;
        let mut stdin = proc.stdin.take().context("get stdin")?;

        let (args_sender, mut args_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (results_sender, results_receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(args) = args_receiver.recv().await {
                let req = Request {
                    version: "1.0.0".to_string(),
                    request: AutoComplete { autocomplete: args },
                };
                if let Ok(json) = serde_json::to_string(&req) {
                    let json = json.replace('\n', "") + "\n";
                    let _ = stdin.write_all(json.as_bytes()).await;
                    let _ = stdin.flush().await;
                }
            }
        });

        tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Some(Ok(line)) = lines.next().await {
                if let Ok(res) = serde_json::from_str::<AutocompleteResponse>(line.as_str()) {
                    if results_sender.send(res).is_err() {
                        return;
                    }
                }
            }
        });

        Ok(TabNineClient {
            _proc : proc,
            results_receiver,
            args_sender,
        })
    }

    pub fn poll(&mut self) -> Option<Vec<TabNineCompletion>> {
        let mut ret = None;
        while let Ok(res) = self.results_receiver.try_recv() {
            let old_prefix = res.old_prefix.clone();
            ret = Some(
                res.results
                    .into_iter()
                    .map(|mut entry| TabNineCompletion {
                        keyword: entry.new_prefix.clone(),
                        doc: entry
                            .documentation
                            .take()
                            .map(|d| match d {
                                Documentation::String(s) => s,
                                Documentation::MarkupContent(m) => m.value,
                            })
                            .unwrap_or_else(|| "TabNine".to_string()),
                        old_prefix: old_prefix.clone(),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        ret
    }

    pub fn request_completion<B: CoreBuffer>(&self, buf: &crate::Buffer<B>) {
        let before = buf.core.core_buffer().get_range(..buf.core.cursor());
        let after = buf.core.core_buffer().get_range(buf.core.cursor()..);

        let args = AutocompleteArgs {
            before,
            after,
            filename: buf.path().map(|p| p.to_string_lossy().into_owned()),
            region_includes_beginning: true,
            region_includes_end: true,
            max_num_results: None,
        };
        let _ = self.args_sender.send(args);
    }
}

#[derive(Serialize, Deserialize)]
struct Request {
    version: String,
    request: AutoComplete,
}

#[derive(Serialize, Deserialize)]
struct AutoComplete {
    #[serde(rename = "Autocomplete")]
    autocomplete: AutocompleteArgs,
}

#[derive(Serialize, Deserialize, Debug)]
struct AutocompleteArgs {
    before: String,
    after: String,
    filename: Option<String>,
    region_includes_beginning: bool,
    region_includes_end: bool,
    max_num_results: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct AutocompleteResponse {
    old_prefix: String,
    results: Vec<ResultEntry>,
}

#[derive(Serialize, Deserialize)]
struct ResultEntry {
    new_prefix: String,
    old_suffix: String,
    new_suffix: String,

    kind: Option<CompletionItemKind>,
    detail: Option<String>,
    documentation: Option<Documentation>,
    deprecated: Option<bool>,
}
