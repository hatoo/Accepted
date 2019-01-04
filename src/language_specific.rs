use lsp;
use std::process;

pub trait Language {
    fn start_lsp(&self) -> Option<lsp::LSPClient>;
}

pub fn detect_language(extension: &str) -> Box<dyn Language> {
    match extension {
        "cpp" | "c" => Box::new(Cpp),
        "rs" => Box::new(Rust),
        _ => Box::new(Text),
    }
}

pub struct Cpp;
pub struct Rust;
pub struct Text;

impl Language for Cpp {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        lsp::LSPClient::start(process::Command::new("clangd"), "cpp".into())
    }
}

impl Language for Rust {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        lsp::LSPClient::start(process::Command::new("rls"), "rs".into())
    }
}

impl Language for Text {
    fn start_lsp(&self) -> Option<lsp::LSPClient> {
        None
    }
}
