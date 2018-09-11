use std::cmp::Ordering;
use std::cmp::{max, min};
use std::num::Wrapping;
use termion;
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    fn to_tuple(&self) -> (usize, usize) {
        (self.row, self.col)
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.to_tuple().cmp(&other.to_tuple()))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        self.to_tuple().cmp(&other.to_tuple())
    }
}

pub struct CursorRange(pub Cursor, pub Cursor);

impl CursorRange {
    pub fn l(&self) -> Cursor {
        min(self.0, self.1)
    }

    pub fn r(&self) -> Cursor {
        max(self.0, self.1)
    }

    pub fn contains(&self, curosor: Cursor) -> bool {
        min(self.0, self.1) <= curosor && curosor <= max(self.0, self.1)
    }
}

#[derive(Debug, Clone)]
pub struct Core {
    pub buffer: Vec<Vec<char>>,
    pub cursor: Cursor,
    pub row_offset: usize,
    pub buffer_changed: Wrapping<usize>,
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

impl Core {
    fn windows_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    pub fn new() -> Self {
        Self {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, col: 0 },
            row_offset: 0,
            buffer_changed: Wrapping(0),
        }
    }

    pub fn char_at_cursor(&self) -> Option<char> {
        self.buffer
            .get(self.cursor.row)
            .and_then(|line| line.get(self.cursor.col).cloned())
    }

    pub fn current_line(&self) -> &[char] {
        &self.buffer[self.cursor.row]
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

    pub fn cursor_inc(&mut self) -> bool {
        if self.cursor.col < self.buffer[self.cursor.row].len() {
            self.cursor_right();
            true
        } else {
            if self.cursor.row + 1 < self.buffer.len() {
                self.cursor.row += 1;
                self.cursor.col = 0;
                true
            } else {
                false
            }
        }
    }

    pub fn cursor_dec(&mut self) -> bool {
        if self.cursor.row == 0 && self.cursor.col == 0 {
            return false;
        }

        if self.cursor.col == 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.buffer[self.cursor.row].len();
        } else {
            self.cursor.col -= 1;
        }

        true
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
        self.buffer_changed += Wrapping(1);
    }

    pub fn insert_newline(&mut self) {
        self.buffer.insert(self.cursor.row + 1, Vec::new());
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.buffer_changed += Wrapping(1);
    }

    pub fn replace(&mut self, c: char) {
        if self.cursor.col == self.buffer[self.cursor.row].len() {
            self.buffer[self.cursor.row].push(c);
        } else {
            self.buffer[self.cursor.row][self.cursor.col] = c;
        }
        self.buffer_changed += Wrapping(1);
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
        self.buffer_changed += Wrapping(1);
    }

    pub fn delete_from_cursor(&mut self, to: Cursor) {
        let range = CursorRange(self.cursor, to);
        let l = range.l();
        let r = range.r();
        if l.row == r.row {
            if r.col == self.buffer[l.row].len() {
                self.buffer[l.row].drain(l.col..);
                if l.row + 1 < self.buffer.len() {
                    let mut line = self.buffer.remove(l.row + 1);
                    self.buffer[l.row].append(&mut line);
                }
            } else {
                self.buffer[l.row].drain(l.col..r.col + 1);
            }
        } else {
            if l.col == 0 && r.col == self.buffer[r.row].len() {
                self.buffer.drain(l.row..r.row + 1);
            } else {
                self.buffer.drain(l.row + 1..r.row);
                self.buffer[l.row].drain(l.col..);
                let rr = self.buffer.remove(l.row + 1);
                if r.col < rr.len() {
                    self.buffer[l.row].extend(rr[r.col + 1..].into_iter());
                }
            }
        }
        self.cursor = l;
        self.buffer_changed += Wrapping(1);
    }
}
