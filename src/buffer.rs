use core::Cursor;
use draw::{CharStyle, View};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use termion::color::{Fg, Reset, Rgb};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use Core;

pub struct Buffer {
    pub path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            core: Core::new(),
            search: Vec::new(),
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        fs::read_to_string(path.as_ref()).map(|s| {
            let mut core = Core::new();
            core.buffer = s
                .lines()
                .map(|l| l.trim_right().chars().collect())
                .collect();

            if core.buffer.is_empty() {
                core.buffer = vec![Vec::new()];
            }

            Self {
                path: Some(path.as_ref().to_path_buf()),
                core,
                search: Vec::new(),
            }
        })
    }

    pub fn draw(&self, mut view: View) -> Option<Cursor> {
        let mut cursor = None;

        'outer: for i in self.core.row_offset..self.core.buffer.len() {
            let line: Vec<(char, CharStyle)> = self.core.buffer[i]
                .iter()
                .cloned()
                .zip(
                    search(self.search.as_slice(), &self.core.buffer[i])
                        .into_iter()
                        .map(|b| {
                            if b {
                                CharStyle::Highlight
                            } else {
                                CharStyle::Default
                            }
                        }),
                ).collect();

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

                let t = Cursor {
                    row: i,
                    col: self.core.buffer[i].len(),
                };

                if self.core.cursor == t {
                    cursor = Some(view.cursor);
                }
            }
            view.newline();
        }

        cursor
    }

    /*
    pub fn draw<T: Write>(&self, rows: usize, cols: usize, out: &mut T) -> Option<Cursor> {
        let mut draw = DrawBuffer::new(rows, cols);

        let mut cursor = None;

        'outer: for i in self.core.row_offset..self.core.buffer.len() {
            let line: Vec<(char, Option<Rgb>)> = self.core.buffer[i]
                .iter()
                .cloned()
                .zip(
                    search(self.search.as_slice(), &self.core.buffer[i])
                        .into_iter()
                        .map(|b| if b { Some(Rgb(255, 0, 0)) } else { None }),
                ).collect();

            for j in 0..self.core.buffer[i].len() {
                let c = line[j];
                let t = Cursor { row: i, col: j };

                if self.core.cursor == t {
                    cursor = draw.put(c);
                } else {
                    if draw.put(c).is_none() {
                        break 'outer;
                    }
                }
            }

            let t = Cursor {
                row: i,
                col: self.core.buffer[i].len(),
            };

            if self.core.cursor == t {
                cursor = Some(draw.cursor);
            }

            draw.newline();
        }

        draw.draw(out);

        cursor
    }
    */
}

fn search(seq: &[char], line: &[char]) -> Vec<bool> {
    let mut res = vec![false; line.len()];
    if seq.is_empty() || line.is_empty() {
        return res;
    }
    let mut i = 0;
    while i + seq.len() <= line.len() {
        if &line[i..i + seq.len()] == seq {
            for _ in 0..seq.len() {
                res[i] = true;
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    res
}

struct DrawBuffer {
    width: usize,
    buffer: Vec<Vec<(char, Option<Rgb>)>>,
    cursor: Cursor,
}

impl DrawBuffer {
    fn new(height: usize, width: usize) -> Self {
        DrawBuffer {
            width,
            buffer: vec![Vec::new(); height],
            cursor: Cursor { row: 0, col: 0 },
        }
    }

    fn newline(&mut self) {
        self.cursor.col = 0;
        self.cursor.row += 1;
    }

    fn put(&mut self, c: (char, Option<Rgb>)) -> Option<Cursor> {
        if self.cursor.row >= self.buffer.len() {
            return None;
        }

        let w = c.0.width().unwrap_or(0);
        if self.cursor.col + w < self.width {
            let prev = self.cursor;
            self.buffer[self.cursor.row].push(c);
            self.cursor.col += w;

            Some(prev)
        } else {
            self.cursor.row += 1;
            if self.cursor.row >= self.buffer.len() {
                return None;
            }
            self.buffer[self.cursor.row].push(c);
            self.cursor.col = w;

            Some(Cursor {
                row: self.cursor.row,
                col: 0,
            })
        }
    }

    fn draw<W: Write>(&self, out: &mut W) {
        let mut current_color = None;
        write!(out, "{}", Fg(Reset));
        for (i, line) in self.buffer.iter().enumerate() {
            for &(c, color) in line {
                if current_color != color {
                    if let Some(rgb) = color {
                        write!(out, "{}", Fg(rgb));
                    } else {
                        write!(out, "{}", Fg(Reset));
                    }
                    current_color = color;
                }
                write!(out, "{}", c);
            }
            if i != self.buffer.len() - 1 {
                write!(out, "\r\n");
            }
        }
        write!(out, "{}", Fg(Reset));
    }
}
