use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Result, Write};
use std::process;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use jsonrpc_core;
use jsonrpc_core::Output;
use languageserver_types;
use serde;
use serde_json;

use crate::core::Cursor;
use crate::mode::Completion;

pub struct LSPClient {
    process: process::Child,
    completion_req: Sender<(String, Cursor)>,
    completion_recv: Receiver<Vec<Completion>>,
}

impl Drop for LSPClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

const ID_INIT: u64 = 0;
const ID_COMPLETION: u64 = 1;

impl LSPClient {
    pub fn start(mut lsp_command: process::Command, extension: String) -> Option<Self> {
        let mut lsp = lsp_command
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()
            .ok()?;

        let init = languageserver_types::InitializeParams {
            process_id: Some(u64::from(process::id())),
            root_path: Some("./".to_string()),
            root_uri: None,
            initialization_options: None,
            capabilities: languageserver_types::ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
        };

        let mut stdin: process::ChildStdin = lsp.stdin.take()?;
        let mut reader = BufReader::new(lsp.stdout.take()?);

        send_request::<_, languageserver_types::request::Initialize>(&mut stdin, ID_INIT, init)
            .ok()?;

        let (init_tx, init_rx) = channel::<()>();
        let (tx, rx) = channel();

        let (c_tx, c_rx) = channel::<(String, Cursor)>();
        thread::spawn(move || {
            // Wait initialize
            let _ = init_rx.recv();
            let file_url =
                languageserver_types::Url::parse(&format!("file://localhost/main.{}", extension))
                    .unwrap();

            while let Ok((src, cursor)) = c_rx.recv() {
                let open = languageserver_types::DidOpenTextDocumentParams {
                    text_document: languageserver_types::TextDocumentItem {
                        uri: file_url.clone(),
                        language_id: extension.clone(),
                        version: 0,
                        text: src,
                    },
                };
                send_notify::<_, languageserver_types::notification::DidOpenTextDocument>(
                    &mut stdin, open,
                )
                .unwrap();
                let completion = languageserver_types::CompletionParams {
                    text_document: languageserver_types::TextDocumentIdentifier {
                        uri: file_url.clone(),
                    },
                    position: languageserver_types::Position {
                        line: cursor.row as u64,
                        character: cursor.col as u64,
                    },
                    context: None,
                };
                send_request::<_, languageserver_types::request::Completion>(
                    &mut stdin, 1, completion,
                )
                .unwrap();
            }
        });

        thread::spawn(move || {
            let mut headers = HashMap::new();
            loop {
                headers.clear();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap() == 0 {
                        return;
                    }
                    let header = header.trim();
                    if header.is_empty() {
                        break;
                    }
                    let parts: Vec<&str> = header.split(": ").collect();
                    if parts.len() != 2 {
                        return;
                    }
                    headers.insert(parts[0].to_string(), parts[1].to_string());
                }
                let content_len = headers["Content-Length"].parse().unwrap();
                let mut content = vec![0; content_len];
                reader.read_exact(&mut content).unwrap();
                let msg = String::from_utf8(content).unwrap();
                let output: serde_json::Result<Output> = serde_json::from_str(&msg);
                if let Ok(Output::Success(suc)) = output {
                    if suc.id == jsonrpc_core::id::Id::Num(ID_INIT) {
                        init_tx.send(()).unwrap();
                    } else if suc.id == jsonrpc_core::id::Id::Num(ID_COMPLETION) {
                        let completion = serde_json::from_value::<
                            languageserver_types::CompletionResponse,
                        >(suc.result)
                        .unwrap();

                        let completion = extract_completion(completion);
                        tx.send(completion).unwrap();
                    }
                }
            }
        });

        Some(Self {
            process: lsp,
            completion_recv: rx,
            completion_req: c_tx,
        })
    }

    pub fn request_completion(&self, src: String, cursor: Cursor) {
        let _ = self.completion_req.send((src, cursor));
    }

    pub fn poll(&self) -> Option<Vec<Completion>> {
        let mut res = None;
        while let Ok(completion) = self.completion_recv.try_recv() {
            res = Some(completion);
        }
        res
    }
}

fn send_request<T: Write, R: languageserver_types::request::Request>(
    t: &mut T,
    id: u64,
    params: R::Params,
) -> Result<()>
where
    R::Params: serde::Serialize,
{
    if let serde_json::value::Value::Object(params) = serde_json::to_value(params).unwrap() {
        let req = jsonrpc_core::Call::MethodCall(jsonrpc_core::MethodCall {
            jsonrpc: Some(jsonrpc_core::Version::V2),
            method: R::METHOD.to_string(),
            params: jsonrpc_core::Params::Map(params),
            id: jsonrpc_core::Id::Num(id),
        });
        let request = serde_json::to_string(&req).unwrap();
        write!(t, "Content-Length: {}\r\n\r\n{}", request.len(), request)
    } else {
        Ok(())
    }
}

fn send_notify<T: Write, R: languageserver_types::notification::Notification>(
    t: &mut T,
    params: R::Params,
) -> Result<()>
where
    R::Params: serde::Serialize,
{
    if let serde_json::value::Value::Object(params) = serde_json::to_value(params).unwrap() {
        let req = jsonrpc_core::Notification {
            jsonrpc: Some(jsonrpc_core::Version::V2),
            method: R::METHOD.to_string(),
            params: jsonrpc_core::Params::Map(params),
        };
        let request = serde_json::to_string(&req).unwrap();
        write!(t, "Content-Length: {}\r\n\r\n{}", request.len(), request)
    } else {
        Ok(())
    }
}

fn extract_completion(completion: languageserver_types::CompletionResponse) -> Vec<Completion> {
    match completion {
        languageserver_types::CompletionResponse::Array(array) => array
            .into_iter()
            .map(|item| Completion {
                keyword: item.label,
                doc: item.detail.unwrap_or_default(),
            })
            .collect(),
        languageserver_types::CompletionResponse::List(list) => list
            .items
            .into_iter()
            .map(|item| Completion {
                keyword: item.label,
                doc: item.detail.unwrap_or_default(),
            })
            .collect(),
    }
}
