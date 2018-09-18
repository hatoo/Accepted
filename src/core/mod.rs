use indent;
use std;
use std::cmp::Ordering;
use std::cmp::{max, min};
use std::num::Wrapping;
use termion;
use unicode_width::UnicodeWidthChar;

pub mod operation;

use self::operation::{Operation, OperationArg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    pub fn to_tuple(&self) -> (usize, usize) {
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug)]
pub struct Core {
    buffer: Vec<Vec<char>>,
    cursor: Cursor,
    history: Vec<Vec<Box<Operation>>>,
    history_tmp: Vec<Box<Operation>>,
    redo: Vec<Vec<Box<Operation>>>,
    // TODO: Consider to move this to Buffer.
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

impl Default for Core {
    fn default() -> Self {
        Self {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, col: 0 },
            row_offset: 0,
            history: Vec::new(),
            history_tmp: Vec::new(),
            redo: Vec::new(),
            buffer_changed: Wrapping(1),
        }
    }
}

impl Core {
    fn windows_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    pub fn char_at_cursor(&self) -> Option<char> {
        self.buffer
            .get(self.cursor.row)
            .and_then(|line| line.get(self.cursor.col).cloned())
    }

    pub fn char_ar(&self, cursor: Cursor) -> Option<char> {
        self.buffer
            .get(cursor.row)
            .and_then(|line| line.get(cursor.col).cloned())
    }

    pub fn current_line(&self) -> &[char] {
        &self.buffer[self.cursor.row]
    }

    pub fn set_offset(&mut self) {
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
        } else if self.cursor.row + 1 < self.buffer.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
            true
        } else {
            false
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

    pub fn indent(&mut self) {
        self.cursor.col = 0;
        if !self.cursor.row > 0 {
            let indent = indent::next_indent_level(&self.buffer[self.cursor.row - 1]);
            for _ in 0..4 * indent {
                self.insert(' ');
            }
        }
    }

    pub fn insert(&mut self, c: char) {
        let op = operation::Insert {
            cursor: self.cursor,
            c,
        };
        self.perform(op);
    }

    // o
    pub fn insert_newline(&mut self) {
        let op = operation::Insert {
            cursor: Cursor {
                row: self.cursor.row,
                col: self.buffer[self.cursor.row].len(),
            },
            c: '\n',
        };
        self.perform(op);
    }

    pub fn replace(&mut self, c: char) {
        let op = operation::Replace::new(self.cursor, c);
        self.perform(op);
    }

    pub fn delete(&mut self) {
        let op = operation::Delete::new(self.cursor);
        self.perform(op);
    }

    pub fn delete_from_cursor(&mut self, to: Cursor) {
        let range = CursorRange(self.cursor, to);
        let l = range.l();
        let r = range.r();
        let mut t = l;
        let mut cnt = 0;
        while t != r {
            if t.row < r.row {
                cnt += self.buffer[t.row].len() - t.col + 1;
                t.col = 0;
                t.row += 1;
            } else {
                cnt += r.col - t.col;
                t.col = r.col;
            }
        }
        for _ in 0..=cnt {
            let op = operation::Delete::new(l);
            self.perform(op);
        }
        self.set_offset();
    }

    pub fn get_string_by_range(&mut self, range: CursorRange) -> String {
        let mut res = String::new();
        let mut l = range.l();
        let r = range.r();

        while l.row < r.row {
            for &c in &self.buffer[l.row][l.col..] {
                res.push(c);
            }
            res.push('\n');
            l.row += 1;
            l.col = 0;
        }
        if r.col == self.buffer[r.row].len() {
            for &c in &self.buffer[l.row][l.col..r.col] {
                res.push(c);
            }
            res.push('\n');
        } else {
            for &c in &self.buffer[l.row][l.col..=r.col] {
                res.push(c);
            }
        }
        res
    }

    pub fn get_string(&self) -> String {
        let mut buf = String::new();
        for line in &self.buffer {
            for &c in line {
                buf.push(c);
            }
            // Supports LF only...
            buf.push('\n');
        }

        buf
    }

    pub fn set_string(&mut self, s: &str, clear_history: bool) {
        let mut buffer: Vec<Vec<char>> = s
            .lines()
            .map(|l| l.trim_right().chars().collect())
            .collect();

        if buffer.is_empty() {
            buffer = vec![Vec::new()];
        }

        let op = operation::Set::new(buffer);
        self.perform(op);

        if clear_history {
            self.redo.clear();
            self.history.clear();
            self.history_tmp.clear();
        }
    }

    pub fn buffer(&self) -> &Vec<Vec<char>> {
        &self.buffer
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        assert!(cursor.row < self.buffer.len());
        assert!(cursor.col <= self.buffer[cursor.row].len());
        self.cursor = cursor;
    }

    fn arg(&mut self) -> OperationArg {
        OperationArg {
            buffer: &mut self.buffer,
            cursor: &mut self.cursor,
        }
    }

    fn perform<T: Operation + 'static>(&mut self, mut op: T) {
        op.perform(self.arg());
        self.history_tmp.push(Box::new(op));
        self.redo.clear();
        self.buffer_changed += Wrapping(1);
    }

    pub fn commit(&mut self) {
        if !self.history_tmp.is_empty() {
            let mut h = Vec::new();
            std::mem::swap(&mut self.history_tmp, &mut h);
            self.history.push(h);
        }
    }

    pub fn undo(&mut self) {
        self.commit();
        if let Some(mut ops) = self.history.pop() {
            for op in ops.iter_mut().rev() {
                op.undo(self.arg());
            }
            self.redo.push(ops);
            self.buffer_changed += Wrapping(1);
        }
        self.set_offset();
    }

    pub fn redo(&mut self) {
        if let Some(mut ops) = self.redo.pop() {
            for op in &mut ops {
                op.perform(self.arg());
            }
            self.history.push(ops);
            self.buffer_changed += Wrapping(1);
        }
        self.set_offset();
    }
}
