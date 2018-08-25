extern crate termion;
extern crate unicode_width;

use std::cmp::{max, min};
use std::io::{stdin, stdout, Write};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, PartialEq, Eq)]
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

        let (cols, rows) = termion::terminal_size().unwrap();
        let cols = cols as usize;
        let rows = rows as usize;

        let mut cr = 0;
        let mut cc = 0;

        let mut row = 0;
        'outer: for y in self.row_offset..self.buffer.len() {
            let mut col = 0;
            for x in 0..self.buffer[y].len() + 1 {
                let cursor = Cursor { row: y, col: x };
                if self.cursor == cursor {
                    cr = row;
                    cc = col;
                }
                if x < self.buffer[y].len() {
                    let c = self.buffer[y][x];
                    let w = c.width().unwrap_or(0);
                    if col + w < cols {
                        write!(out, "{}", c);
                        col += w;
                    } else {
                        row += 1;
                        if row == rows {
                            break 'outer;
                        }
                        write!(out, "\r\n");
                        write!(out, "{}", c);
                        col = w;
                    }
                }
            }
            row += 1;
            if row == rows {
                break 'outer;
            }
            write!(out, "\r\n");
        }

        // write!(out, "\r\n {:?} {} {}\r\n", self.cursor, cr, cc);
        write!(
            out,
            "{}",
            termion::cursor::Goto(cc as u16 + 1, cr as u16 + 1)
        );
    }
}
