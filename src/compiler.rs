use core::CursorRange;

pub struct CompilerOutput {
    pub message: String,
    pub line: usize,
    pub level: String,
    pub span: CursorRange,
}
