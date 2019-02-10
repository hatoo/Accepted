use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use termion;
use unicode_width::UnicodeWidthChar;

use crate::compiler::CompileId;
use crate::compiler::CompileResult;
use crate::compiler::Compiler;
use crate::config;
use crate::core::Cursor;
use crate::core::CursorRange;
use crate::core::Id;
use crate::draw;
use crate::draw::{styles, CharStyle, LinenumView, View};
use crate::draw_cache::DrawCache;
use crate::formatter;
use crate::lsp::LSPClient;
use crate::ropey_util::RopeExt;
use crate::syntax;
use crate::Core;

pub struct Yank {
    pub insert_newline: bool,
    pub content: String,
}

impl Default for Yank {
    fn default() -> Self {
        Yank {
            insert_newline: false,
            content: String::new(),
        }
    }
}

fn get_rows(s: &str, width: usize) -> usize {
    let mut x = 0;
    let mut y = 1;

    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if x + w < width {
            x += w;
        } else {
            y += 1;
            x = w;
        }
    }
    y
}

pub struct Buffer<'a> {
    path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
    syntax_parent: &'a syntax::SyntaxParent,
    config: &'a config::ConfigWithDefault,
    syntax: syntax::Syntax<'a>,
    pub snippet: BTreeMap<String, String>,
    pub yank: Yank,
    last_save: Id,
    pub lsp: Option<LSPClient>,
    compiler: Option<Compiler<'a>>,
    row_offset: usize,
    last_compiler_result: Option<CompileResult>,
    cache: DrawCache<'a>,
    buffer_update: Id,
    last_compiler_submit: CompileId,
    last_compiler_compiled: CompileId,
}

impl<'a> Buffer<'a> {
    fn windows_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    pub fn new(
        syntax_parent: &'a syntax::SyntaxParent,
        config: &'a config::ConfigWithDefault,
    ) -> Self {
        let syntax = syntax_parent.load_syntax_or_txt("txt");

        let mut res = Self {
            path: None,
            core: Core::default(),
            search: Vec::new(),
            cache: DrawCache::new(&syntax),
            syntax,
            snippet: BTreeMap::new(),
            yank: Yank::default(),
            last_save: Id::default(),
            lsp: None,
            compiler: config.compiler(None).map(Compiler::new),
            row_offset: 0,
            last_compiler_result: None,
            syntax_parent,
            config,
            buffer_update: Id::default(),
            last_compiler_submit: CompileId::default(),
            last_compiler_compiled: CompileId::default(),
        };
        res.restart_lsp();
        res.reset_snippet();
        res
    }

    fn extension(&self) -> Option<&OsStr> {
        self.path.as_ref().and_then(|p| p.extension())
    }

    fn reset_snippet(&mut self) {
        self.snippet = self.config.snippets(self.extension());
    }

    pub fn extend_cache_duration(&mut self, duration: std::time::Duration) {
        self.cache
            .extend_cache_duration(self.core.buffer(), duration);
    }

    pub fn indent_width(&self) -> usize {
        self.config.indent_width(self.extension())
    }

    pub fn is_ansi_color(&self) -> bool {
        self.config.ansi_color(self.extension())
    }

    pub fn restart_lsp(&mut self) {
        let ext = self
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.lsp = self
            .config
            .lsp(self.extension())
            .and_then(|c| LSPClient::start(c.command(), ext));
    }

    fn set_syntax(&mut self, extension: &str) {
        self.syntax = self.syntax_parent.load_syntax_or_txt(extension);
        self.cache = DrawCache::new(&self.syntax);
    }

    pub fn set_language(&mut self) {
        self.compiler = self.config.compiler(self.extension()).map(Compiler::new);
        self.restart_lsp();
    }

