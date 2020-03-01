use std::io;
use std::io::BufRead;
use std::path::PathBuf;
use std::process;

use regex;

use crate::config::types::CompilerConfig;
use crate::config::types::CompilerType;
use crate::core::Cursor;
use crate::core::Id;
use crate::job_queue::JobQueue;
use crate::rustc;
use std::ffi::OsString;
use std::ops::RangeInclusive;

use futures::prelude::*;
pub struct CompilerOutput {
    pub message: String,
    pub line: usize,
    pub level: String,
    pub span: RangeInclusive<Cursor>,
}

pub struct Compiler<'a> {
    config: &'a CompilerConfig,
    worker: Box<dyn CompilerWorker>,
}

impl<'a> Compiler<'a> {
    pub fn new(config: &'a CompilerConfig) -> Self {
        let worker: Box<dyn CompilerWorker> = match config.output_type {
            None => Box::new(Unknown::default()),
            Some(CompilerType::Gcc) => Box::new(Cpp::default()),
            Some(CompilerType::Rustc) => Box::new(Rust::default()),
        };

        Self { config, worker }
    }

    pub fn compile(&self, path: PathBuf, compile_id: CompileId) {
        crate::env::set_env(&path);
        if let Some((head, tail)) = self.config.command.split_first() {
            if let Ok(head) = shellexpand::full(head) {
                let mut command = process::Command::new(OsString::from(head.as_ref()));
                let mut args = tail.to_vec();

                if compile_id.is_optimize {
                    args.extend_from_slice(self.config.optimize_option.as_slice());
                }

                if let Ok(args) = args
                    .iter()
                    .map(shellexpand::full)
                    .collect::<Result<Vec<_>, _>>()
                {
                    command.args(args.into_iter().map(|s| OsString::from(s.as_ref())));
                    self.worker.compile(command, compile_id);
                }
            }
        }
    }

    pub fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.worker.try_recv_compile_result()
    }
    // Block
    pub fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.worker.recv_compile_result()
    }
    pub fn is_compiling(&self) -> bool {
        self.worker.is_compiling()
    }
}

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

trait CompilerWorker {
    // Must be async
    fn compile(&self, _command: process::Command, _compile_id: CompileId) {}
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

pub struct Cpp {
    job_queue: JobQueue<(process::Command, CompileId), (CompileId, CompileResult)>,
}

pub struct Rust {
    job_queue: JobQueue<(process::Command, CompileId), (CompileId, CompileResult)>,
}

pub struct Unknown {
    job_queue: JobQueue<(process::Command, CompileId), (CompileId, CompileResult)>,
}

impl Default for Unknown {
    fn default() -> Self {
        let job_queue = JobQueue::new(|(mut cmd, req): (process::Command, CompileId)| {
            async move {
                let messages = Vec::new();
                let mut success = false;

                if let Ok(cmd) = cmd.stderr(process::Stdio::piped()).output() {
                    success = cmd.status.success();
                }
                (req, CompileResult { messages, success })
            }
            .boxed()
        });

        Self { job_queue }
    }
}

impl Default for Rust {
    fn default() -> Self {
        let job_queue = JobQueue::new(|(mut rustc, req): (process::Command, CompileId)| {
            async move {
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
            }
            .boxed()
        });

        Self { job_queue }
    }
}

impl Default for Cpp {
    fn default() -> Self {
        let job_queue = JobQueue::new(|(mut clang, req): (process::Command, CompileId)| {
            async move {
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
                                span: Cursor { row: line, col }..=Cursor { row: line, col },
                            };

                            messages.push(out);
                        }
                    }
                }
                (req, CompileResult { success, messages })
            }
            .boxed()
        });

        Self { job_queue }
    }
}

impl CompilerWorker for Unknown {
    fn compile(&self, command: process::Command, compile_id: CompileId) {
        self.job_queue.send((command, compile_id)).unwrap();
    }
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().try_recv().ok()
    }
    fn recv_compile_result(&mut self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().recv().ok()
    }
    fn is_compiling(&self) -> bool {
        self.job_queue.is_running()
    }
}

impl CompilerWorker for Cpp {
    fn compile(&self, command: process::Command, compile_id: CompileId) {
        self.job_queue.send((command, compile_id)).unwrap();
    }
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().try_recv().ok()
    }
    fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().recv().ok()
    }
    fn is_compiling(&self) -> bool {
        self.job_queue.is_running()
    }
}

impl CompilerWorker for Rust {
    fn compile(&self, command: process::Command, compile_id: CompileId) {
        self.job_queue.send((command, compile_id)).unwrap();
    }
    fn try_recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().try_recv().ok()
    }
    fn recv_compile_result(&self) -> Option<(CompileId, CompileResult)> {
        self.job_queue.rx().recv().ok()
    }
    fn is_compiling(&self) -> bool {
        self.job_queue.is_running()
    }
}
