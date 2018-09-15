use core::Cursor;
use core::CursorRange;
use draw;
use draw::{CharStyle, LinenumView, View};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fs;
use std::io;
use std::io::Write;
use std::num::Wrapping;
use std::path::{Path, PathBuf};
use syntax;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack};

use Core;

struct DrawCache<'a> {
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
    cache: Vec<Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    fn new(syntax: &syntax::Syntax<'a>) -> Self {
        let highlighter = Highlighter::new(syntax.theme);
        let hstate = HighlightState::new(&highlighter, ScopeStack::new());
        Self {
            highlighter: highlighter,
            parse_state: ParseState::new(syntax.syntax),
            highlight_state: hstate,
            cache: Vec::new(),
        }
    }

    fn get_line(&mut self, buffer: &Vec<Vec<char>>, i: usize) -> &[(char, CharStyle)] {
        for i in self.cache.len()..=i {
            let line = &buffer[i].iter().collect::<String>();
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
            self.cache.push(line);
        }
        &self.cache[i]
    }
}

pub struct Buffer<'a> {
    pub path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
    pub syntax: syntax::Syntax<'a>,
    cache: RefCell<DrawCache<'a>>,
    buffer_update: Cell<Wrapping<usize>>,
}

impl<'a> Buffer<'a> {
    pub fn new(syntax: syntax::Syntax<'a>) -> Self {
        Self {
            path: None,
            core: Core::new(),
            search: Vec::new(),
            cache: RefCell::new(DrawCache::new(&syntax)),
            syntax,
            buffer_update: Cell::new(Wrapping(0)),
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) {
        let s = fs::read_to_string(path.as_ref()).unwrap_or(String::new());
        let mut core = Core::new();
        core.set_string(&s, true);

        self.core = core;
        self.path = Some(path.as_ref().to_path_buf());
    }

    pub fn save(&self) -> Option<io::Result<()>> {
        self.path.as_ref().map(|path| {
            let mut f = fs::File::create(path)?;
            for line in self.core.buffer() {
                write!(f, "{}\n", line.iter().collect::<String>());
            }
            Ok(())
        })
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
        let mut view =
            LinenumView::new(self.core.row_offset + 1, self.core.buffer().len() + 1, view);
        let mut cursor = None;

        if self.core.buffer_changed != self.buffer_update.get() {
            self.buffer_update.set(self.core.buffer_changed);
            self.cache.replace(DrawCache::new(&self.syntax));
        }

        'outer: for i in self.core.row_offset..self.core.buffer().len() {
            let mut cache = self.cache.borrow_mut();
            let line_ref = cache.get_line(self.core.buffer(), i);
            let mut line = Cow::Borrowed(line_ref);

            if !self.search.is_empty() && line.len() >= self.search.len() {
                for j in 0..line.len() - self.search.len() + 1 {
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

            for j in 0..line.len() {
                let (c, style) = line[j];
                let t = Cursor { row: i, col: j };

                let style = if selected.as_ref().map(|r| r.contains(t)) == Some(true) {
                    CharStyle::Selected
                } else {
                    style
                };

                if self.core.cursor() == t {
                    cursor = view.put(c, style, Some(t));
                } else {
                    if view.put(c, style, Some(t)).is_none() {
                        break 'outer;
                    }
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