    pub fn indent(&mut self) {
        self.core.indent(self.indent_width());
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_ref().map(|p| p.as_path())
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if self.extension() != path.extension() {
            self.set_language();
        }
        self.path = Some(path);
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) {
        let core = if let Ok(f) = fs::File::open(path.as_ref()) {
            Core::from_reader(BufReader::new(f)).unwrap()
        } else {
            Core::default()
        };

        let syntax_extension = self
            .config
            .syntax_extension(path.as_ref().extension())
            .unwrap_or_default()
            .to_string();
        self.set_syntax(&syntax_extension);

        self.row_offset = 0;
        self.last_save = core.buffer_changed();
        self.core = core;
        self.path = Some(path.as_ref().to_path_buf());
        self.set_language();
        self.cache = DrawCache::new(&self.syntax);
        self.compile(false);
        self.reset_snippet();
    }

    pub fn save(&mut self, is_optimize: bool) -> bool {
        let saved = if let Some(path) = self.path.as_ref() {
            if let Ok(f) = fs::File::create(path) {
                self.core.buffer().write_to(BufWriter::new(f)).unwrap();
                true
            } else {
                false
            }
        } else {
            false
        };
        if saved {
            self.compile(is_optimize);
        }
        saved
    }

    pub fn show_cursor(&mut self) {
        if self.row_offset >= self.core.cursor().row {
            self.row_offset = self.core.cursor().row;
        } else {
            let (rows, cols) = Self::windows_size();
            if cols < LinenumView::prefix_width(self.core.buffer().len_lines()) {
                return;
            }
            let cols = cols - LinenumView::prefix_width(self.core.buffer().len_lines());
            let rows = rows - 1;
            let mut i = self.core.cursor().row + 1;
            let mut sum = 0;
            while i > 0 && sum + get_rows(&Cow::from(self.core.buffer().l(i - 1)), cols) <= rows {
                sum += get_rows(&Cow::from(self.core.buffer().l(i - 1)), cols);
                i -= 1;
            }
            self.row_offset = max(i, self.row_offset);
        }
    }

    pub fn scroll_up(&mut self) {
        if self.row_offset < 3 {
            self.row_offset = 0;
        } else {
            self.row_offset -= 3;
        }
    }

    pub fn scroll_down(&mut self) {
        self.row_offset = min(self.row_offset + 3, self.core.buffer().len_lines() - 1);
    }

    pub fn show_cursor_middle(&mut self) {
        let (rows, _) = Self::windows_size();
        if rows / 2 > self.core.cursor().row {
            self.row_offset = 0;
        } else {
            self.row_offset = self.core.cursor().row - rows / 2;
        }
    }

    pub fn format(&mut self) {
        let src = self.core.get_string();
        let command = self.config.formatter(self.extension());

        if let Some(command) = command {
            if let Some(formatted) = formatter::system_format(command.command(), &src) {
                if formatted != self.core.get_string() {
                    self.core.set_string(formatted, false);
                }
            }
        }
    }

    pub fn compile(&mut self, is_optimize: bool) {
        if self.last_compiler_submit
            == (CompileId {
                id: self.core.buffer_changed(),
                is_optimize,
            })
        {
            return;
        }
        self.last_compiler_submit = CompileId {
            id: self.core.buffer_changed(),
            is_optimize,
        };

        if let Some(path) = self.path.as_ref() {
            if let Some(compiler) = self.compiler.as_ref() {
                compiler.compile(path.clone(), self.last_compiler_submit);
            }
        }
    }

    pub fn last_compile_success(&self) -> Option<bool> {
        self.last_compiler_result.as_ref().map(|res| res.success)
    }

    fn is_annotate(&self, cursor: Cursor) -> bool {
        self.last_compiler_result
            .as_ref()
            .map(|res| res.messages.iter().any(|r| r.span.contains(cursor)))
            .unwrap_or(false)
    }

    pub fn compiler_message_on_cursor(&self) -> Option<&str> {
        let line = self.core.cursor().row;
        self.last_compiler_result.as_ref().and_then(|res| {
            res.messages
                .iter()
                .find(|r| r.line == line)
                .map(|r| r.message.as_str())
        })
    }

