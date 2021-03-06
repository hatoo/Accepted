use std::collections::HashMap;
use std::io::Write;
use std::process;

use anyhow::Context;
use jsonrpc_core;
use jsonrpc_core::Output;
use lsp_types;
use serde;
use serde_json;
use tokio::prelude::*;

use crate::core::Cursor;

#[derive(Debug)]
pub struct LSPCompletion {
    pub keyword: String,
    pub doc: String,
}

pub struct LSPClient {
    process: tokio::process::Child,
    completion_req: tokio::sync::mpsc::UnboundedSender<(String, Cursor)>,
    completion_recv: tokio::sync::mpsc::UnboundedReceiver<Vec<LSPCompletion>>,
}

impl Drop for LSPClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

const ID_INIT: u64 = 0;
const ID_COMPLETION: u64 = 1;

impl LSPClient {
    pub fn start(lsp_command: process::Command, extension: String) -> anyhow::Result<Self> {
        let mut lsp = tokio::process::Command::from(lsp_command)
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        #[allow(deprecated)]
        let init = lsp_types::InitializeParams {
            process_id: Some(u64::from(process::id())),
            root_path: None,
            root_uri: Some(lsp_types::Url::parse("file://localhost/")?),
            initialization_options: None,
            capabilities: lsp_types::ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
        };

        let mut stdin = lsp.stdin.take().context("take stdin")?;
        let mut reader = tokio::io::BufReader::new(lsp.stdout.take().context("take stdout")?);

        // send_request::<_, lsp_types::request::Initialize>(&mut stdin, ID_INIT, init)?;

        let (init_tx, mut init_rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let (c_tx, mut c_rx) = tokio::sync::mpsc::unbounded_channel::<(String, Cursor)>();
        tokio::spawn(async move {
            send_request_async::<_, lsp_types::request::Initialize>(&mut stdin, ID_INIT, init)
                .await?;
            // Wait initialize
            init_rx.recv().await.unwrap();
            let file_url =
                lsp_types::Url::parse(&format!("file://localhost/main.{}", extension)).unwrap();

            while let Some((src, cursor)) = c_rx.recv().await {
                let open = lsp_types::DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri: file_url.clone(),
                        language_id: extension.clone(),
                        version: 0,
                        text: src,
                    },
                };
                send_notify_async::<_, lsp_types::notification::DidOpenTextDocument>(
                    &mut stdin, open,
                )
                .await?;
                let completion = lsp_types::CompletionParams {
                    text_document_position: lsp_types::TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier {
                            uri: file_url.clone(),
                        },
                        position: lsp_types::Position {
                            line: cursor.row as u64,
                            character: cursor.col as u64,
                        },
                    },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                    context: None,
                };
                send_request_async::<_, lsp_types::request::Completion>(&mut stdin, 1, completion)
                    .await?;
            }
            Ok::<(), anyhow::Error>(())
        });

        tokio::spawn(async move {
            let mut headers = HashMap::new();
            loop {
                headers.clear();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).await? == 0 {
                        return Ok::<(), anyhow::Error>(());
                    }
                    let header = header.trim();
                    if header.is_empty() {
                        break;
                    }
                    let parts: Vec<&str> = header.split(": ").collect();
                    assert_eq!(parts.len(), 2);
                    headers.insert(parts[0].to_string(), parts[1].to_string());
                }
                let content_len = headers["Content-Length"].parse()?;
                let mut content = vec![0; content_len];
                reader.read_exact(&mut content).await?;
                let msg = String::from_utf8(content)?;
                let output: serde_json::Result<Output> = serde_json::from_str(&msg);
                if let Ok(Output::Success(suc)) = output {
                    if suc.id == jsonrpc_core::id::Id::Num(ID_INIT) {
                        init_tx.send(())?;
                    } else if suc.id == jsonrpc_core::id::Id::Num(ID_COMPLETION) {
                        let completion =
                            serde_json::from_value::<lsp_types::CompletionResponse>(suc.result)?;

                        let completion = extract_completion(completion);
                        tx.send(completion)?;
                    }
                }
            }
        });

        Ok(Self {
            process: lsp,
            completion_recv: rx,
            completion_req: c_tx,
        })
    }

    pub fn request_completion(&self, src: String, cursor: Cursor) {
        let _ = self.completion_req.send((src, cursor));
    }

    pub fn poll(&mut self) -> Option<Vec<LSPCompletion>> {
        let mut res = None;
        while let Ok(completion) = self.completion_recv.try_recv() {
            res = Some(completion);
        }
        res
    }
}

async fn send_request_async<T: AsyncWrite + std::marker::Unpin, R: lsp_types::request::Request>(
    t: &mut T,
    id: u64,
    params: R::Params,
) -> anyhow::Result<()>
where
    R::Params: serde::Serialize,
{
    if let serde_json::value::Value::Object(params) = serde_json::to_value(params)? {
        let req = jsonrpc_core::Call::MethodCall(jsonrpc_core::MethodCall {
            jsonrpc: Some(jsonrpc_core::Version::V2),
            method: R::METHOD.to_string(),
            params: jsonrpc_core::Params::Map(params),
            id: jsonrpc_core::Id::Num(id),
        });
        let request = serde_json::to_string(&req)?;
        let mut buffer: Vec<u8> = Vec::new();
        write!(
            &mut buffer,
            "Content-Length: {}\r\n\r\n{}",
            request.len(),
            request
        )?;
        t.write_all(&buffer).await?;
        Ok(())
    } else {
        anyhow::bail!("Invalid params");
    }
}

async fn send_notify_async<
    T: AsyncWrite + std::marker::Unpin,
    R: lsp_types::notification::Notification,
>(
    t: &mut T,
    params: R::Params,
) -> anyhow::Result<()>
where
    R::Params: serde::Serialize,
{
    if let serde_json::value::Value::Object(params) = serde_json::to_value(params)? {
        let req = jsonrpc_core::Notification {
            jsonrpc: Some(jsonrpc_core::Version::V2),
            method: R::METHOD.to_string(),
            params: jsonrpc_core::Params::Map(params),
        };
        let request = serde_json::to_string(&req)?;
        let mut buf: Vec<u8> = Vec::new();
        write!(
            &mut buf,
            "Content-Length: {}\r\n\r\n{}",
            request.len(),
            request
        )?;
        t.write_all(&buf).await?;
        Ok(())
    } else {
        anyhow::bail!("Invalid params")
    }
}

fn extract_completion(completion: lsp_types::CompletionResponse) -> Vec<LSPCompletion> {
    match completion {
        lsp_types::CompletionResponse::Array(array) => array
            .into_iter()
            .map(|item| LSPCompletion {
                keyword: item.label,
                doc: item.detail.unwrap_or_default(),
            })
            .collect(),
        lsp_types::CompletionResponse::List(list) => list
            .items
            .into_iter()
            .map(|item| LSPCompletion {
                keyword: item.label,
                doc: item.detail.unwrap_or_default(),
            })
            .collect(),
    }
}
