extern crate termion;

use std::cmp::{max, min};
use std::io::{stdin, stdout, Write};

#[derive(Debug)]
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

impl Window {
    pub fn new() -> Self {
        Window {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, col: 0 },
            row_offset: 0,
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor.col != 0 {
            self.cursor.col -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col + 1);
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row != 0 {
            self.cursor.row -= 1;
            self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col);
        }
    }

    pub fn cursor_down(&mut self) {
        self.cursor.row = min(self.buffer.len() - 1, self.cursor.row + 1);
        self.cursor.col = min(self.buffer[self.cursor.row].len(), self.cursor.col);
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
    }

    pub fn draw<T: Write>(&self, w: &mut T) {
        refresh_screen(w);

        let (cols, rows) = termion::terminal_size().unwrap();

        for i in 0..rows as usize {
            if i < self.buffer.len() {
                write!(
                    w,
                    "{}\r\n",
                    self.buffer[i]
                        .iter()
                        .take(cols as usize)
                        .collect::<String>()
                );
            } else {
                write!(w, "~");
                if i != rows as usize - 1 {
                    write!(w, "\r\n");
                }
            }
        }

        write!(
            w,
            "{}",
            termion::cursor::Goto(self.cursor.col as u16 + 1, self.cursor.row as u16 + 1)
        );
    }
}