    pub fn poll_compile_message(&mut self) {
        if let Some(compiler) = self.compiler.as_ref() {
            while let Some((id, res)) = compiler.try_recv_compile_result() {
                self.last_compiler_compiled = id;
                self.last_compiler_result = Some(res);
            }
        }
    }

    pub fn wait_compile_message(&mut self) {
        while self.is_compiling() {
            if let Some(compiler) = self.compiler.as_ref() {
                if let Some((id, res)) = compiler.recv_compile_result() {
                    self.last_compiler_compiled = id;
                    self.last_compiler_result = Some(res);
                }
            }
        }
    }

    pub fn is_compiling(&self) -> bool {
        self.compiler
            .as_ref()
            .map(|c| c.is_compiling())
            .unwrap_or(false)
    }

    pub fn draw(&mut self, view: View) -> Option<Cursor> {
        self.poll_compile_message();
        self.draw_with_selected(view, None)
    }

    pub fn draw_with_selected(
        &mut self,
        mut view: View,
        selected: Option<CursorRange>,
    ) -> Option<Cursor> {
        view.bg = self.syntax.theme.settings.background.map(|c| c.into());
        let v = Vec::new();
        let compiler_outputs = self
            .last_compiler_result
            .as_ref()
            .map(|res| &res.messages)
            .unwrap_or_else(|| &v);
        let mut view = LinenumView::new(
            self.row_offset,
            self.core.buffer().len_lines(),
            &compiler_outputs,
            view,
        );
        let mut cursor = None;

        if self.buffer_update != self.core.buffer_changed() {
            self.buffer_update = self.core.buffer_changed();
            self.cache.dirty_from(self.core.dirty_from);
        }

        'outer: for i in self.row_offset..self.core.buffer().len_lines() {
            self.cache.cache_line(self.core.buffer(), i);
            let line_ref = self.cache.get_line(i).unwrap();
            let mut line = Cow::Borrowed(line_ref);

            self.core.dirty_from = i;

            if !self.search.is_empty() && line.len() >= self.search.len() {
                for j in 0..=line.len() - self.search.len() {
                    let m = self
                        .search
                        .iter()
                        .zip(line[j..j + self.search.len()].iter())
                        .all(|(c1, (c2, _))| c1 == c2);
                    if m {
                        for k in j..j + self.search.len() {
                            line.to_mut()[k].1 = draw::styles::HIGHLIGHT;
                        }
                    }
                }
            }

            for (j, &c) in line.iter().enumerate() {
                let (c, mut style) = c;
                let t = Cursor { row: i, col: j };

                if self.is_annotate(t) {
                    style.modification = draw::CharModification::UnderLine;
                }

                let style = if selected.as_ref().map(|r| r.contains(t)) == Some(true) {
                    styles::SELECTED
                } else {
                    style
                };

                if self.core.cursor() == t {
                    cursor = view.put(c, style, Some(t));
                } else if view.put(c, style, Some(t)).is_none() {
                    break 'outer;
                }
            }
            let t = Cursor {
                row: i,
                col: self.core.buffer().l(i).len_chars(),
            };

            if self.core.cursor() == t {
                cursor = view.cursor();
            }

            if self.core.buffer().l(i).len_chars() == 0 {
                if let Some(col) = self.syntax.theme.settings.background {
                    view.put(' ', CharStyle::bg(col.into()), Some(t));
                } else {
                    view.put(' ', styles::DEFAULT, Some(t));
                }
            }

            if i != self.core.buffer().len_lines() - 1 {
                if let Some(col) = self.syntax.theme.settings.background {
                    while !view.cause_newline(' ') {
                        view.put(' ', CharStyle::bg(col.into()), Some(t));
                    }
                } else {
                    while !view.cause_newline(' ') {
                        view.put(' ', styles::DEFAULT, Some(t));
                    }
                }
                view.newline();
            }
        }

        cursor
    }
}
