use crate::core::CoreBuffer;
use lsp_types::{CompletionItemKind, Documentation};
use serde_derive::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

pub struct TabNineClient {
    proc: process::Child,
    args_sender: Sender<AutocompleteArgs>,
    results_receiver: Receiver<AutocompleteResponse>,
}

pub struct TabNineCompletion {
    pub keyword: String,
    pub doc: String,
    pub old_prefix: String,
}

impl TabNineClient {
    pub fn new(mut command: process::Command) -> Result<Self, failure::Error> {
        let mut proc = command
            .stdin(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .spawn()?;

        let stdout = BufReader::new(proc.stdout.take().unwrap());
        let mut stdin = proc.stdin.take().unwrap();

        let (args_sender, args_receiver) = mpsc::channel::<AutocompleteArgs>();
        let (results_sender, results_receiver) = mpsc::channel::<AutocompleteResponse>();

        std::thread::spawn(move || -> std::io::Result<()> {
            for args in args_receiver {
                let req = Request {
                    version: "1.0.0".to_string(),
                    request: AutoComplete { autocomplete: args },
                };
                if let Ok(json) = serde_json::to_string(&req) {
                    let json = json.replace('\n', "");
                    writeln!(stdin, "{}", json)?;
                }
            }
            Ok(())
        });

        std::thread::spawn(move || {
            for line in stdout.lines() {
                if let Ok(line) = line {
                    if let Ok(result) = serde_json::from_str::<AutocompleteResponse>(line.as_str())
                    {
                        if results_sender.send(result).is_err() {
                            return;
                        }
                    }
                }
            }
        });

        Ok(TabNineClient {
            proc,
            args_sender,
            results_receiver,
        })
    }

    pub fn poll(&self) -> Option<Vec<TabNineCompletion>> {
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

        let req = AutocompleteArgs {
            before,
            after,
            filename: buf.path().map(|p| p.to_string_lossy().into_owned()),
            region_includes_beginning: true,
            region_includes_end: true,
            max_num_results: None,
        };

        let _ = self.args_sender.send(req);
    }
}

impl Drop for TabNineClient {
    fn drop(&mut self) {
        let _ = self.proc.kill();
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
