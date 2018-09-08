use core::Cursor;
use draw;
use draw::{CharStyle, LinenumView, View};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use syntax;
use syntect;
use Core;

pub struct Buffer<'a> {
    pub path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
    pub syntax: syntax::Syntax<'a>,
}

impl<'a> Buffer<'a> {
    pub fn new(syntax: syntax::Syntax<'a>) -> Self {
        Self {
            path: None,
            core: Core::new(),
            search: Vec::new(),
            syntax,
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        fs::read_to_string(path.as_ref()).map(|s| {
            let mut core = Core::new();
            core.buffer = s
                .lines()
                .map(|l| l.trim_right().chars().collect())
                .collect();

            if core.buffer.is_empty() {
                core.buffer = vec![Vec::new()];
            }

            self.core = core;
            self.path = Some(path.as_ref().to_path_buf());
        })
    }

    pub fn draw(&self, view: View) -> Option<Cursor> {
        let mut view = LinenumView::new(self.core.row_offset + 1, self.core.buffer.len() + 1, view);
        let mut cursor = None;
        let bg = self.syntax.theme.settings.background;

        let mut hl = self.syntax.highlight_lines();
        for i in 0..self.core.row_offset {
            hl.highlight(self.core.buffer[i].iter().collect::<String>().as_str());
        }

        'outer: for i in self.core.row_offset..self.core.buffer.len() {
            let mut line: Vec<(char, CharStyle)> = hl
                .highlight(self.core.buffer[i].iter().collect::<String>().as_str())
                .into_iter()
                .flat_map(|(style, s)| {
                    s.chars()
                        .map(|c| (c, draw::CharStyle::Style(style.clone())))
                        .collect::<Vec<_>>()
                        .into_iter()
                }).collect();

            if !self.search.is_empty() && line.len() >= self.search.len() {
                for j in 0..line.len() - self.search.len() + 1 {
                    let m = self
                        .search
                        .iter()
                        .zip(line[j..j + self.search.len()].iter())
                        .all(|(c1, (c2, _))| c1 == c2);
                    if m {
                        for k in j..j + self.search.len() {
                            line[k].1 = draw::CharStyle::Highlight;
                        }
                    }
                }
            }

            for j in 0..self.core.buffer[i].len() {
                let c = line[j];
                let t = Cursor { row: i, col: j };

                if self.core.cursor == t {
                    cursor = view.put(c.0, c.1);
                } else {
                    if view.put(c.0, c.1).is_none() {
                        break 'outer;
                    }
                }
            }
            let t = Cursor {
                row: i,
                col: self.core.buffer[i].len(),
            };

            if self.core.cursor == t {
                cursor = view.cursor();
            }

            if let Some(bg) = bg {
                if self.core.buffer[i].is_empty() {
                    let style = syntect::highlighting::Style {
                        foreground: syntect::highlighting::Color::BLACK,
                        background: bg,
                        font_style: syntect::highlighting::FontStyle::default(),
                    };
                    view.put(' ', draw::CharStyle::Style(style));
                }
            }

            if i != self.core.buffer.len() - 1 {
                view.newline();
            }
        }

        cursor
    }
}
