#![feature(bound_cloned)]

pub mod buffer;
pub mod buffer_mode;
pub mod buffer_tab;
mod clipboard;
mod compiler;
pub mod config;
pub mod core;
mod cursor;
pub mod draw;
mod draw_cache;
mod env;
mod formatter;
mod indent;
mod job_queue;
mod lsp;
mod mode;
pub mod parenthesis;
mod rmate;
mod rustc;
pub mod storage;
pub mod syntax;
mod tabnine;
mod text_object;
pub mod theme;

pub use buffer::Buffer;
pub use buffer_mode::BufferMode;
