use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::fs;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use syntect::highlighting::FontStyle;
use termion;
use unicode_width::UnicodeWidthChar;

use crate::core::Cursor;
use crate::core::CursorRange;
use crate::core::Id;
use crate::draw;
use crate::draw::{CharStyle, LinenumView, View};
use crate::draw_cache::DrawCache;
use crate::language_specific;
use crate::language_specific::CompileId;
use crate::language_specific::CompileResult;
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
    syntax: syntax::Syntax<'a>,
    pub snippet: BTreeMap<String, String>,
    pub yank: Yank,
    last_save: Id,
    pub lsp: Option<LSPClient>,
    language: Box<dyn language_specific::Language>,
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

    pub fn new(syntax_parent: &'a syntax::SyntaxParent) -> Self {
        let syntax = syntax_parent.load_syntax_or_txt("txt");
        let language = language_specific::detect_language("txt");

        Self {
            path: None,
            core: Core::default(),
            search: Vec::new(),
            cache: DrawCache::new(&syntax),
            syntax,
            snippet: BTreeMap::new(),
            yank: Yank::default(),
            last_save: Id::default(),
            lsp: language.start_lsp(),
            language,
            row_offset: 0,
            last_compiler_result: None,
            syntax_parent,
            buffer_update: Id::default(),
            last_compiler_submit: CompileId::default(),
            last_compiler_compiled: CompileId::default(),
        }
    }

    pub fn extend_cache_duration(&mut self, duration: std::time::Duration) {
        self.cache
            .extend_cache_duration(self.core.buffer(), duration);
    }

    pub fn restart_lsp(&mut self) {
        self.lsp = self.language.start_lsp();
    }

    fn set_syntax(&mut self, extension: &str) {
        self.syntax = self.syntax_parent.load_syntax_or_txt(extension);
        self.cache = DrawCache::new(&self.syntax);
    }

    pub fn set_language(&mut self, extension: &str) {
        self.language = language_specific::detect_language(extension);
        self.set_syntax(extension);
        self.restart_lsp();
    }

    pub fn language(&self) -> &dyn language_specific::Language {
        self.language.as_ref()
    }

    pub fn indent(&mut self) {
        self.core.indent(self.language.indent_width());
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_ref().map(|p| p.as_path())
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if self.path.as_ref().map(|p| p.extension()) != Some(path.extension()) {
            self.set_language(
                path.extension()
                    .map(|o| o.to_str().unwrap_or("txt"))
                    .unwrap_or("txt"),
            );
        }
        self.path = Some(path);
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) {
        let core = if let Ok(f) = fs::File::open(path.as_ref()) {
            Core::from_reader(BufReader::new(f)).unwrap()
        } else {
            Core::default()
        };

        let extension = path
            .as_ref()
            .extension()
            .map(|o| o.to_str().unwrap_or(""))
            .unwrap_or("txt");
        self.set_language(extension);

        self.row_offset = 0;
        self.last_save = core.buffer_changed();
        self.core = core;
        self.path = Some(path.as_ref().to_path_buf());
        self.cache = DrawCache::new(&self.syntax);
        self.compile(false);
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
        if let Some(formatted) = self.language.format(&src) {
            if formatted != src {
                self.core.set_string(formatted, false);
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
            self.language
                .compile(path.clone(), self.last_compiler_submit);
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
        while let Some((id, res)) = self.language.try_recv_compile_result() {
            self.last_compiler_compiled = id;
            self.last_compiler_result = Some(res);
        }
    }

    pub fn wait_compile_message(&mut self) {
        while self.is_compiling() {
            if let Some((id, res)) = self.language.recv_compile_result() {
                self.last_compiler_compiled = id;
                self.last_compiler_result = Some(res);
            }
        }
    }

    pub fn is_compiling(&self) -> bool {
        self.language.is_compiling()
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
        view.bg = self.syntax.theme.settings.background;
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
                            line.to_mut()[k].1 = draw::CharStyle::Highlight;
                        }
                    }
                }
            }

            for (j, &c) in line.iter().enumerate() {
                let (c, mut style) = c;
                let t = Cursor { row: i, col: j };

                if self.is_annotate(t) {
                    if let CharStyle::Style(s) = &mut style {
                        s.font_style = FontStyle::UNDERLINE;
                    }
                }

                let style = if selected.as_ref().map(|r| r.contains(t)) == Some(true) {
                    CharStyle::Selected
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
                    view.put(' ', CharStyle::bg(col), Some(t));
                } else {
                    view.put(' ', CharStyle::Default, Some(t));
                }
            }

            if i != self.core.buffer().len_lines() - 1 {
                if let Some(col) = self.syntax.theme.settings.background {
                    while !view.cause_newline(' ') {
                        view.put(' ', CharStyle::bg(col), Some(t));
                    }
                } else {
                    while !view.cause_newline(' ') {
                        view.put(' ', CharStyle::Default, Some(t));
                    }
                }
                view.newline();
            }
        }

        cursor
    }
}
