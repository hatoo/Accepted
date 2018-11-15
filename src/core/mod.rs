use indent;
use rustfmt;
use std;
use std::cmp::Ordering;
use std::cmp::{max, min};
use std::num::Wrapping;

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
    pub buffer_changed: Wrapping<usize>,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, col: 0 },
            history: Vec::new(),
            history_tmp: Vec::new(),
            redo: Vec::new(),
            buffer_changed: Wrapping(1),
        }
    }
}

impl Core {
    pub fn char_at_cursor(&self) -> Option<char> {
        self.buffer
            .get(self.cursor.row)
            .and_then(|line| line.get(self.cursor.col).cloned())
    }

    pub fn char_at(&self, cursor: Cursor) -> Option<char> {
        self.buffer
            .get(cursor.row)
            .and_then(|line| line.get(cursor.col).cloned())
    }

    pub fn current_line(&self) -> &[char] {
        &self.buffer[self.cursor.row]
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

    pub fn prev_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.row == 0 && cursor.col == 0 {
            return None;
        }

        Some(if cursor.col > 0 {
            Cursor {
                row: cursor.row,
                col: cursor.col - 1,
            }
        } else {
            Cursor {
                row: cursor.row - 1,
                col: self.buffer[cursor.row - 1].len(),
            }
        })
    }

    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.row == self.buffer.len() - 1 && cursor.col == self.buffer[cursor.row].len() {
            return None;
        }

        Some(if cursor.col < self.buffer[cursor.row].len() {
            Cursor {
                row: cursor.row,
                col: cursor.col + 1,
            }
        } else {
            Cursor {
                row: cursor.row + 1,
                col: 0,
            }
        })
    }

    pub fn indent(&mut self) {
        self.cursor.col = 0;
        if self.cursor.row > 0 {
            let indent = indent::next_indent_level(&self.buffer[self.cursor.row - 1]);
            for _ in 0..4 * indent {
                self.insert(' ');
            }
        }
    }

    pub fn w(&mut self) {
        if self
            .char_at_cursor()
            .map(|c| ['{', '(', '['].into_iter().any(|&p| p == c))
            == Some(true)
        {
            self.cursor_inc();
        } else {
            while {
                self.char_at_cursor()
                    .map(|c| c.is_alphanumeric())
                    .unwrap_or(true)
                    && self.cursor_inc()
            } {}
        }
        while {
            self.char_at_cursor()
                .map(|c| !c.is_alphanumeric() && !['{', '(', '['].into_iter().any(|&p| p == c))
                .unwrap_or(true)
                && self.cursor_inc()
        } {}
    }

    pub fn b(&mut self) {
        self.cursor_dec();
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) && self.cursor_dec()
        } {}
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true) && self.cursor_dec()
        } {}
        if self.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
            self.cursor_inc();
        }
    }

    pub fn e(&mut self) {
        self.cursor_inc();
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) && self.cursor_inc()
        } {}
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true) && self.cursor_inc()
        } {}
        if self.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
            self.cursor_dec();
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

    // O
    pub fn insert_newline_here(&mut self) {
        let op = operation::Insert {
            cursor: Cursor {
                row: self.cursor.row,
                col: 0,
            },
            c: '\n',
        };
        self.perform(op);
        self.cursor_up();
    }

    pub fn replace(&mut self, c: char) {
        let op = operation::Replace::new(self.cursor, c);
        self.perform(op);
    }

    pub fn delete(&mut self) {
        let op = operation::Delete::new(self.cursor);
        self.perform(op);
    }

    pub fn delete_range(&mut self, range: CursorRange) {
        let l = range.l();
        let r = range.r();
        self.set_cursor(l);
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
    }

    pub fn get_string_by_range(&self, range: CursorRange) -> String {
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

    pub fn rustfmt(&mut self) {
        let src = self.get_string();
        if let Some(formatted) = rustfmt::system_rustfmt(&src) {
            if formatted != src {
                self.set_string(&formatted, false);
            }
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
    }

    pub fn redo(&mut self) {
        if let Some(mut ops) = self.redo.pop() {
            for op in &mut ops {
                op.perform(self.arg());
            }
            self.history.push(ops);
            self.buffer_changed += Wrapping(1);
        }
    }
}
