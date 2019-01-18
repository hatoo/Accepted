#![allow(dead_code)]

use serde_derive::Deserialize;
use serde_json;
use serde_json::Value;

use crate::compiler::CompilerOutput;
use crate::core::{Cursor, CursorRange};

pub fn parse_rustc_json(json: &str) -> Option<CompilerOutput> {
    let d: Diagnostic = serde_json::from_str(json).ok()?;
    let span = d.spans.iter().find(|s| s.is_primary)?;
    let line = span.line_start - 1;
    let start = Cursor {
        row: span.line_start - 1,
        col: span.column_start - 1,
    };
    let mut end = Cursor {
        row: span.line_end - 1,
        col: span.column_end - 1,
    };

    if end.col > 0 {
        end.col -= 1;
    }

    let span = CursorRange(start, end);

    Some(CompilerOutput {
        message: d.message,
        line,
        level: d.level,
        span,
    })
}

#[derive(Deserialize)]
struct Diagnostic {
    /// The primary error message.
    message: String,
    code: Option<DiagnosticCode>,
    /// "error: internal compiler error", "error", "warning", "note", "help".
    level: String,
    spans: Vec<DiagnosticSpan>,
    /// Associated diagnostic messages.
    children: Vec<Diagnostic>,
    /// The message as rustc would render it.
    rendered: Option<String>,
}

#[derive(Deserialize)]
#[allow(unused_attributes)]
struct DiagnosticSpan {
    file_name: String,
    byte_start: u32,
    byte_end: u32,
    /// 1-based.
    line_start: usize,
    line_end: usize,
    /// 1-based, character offset.
    column_start: usize,
    column_end: usize,
    /// Is this a "primary" span -- meaning the point, or one of the points,
    /// where the error occurred?
    is_primary: bool,
    /// Source text from the start of line_start to the end of line_end.
    text: Vec<DiagnosticSpanLine>,
    /// Label that should be placed at this location (if any)
    label: Option<String>,
    /// If we are suggesting a replacement, this will contain text
    /// that should be sliced in atop this span.
    suggested_replacement: Option<String>,
    /// If the suggestion is approximate
    suggestion_applicability: Option<Value>,
    /// Macro invocations that created the code at this span, if any.
    expansion: Option<Box<DiagnosticSpanMacroExpansion>>,
}

#[derive(Deserialize)]
struct DiagnosticSpanLine {
    text: String,

    /// 1-based, character offset in self.text.
    highlight_start: usize,

    highlight_end: usize,
}

#[derive(Deserialize)]
struct DiagnosticSpanMacroExpansion {
    /// span where macro was applied to generate this code; note that
    /// this may itself derive from a macro (if
    /// `span.expansion.is_some()`)
    span: DiagnosticSpan,

    /// name of macro that was applied (e.g., "foo!" or "#[derive(Eq)]")
    macro_decl_name: String,

    /// span where macro was defined (if known)
    def_site_span: Option<DiagnosticSpan>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    /// The code itself.
    code: String,
    /// An explanation for the code.
    explanation: Option<String>,
}
