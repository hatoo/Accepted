use compiler::CompilerOutput;
use core::Cursor;
use core::CursorRange;
use core::Id;
use formatter;
use lsp;
use regex;
use rustc;
use std::ffi::OsString;
use std::io;
use std::io::BufRead;
use std::path;
use std::path::PathBuf;
use std::process;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct CompileId {
    pub id: Id,
    pub is_optimize: bool,
}

pub trait Language {
    fn start_lsp(&self) -> Option<lsp::LSPClient>;
    fn format(&self, src: &str) -> Option<String>;
    // Must be async
    fn compile(&self, _path: path::PathBuf, _compile_id: CompileId) {}
    // Do not Block
    fn try_recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        None
    }
    // Block
    fn recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        None
    }
    fn is_compiling(&self) -> bool {
        false
    }
}

pub fn detect_language(extension: &str) -> Box<dyn Language> {
    match extension {
        "cpp" | "c" => Box::new(Cpp::default()),
        "rs" => Box::new(Rust::default()),
        _ => Box::new(Text),
    }
}

pub struct Cpp {
    jobs: Arc<Mutex<usize>>,
    compile_tx: mpsc::Sender<(PathBuf, CompileId)>,
    message_rx: mpsc::Receiver<(CompileId, Vec<CompilerOutput>)>,
}
pub struct Rust {
    jobs: Arc<Mutex<usize>>,
    compile_tx: mpsc::Sender<(PathBuf, CompileId)>,
    message_rx: mpsc::Receiver<(CompileId, Vec<CompilerOutput>)>,
}
pub struct Text;

impl Default for Rust {
    fn default() -> Self {
        let (compile_tx, compile_rx) = mpsc::channel::<(PathBuf, CompileId)>();
        let (message_tx, message_rx) = mpsc::channel::<(CompileId, Vec<CompilerOutput>)>();
        let jobs = Arc::new(Mutex::new(0));

        let j = jobs.clone();
        // Compiler thread
        thread::spawn(move || {
            for (path, req) in compile_rx {
                let mut rustc = process::Command::new("rustc");
                if req.is_optimize {
                    rustc.args(&[
                        &OsString::from("-Z"),
                        &OsString::from("unstable-options"),
                        &OsString::from("--error-format=json"),
                        &OsString::from("-O"),
                        path.as_os_str(),
                    ]);
                } else {
                    rustc.args(&[
                        &OsString::from("-Z"),
                        &OsString::from("unstable-options"),
                        &OsString::from("--error-format=json"),
                        path.as_os_str(),
                    ]);
                }

                let mut messages = Vec::new();
                if let Ok(rustc) = rustc.stderr(process::Stdio::piped()).output() {
                    let mut buf = rustc.stderr;
                    let mut reader = io::Cursor::new(buf);
                    let mut line = String::new();

                    while {
                        line.clear();
                        reader.read_line(&mut line).is_ok() && !line.is_empty()
                    } {
                        if let Some(rustc_output) = rustc::parse_rustc_json(&line) {
                            messages.push(rustc_output);
                        }
                    }
                }

                {
                    let mut data = j.lock().unwrap();
                    *data -= 1;
                }
                message_tx.send((req, messages)).unwrap();
            }
        });

        Self {
            jobs,
            compile_tx,
            message_rx,
        }
    }
}

impl Default for Cpp {
    fn default() -> Self {
        let (compile_tx, compile_rx) = mpsc::channel::<(PathBuf, CompileId)>();
        let (message_tx, message_rx) = mpsc::channel::<(CompileId, Vec<CompilerOutput>)>();
        let jobs = Arc::new(Mutex::new(0));

        let j = jobs.clone();
        // Compiler thread
        thread::spawn(move || {
            for (path, req) in compile_rx {
                let mut clang = process::Command::new("clang");
                let stem = path.file_stem().unwrap();
                if req.is_optimize {
                    clang.args(&[
                        path.as_os_str(),
                        &OsString::from("-O2"),
                        &OsString::from("-o"),
                        stem,
                    ]);
                } else {
                    clang.args(&[path.as_os_str(), &OsString::from("-o"), stem]);
                }

                let mut messages = Vec::new();

                if let Ok(clang) = clang.stderr(process::Stdio::piped()).output() {
                    let mut buf = clang.stderr;
                    let mut reader = io::Cursor::new(buf);
                    let mut line = String::new();

                    let re = regex::Regex::new(
                        r"^[^:]*:(?P<line>\d*):(?P<col>\d*): (?P<level>[^:]*): (?P<msg>.*)",
                    )
                    .unwrap();

                    while {
                        line.clear();
                        reader.read_line(&mut line).is_ok() && !line.is_empty()
                    } {
                        if let Some(caps) = re.captures(&line) {
                            let line = caps["line"].parse::<usize>().unwrap() - 1;
                            let col = caps["col"].parse::<usize>().unwrap() - 1;
                            let out = CompilerOutput {
                                message: caps["msg"].into(),
                                line,
                                level: caps["level"].into(),
                                span: CursorRange(
                                    Cursor { row: line, col },
                                    Cursor { row: line, col },
                                ),
                            };

                            messages.push(out);
                        }
                    }
                }

                {
                    let mut data = j.lock().unwrap();
                    *data -= 1;
                }
                message_tx.send((req, messages)).unwrap();
            }
        });

        Self {
            jobs,
            compile_tx,
            message_rx,
        }
    }
}

impl Language for Cpp {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        lsp::LSPClient::start(process::Command::new("clangd"), "cpp".into())
    }
    fn format(&self, src: &str) -> Option<String> {
        formatter::system_clang_format(src)
    }
    fn compile(&self, path: path::PathBuf, compile_id: CompileId) {
        let mut j = self.jobs.lock().unwrap();
        *j += 1;
        self.compile_tx.send((path, compile_id)).unwrap();
    }
    fn try_recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        self.message_rx.try_recv().ok()
    }
    fn recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        self.message_rx.recv().ok()
    }
    fn is_compiling(&self) -> bool {
        *self.jobs.lock().unwrap() != 0
    }
}

impl Language for Rust {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        lsp::LSPClient::start(process::Command::new("rls"), "rs".into())
    }
    fn format(&self, src: &str) -> Option<String> {
        formatter::system_rustfmt(src)
    }
    fn compile(&self, path: path::PathBuf, compile_id: CompileId) {
        let mut j = self.jobs.lock().unwrap();
        *j += 1;
        self.compile_tx.send((path, compile_id)).unwrap();
    }
    fn try_recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        self.message_rx
            .try_recv()
            .map(|mut res| {
                for m in &mut res.1 {
                    let r = m.span.r_mut();
                    if r.col > 0 {
                        r.col -= 1;
                    }
                }
                res
            })
            .ok()
    }
    fn recv_compile_message(&self) -> Option<(CompileId, Vec<CompilerOutput>)> {
        self.message_rx
            .recv()
            .map(|mut res| {
                for m in &mut res.1 {
                    let r = m.span.r_mut();
                    if r.col > 0 {
                        r.col -= 1;
                    }
                }
                res
            })
            .ok()
    }
    fn is_compiling(&self) -> bool {
        *self.jobs.lock().unwrap() != 0
    }
}

impl Language for Text {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        None
    }
    fn format(&self, _src: &str) -> Option<String> {
        None
    }
}
