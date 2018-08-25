extern crate termion;
extern crate unicode_width;

use std::cmp::{max, min};
use std::io::{stdin, stdout, Write};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    row: usize,
    col: usize,
}

#[derive(Debug)]
pub struct Window {
    buffer: Vec<Vec<char>>,
    cursor: Cursor,
    row_offset: usize,
}

struct DrawBuffer {
    width: usize,
    buffer: Vec<Vec<char>>,
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

    fn put(&mut self, c: char) -> Option<Cursor> {
        if self.cursor.row >= self.buffer.len() {
            return None;
        }

        let w = c.width().unwrap_or(0);
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
        for (i, line) in self.buffer.iter().enumerate() {
            for &c in line {
                write!(out, "{}", c);
            }
            if i != self.buffer.len() - 1 {
                write!(out, "\r\n");
            }
        }
    }
}

fn refresh_screen<T: Write>(w: &mut T) {
    write!(w, "{}{}", termion::clear::All, termion::cursor::Goto(1, 1));
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

impl Window {
    pub fn new() -> Self {
        Window {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, col: 0 },
            row_offset: 0,
        }
    }

    fn windows_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    fn set_offset(&mut self) {
        if self.row_offset >= self.cursor.row {
            self.row_offset = self.cursor.row;
        } else {
            let (rows, cols) = Self::windows_size();
            let rows = rows - 1;
            let mut i = self.cursor.row + 1;
            let mut sum = 0;
            while i > 0 && sum + get_rows(&self.buffer[i - 1], cols) <= rows {
                sum += get_rows(&self.buffer[i - 1], cols);
                i -= 1;
            }
            self.row_offset = max(i, self.row_offset);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor.col != 0 {
            self.cursor.col -= 1;
        }
        self.set_offset();
    }

    pub fn cursor_right(&mut self) {
        self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col + 1);
        self.set_offset();
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row != 0 {
            self.cursor.row -= 1;
            self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col);
        }
        self.set_offset();
    }

    pub fn cursor_down(&mut self) {
        self.cursor.row = min(self.buffer.len() - 1, self.cursor.row + 1);
        self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col);
        self.set_offset();
    }

    pub fn insert(&mut self, c: char) {
        if c == '\n' {
            let rest: Vec<char> = self.buffer[self.cursor.row]
                .drain(self.cursor.col..)
                .collect();

            self.buffer.insert(self.cursor.row + 1, rest);
            self.cursor.row += 1;
            self.cursor.col = 0;
        } else {
            self.buffer[self.cursor.row].insert(self.cursor.col, c);
            self.cursor.col += 1;
        }
        self.set_offset();
    }

    pub fn backspase(&mut self) {
        if self.cursor.col > 0 {
            self.buffer[self.cursor.row].remove(self.cursor.col - 1);
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            let mut line = self.buffer.remove(self.cursor.row);
            self.cursor.col = self.buffer[self.cursor.row - 1].len();
            self.buffer[self.cursor.row - 1].append(&mut line);
            self.cursor.row -= 1;
        }
        self.set_offset();
    }

    pub fn draw<T: Write>(&self, out: &mut T) {
        refresh_screen(out);

        let (rows, cols) = Self::windows_size();

        let mut draw = DrawBuffer::new(rows - 1, cols);

        let mut cursor = None;

        'outer: for i in self.row_offset..self.buffer.len() {
            for j in 0..self.buffer[i].len() {
                let c = self.buffer[i][j];
                let t = Cursor { row: i, col: j };

                if self.cursor == t {
                    cursor = draw.put(c);
                } else {
                    if draw.put(c).is_none() {
                        break 'outer;
                    }
                }
            }

            let t = Cursor {
                row: i,
                col: self.buffer[i].len(),
            };

            if self.cursor == t {
                cursor = Some(draw.cursor);
            }

            draw.newline();
        }

        draw.draw(out);

        write!(
            out,
            "\r\n{}{}, {}{}",
            termion::color::Bg(termion::color::LightBlack),
            self.cursor.row + 1,
            self.cursor.col,
            termion::color::Bg(termion::color::Reset),
        );

        if let Some(c) = cursor {
            write!(
                out,
                "{}",
                termion::cursor::Goto(c.col as u16 + 1, c.row as u16 + 1)
            );
        }
    }
}
