use core::Cursor;
use core::CursorRange;
use draw;
use draw::{CharStyle, LinenumView, View};
use lsp::LSPClient;
use rustc;
use rustc::RustcOutput;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::num::Wrapping;
use std::path::{Path, PathBuf};
use std::process;
use syntax;
use syntect::highlighting::Color;
use syntect::highlighting::FontStyle;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack};
use termion;
use unicode_width::UnicodeWidthChar;
use Core;

struct DrawCache<'a> {
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
    // 3 of { ( [
    parens_level: [usize; 3],
    bg: Color,
    cache: Vec<Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    const RAINBOW: [Color; 12] = [
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
            g: 0x0e,
            b: 0x96,
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
            r: 0x1d,
            g: 0x20,
            b: 0x88,
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
            highlighter,
            parse_state: ParseState::new(syntax.syntax),
            highlight_state: hstate,
            cache: Vec::new(),
            bg,
            parens_level: [0, 0, 0],
        }
    }

    fn get_line(&mut self, buffer: &[Vec<char>], i: usize) -> &[(char, CharStyle)] {
        if self.cache.len() <= i {
            for i in self.cache.len()..=i {
                let line = buffer[i].iter().collect::<String>();
                let ops = self.parse_state.parse_line(&line);
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
                    for (k, (l, r)) in parens.into_iter().enumerate() {
                        if c.0 == *l {
                            if let Some(&fg) = Self::RAINBOW.get(self.parens_level[k]) {
                                c.1 = CharStyle::fg_bg(fg, self.bg);
                            }
                            self.parens_level[k] += 1;
                        }
                        if c.0 == *r && self.parens_level[k] > 0 {
                            self.parens_level[k] -= 1;
                            if let Some(&fg) = Self::RAINBOW.get(self.parens_level[k]) {
                                c.1 = CharStyle::fg_bg(fg, self.bg);
                            }
                        }
                    }
                }
                self.cache.push(line);
            }
        }
        &self.cache[i]
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
    pub path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
    pub syntax: syntax::Syntax<'a>,
    pub snippet: BTreeMap<String, String>,
    pub yank: Yank,
    pub last_save: Wrapping<usize>,
    pub lsp: Option<LSPClient>,
    row_offset: usize,
    rustc_outputs: Vec<RustcOutput>,
    cache: RefCell<DrawCache<'a>>,
    buffer_update: Cell<Wrapping<usize>>,
    last_rustc: (Wrapping<usize>, bool),
}

impl<'a> Buffer<'a> {
    fn windows_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    pub fn new(syntax: syntax::Syntax<'a>) -> Self {
        Self {
            path: None,
            core: Core::default(),
            search: Vec::new(),
            cache: RefCell::new(DrawCache::new(&syntax)),
            snippet: BTreeMap::new(),
            yank: Yank::default(),
            last_save: Wrapping(0),
            lsp: LSPClient::new(),
            row_offset: 0,
            rustc_outputs: Vec::new(),
            syntax,
            buffer_update: Cell::new(Wrapping(0)),
            last_rustc: (Wrapping(0), false),
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) {
        let s = fs::read_to_string(path.as_ref()).unwrap_or_default();
        let mut core = Core::default();
        core.set_string(&s, true);

        self.row_offset = 0;
        self.last_save = core.buffer_changed;
        self.core = core;
        self.path = Some(path.as_ref().to_path_buf());
        self.cache.replace(DrawCache::new(&self.syntax));
        self.rustc(false);
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

    pub fn rustc(&mut self, is_optimize: bool) {
        if self.last_rustc == (self.core.buffer_changed, is_optimize) {
            return;
        }
        self.last_rustc = (self.core.buffer_changed, is_optimize);
        if let Some(path) = self.path.as_ref() {
            let mut rustc = process::Command::new("rustc");
            if is_optimize {
                rustc.args(
                    [
                        &OsString::from("-Z"),
                        &OsString::from("unstable-options"),
                        &OsString::from("--error-format=json"),
                        &OsString::from("-O"),
                        path.as_os_str(),
                    ]
                        .iter(),
                );
            } else {
                rustc.args(
                    [
                        &OsString::from("-Z"),
                        &OsString::from("unstable-options"),
                        &OsString::from("--error-format=json"),
                        path.as_os_str(),
                    ]
                        .iter(),
                );
            }

            if let Ok(rustc) = rustc.stderr(process::Stdio::piped()).output() {
                let mut buf = rustc.stderr;
                let mut reader = io::Cursor::new(buf);
                let mut line = String::new();

                self.rustc_outputs.clear();
                while {
                    line.clear();
                    reader.read_line(&mut line).is_ok() && !line.is_empty()
                } {
                    if let Some(rustc_output) = rustc::parse_rustc_json(&line, &self.core) {
                        self.rustc_outputs.push(rustc_output);
                    }
                }
            }
        }
    }

    fn is_annotate(&self, cursor: Cursor) -> bool {
        self.rustc_outputs.iter().any(|r| r.span.contains(cursor))
    }

    pub fn rustc_message(&self) -> Option<&str> {
        let line = self.core.cursor().row;
        self.rustc_outputs
            .iter()
            .find(|r| r.line == line)
            .map(|r| r.message.as_str())
    }

    pub fn save(&mut self, is_optimize: bool) -> bool {
        let saved = if let Some(path) = self.path.as_ref() {
            if let Ok(mut f) = fs::File::create(path) {
                for line in self.core.buffer() {
                    writeln!(f, "{}", line.iter().collect::<String>());
                }
                true
            } else {
                false
            }
        } else {
            false
        };
        if saved {
            self.rustc(is_optimize);
        }
        saved
    }

    pub fn draw(&self, view: View) -> Option<Cursor> {
        self.draw_with_selected(view, None)
    }

    pub fn draw_with_selected(
        &self,
        mut view: View,
        selected: Option<CursorRange>,
    ) -> Option<Cursor> {
        view.bg = self.syntax.theme.settings.background;
        let mut view = LinenumView::new(
            self.row_offset,
            self.core.buffer().len(),
            &self.rustc_outputs,
            view,
        );
        let mut cursor = None;

        if self.core.buffer_changed != self.buffer_update.get() {
            self.buffer_update.set(self.core.buffer_changed);
            self.cache.replace(DrawCache::new(&self.syntax));
        }

        'outer: for i in self.row_offset..self.core.buffer().len() {
            let mut cache = self.cache.borrow_mut();
            let line_ref = cache.get_line(self.core.buffer(), i);
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
                view.newline();
            }
        }

        cursor
    }
}
