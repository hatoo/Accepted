use compiler::CompilerOutput;
use core::Cursor;
use core::CursorRange;
use core::Id;
use draw;
use draw::{CharStyle, LinenumView, View};
use language_specific;
use language_specific::CompileId;
use lsp::LSPClient;
use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use syntax;
use syntect::highlighting::Color;
use syntect::highlighting::FontStyle;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack};
use termion;
use unicode_width::UnicodeWidthChar;
use Core;

struct DrawCache<'a> {
    syntax_set: &'a syntect::parsing::SyntaxSet,
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
    // 3 of { ( [
    parens_level: [usize; 3],
    bg: Color,
    cache: Vec<Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    const RAINBOW: [Color; 10] = [
        Color {
            r: 0xE6,
            g: 0x0,
            b: 0x12,
            a: 0xff,
        },
        Color {
            r: 0xf3,
            g: 0x98,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0xff,
            g: 0xf1,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0x8f,
            g: 0xc3,
            b: 0x1f,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x99,
            b: 0x44,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0xa0,
            b: 0xe9,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x68,
            b: 0xb7,
            a: 0xff,
        },
        Color {
            r: 0x92,
            g: 0x07,
            b: 0x83,
            a: 0xff,
        },
        Color {
            r: 0xe4,
            g: 0x00,
            b: 0x7f,
            a: 0xff,
        },
        Color {
            r: 0xe5,
            g: 0x00,
            b: 0x4f,
            a: 0xff,
        },
    ];

    fn new(syntax: &syntax::Syntax<'a>) -> Self {
        let highlighter = Highlighter::new(syntax.theme);
        let hstate = HighlightState::new(&highlighter, ScopeStack::new());
        let bg = syntax.theme.settings.background.unwrap();
        Self {
            syntax_set: syntax.syntax_set,
            highlighter,
            parse_state: ParseState::new(syntax.syntax),
            highlight_state: hstate,
            cache: Vec::new(),
            bg,
            parens_level: [0, 0, 0],
        }
    }

    fn cache_line(&mut self, buffer: &[Vec<char>], i: usize) -> &[(char, CharStyle)] {
        if self.cache.len() <= i {
            for i in self.cache.len()..=i {
                let line = buffer[i].iter().collect::<String>();
                let ops = self.parse_state.parse_line(&line, self.syntax_set);
                let iter = HighlightIterator::new(
                    &mut self.highlight_state,
                    &ops[..],
                    &line,
                    &self.highlighter,
                );
                let mut line = Vec::new();
                for (style, s) in iter {
                    for c in s.chars() {
                        line.push((c, draw::CharStyle::Style(style)));
                    }
                }
                let parens = [('{', '}'), ('(', ')'), ('[', ']')];
                for c in &mut line {
                    for (k, (l, r)) in parens.iter().enumerate() {
                        if c.0 == *l {
                            let fg = Self::RAINBOW[self.parens_level[k] % Self::RAINBOW.len()];
                            c.1 = CharStyle::fg_bg(fg, self.bg);
                            self.parens_level[k] += 1;
                        }
                        if c.0 == *r && self.parens_level[k] > 0 {
                            self.parens_level[k] -= 1;
                            let fg = Self::RAINBOW[self.parens_level[k] % Self::RAINBOW.len()];
                            c.1 = CharStyle::fg_bg(fg, self.bg);
                        }
                    }
                }
                self.cache.push(line);
            }
        }
        &self.cache[i]
    }

    fn get_line(&self, i: usize) -> Option<&[(char, CharStyle)]> {
        self.cache.get(i).map(|v| v.as_slice())
    }
}

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

fn get_rows(s: &[char], width: usize) -> usize {
    let mut x = 0;
    let mut y = 1;

    for &c in s {
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
    pub syntax_parent: &'a syntax::SyntaxParent,
    pub syntax: syntax::Syntax<'a>,
    pub snippet: BTreeMap<String, String>,
    pub yank: Yank,
    pub last_save: Id,
    pub lsp: Option<LSPClient>,
    language: Box<dyn language_specific::Language>,
    row_offset: usize,
    compiler_outputs: Vec<CompilerOutput>,
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
            compiler_outputs: Vec::new(),
            syntax_parent,
            buffer_update: Id::default(),
            last_compiler_submit: CompileId::default(),
            last_compiler_compiled: CompileId::default(),
        }
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

    pub fn language(&self) -> &Box<language_specific::Language> {
        &self.language
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
        let s = fs::read_to_string(path.as_ref()).unwrap_or_default();
        let mut core = Core::default();
        core.set_string(&s, true);

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
            if let Ok(mut f) = fs::File::create(path) {
                for line in self.core.buffer() {
                    writeln!(f, "{}", line.iter().collect::<String>()).unwrap();
                }
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
            if cols < LinenumView::prefix_width(self.core.buffer().len()) {
                return;
            }
            let cols = cols - LinenumView::prefix_width(self.core.buffer().len());
            let rows = rows - 1;
            let mut i = self.core.cursor().row + 1;
            let mut sum = 0;
            while i > 0 && sum + get_rows(&self.core.buffer()[i - 1], cols) <= rows {
                sum += get_rows(&self.core.buffer()[i - 1], cols);
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
        self.row_offset = min(self.row_offset + 3, self.core.buffer().len() - 1);
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
                self.core.set_string(&formatted, false);
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

    fn is_annotate(&self, cursor: Cursor) -> bool {
        self.compiler_outputs
            .iter()
            .any(|r| r.span.contains(cursor))
    }

    pub fn compiler_message_on_cursor(&self) -> Option<&str> {
        let line = self.core.cursor().row;
        self.compiler_outputs
            .iter()
            .find(|r| r.line == line)
            .map(|r| r.message.as_str())
    }

    pub fn poll_compile_message(&mut self) {
        while let Some((id, msg)) = self.language.try_recv_compile_message() {
            self.last_compiler_compiled = id;
            self.compiler_outputs = msg;
        }
    }

    pub fn wait_compile_message(&mut self) {
        while self.is_compiling() {
            if let Some((id, msg)) = self.language.recv_compile_message() {
                self.last_compiler_compiled = id;
                self.compiler_outputs = msg;
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
        let mut view = LinenumView::new(
            self.row_offset,
            self.core.buffer().len(),
            &self.compiler_outputs,
            view,
        );
        let mut cursor = None;

        if self.core.buffer_changed() != self.buffer_update {
            self.buffer_update = self.core.buffer_changed();
            self.cache = DrawCache::new(&self.syntax);
        }

        'outer: for i in self.row_offset..self.core.buffer().len() {
            self.cache.cache_line(self.core.buffer(), i);
            let line_ref = self.cache.get_line(i).unwrap();
            let mut line = Cow::Borrowed(line_ref);

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
                col: self.core.buffer()[i].len(),
            };

            if self.core.cursor() == t {
                cursor = view.cursor();
            }

            if self.core.buffer()[i].is_empty() {
                if let Some(col) = self.syntax.theme.settings.background {
                    view.put(' ', CharStyle::bg(col), Some(t));
                } else {
                    view.put(' ', CharStyle::Default, Some(t));
                }
            }

            if i != self.core.buffer().len() - 1 {
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
