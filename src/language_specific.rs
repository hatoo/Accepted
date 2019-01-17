use crate::compiler::CompilerOutput;
use crate::core::Cursor;
use crate::core::CursorRange;
use crate::core::Id;
use crate::formatter;
use crate::job_queue::JobQueue;
use crate::lsp;
use crate::rustc;
use regex;
use std::ffi::OsString;
use std::io;
use std::io::BufRead;
use std::path;
use std::path::PathBuf;
use std::process;

#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct CompileId {
    pub id: Id,
    pub is_optimize: bool,
}

#[derive(Default)]
pub struct CompileResult {
    pub success: bool,
    pub messages: Vec<CompilerOutput>,
}

pub trait Language {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        None
    }
    fn indent_width(&self) -> usize {
        4
    }
    fn format(&self, _src: &str) -> Option<String> {
        None
    }
    // Must be async
    fn compile(&self, _path: path::PathBuf, _compile_id: CompileId) {}
    // Do not Block
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        None
    }
    // Block
    fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
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
    job_queue: JobQueue<(PathBuf, CompileId), (CompileId, CompileResult)>,
}
pub struct Rust {
    job_queue: JobQueue<(PathBuf, CompileId), (CompileId, CompileResult)>,
}
pub struct Text;

impl Default for Rust {
    fn default() -> Self {
        let job_queue = JobQueue::new(|(path, req): (PathBuf, CompileId)| {
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
            let mut success = false;

            if let Ok(rustc) = rustc.stderr(process::Stdio::piped()).output() {
                success = rustc.status.success();
                let buf = rustc.stderr;
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
            (req, CompileResult { messages, success })
        });

        Self { job_queue }
    }
}

impl Default for Cpp {
    fn default() -> Self {
        let job_queue = JobQueue::new(|(path, req): (PathBuf, CompileId)| {
            let mut clang = process::Command::new("clang++");
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
            let mut success = false;

            if let Ok(clang) = clang.stderr(process::Stdio::piped()).output() {
                success = clang.status.success();
                let buf = clang.stderr;
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
                            span: CursorRange(Cursor { row: line, col }, Cursor { row: line, col }),
                        };

                        messages.push(out);
                    }
                }
            }
            (req, CompileResult { success, messages })
        });

        Self { job_queue }
    }
}

impl Language for Cpp {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        lsp::LSPClient::start(process::Command::new("clangd"), "cpp".into())
    }
    fn indent_width(&self) -> usize {
        // Respect clang-format
        2
    }
    fn format(&self, src: &str) -> Option<String> {
        formatter::system_clang_format(src)
    }
    fn compile(&self, path: path::PathBuf, compile_id: CompileId) {
        self.job_queue.send((path, compile_id)).unwrap();
    }
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().try_recv().ok()
    }
    fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().recv().ok()
    }
    fn is_compiling(&self) -> bool {
        self.job_queue.is_running().unwrap()
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
        self.job_queue.send((path, compile_id)).unwrap();
    }
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().try_recv().ok()
    }
    fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().recv().ok()
    }
    fn is_compiling(&self) -> bool {
        self.job_queue.is_running().unwrap()
    }
}

impl Language for Text {}
