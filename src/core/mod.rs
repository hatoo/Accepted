use crate::indent;
use crate::ropey_util::{is_line_end, RopeExt};
use ropey::{self, Rope, RopeSlice};
use std;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::cmp::{max, min};
use std::io;
use std::io::Read;
use std::num::Wrapping;

pub mod operation;

use self::operation::{Operation, OperationArg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Id(Wrapping<usize>);

impl Id {
    pub fn inc(&mut self) {
        self.0 += Wrapping(1);
    }
}

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

    pub fn l_mut(&mut self) -> &mut Cursor {
        if self.0 <= self.1 {
            &mut self.0
        } else {
            &mut self.1
        }
    }

    pub fn r(&self) -> Cursor {
        max(self.0, self.1)
    }

    pub fn r_mut(&mut self) -> &mut Cursor {
        if self.1 >= self.0 {
            &mut self.1
        } else {
            &mut self.0
        }
    }

    pub fn contains(&self, curosor: Cursor) -> bool {
        min(self.0, self.1) <= curosor && curosor <= max(self.0, self.1)
    }
}

#[derive(Debug)]
pub struct Core {
    buffer: Rope,
    cursor: Cursor,
    history: Vec<Vec<Box<Operation>>>,
    history_tmp: Vec<Box<Operation>>,
    redo: Vec<Vec<Box<Operation>>>,
    buffer_changed: Id,
    pub dirty_from: usize,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            buffer: Rope::default(),
            cursor: Cursor { row: 0, col: 0 },
            history: Vec::new(),
            history_tmp: Vec::new(),
            redo: Vec::new(),
            buffer_changed: Id(Wrapping(1)),
            dirty_from: 0,
        }
    }
}

impl Core {
    pub fn from_reader<T: Read>(reader: T) -> io::Result<Self> {
        Ok(Self {
            buffer: Rope::from_reader(reader)?,
            cursor: Cursor { row: 0, col: 0 },
            history: Vec::new(),
            history_tmp: Vec::new(),
            redo: Vec::new(),
            buffer_changed: Id(Wrapping(1)),
            dirty_from: 0,
        })
    }

    pub fn buffer_changed(&self) -> Id {
        self.buffer_changed
    }

    pub fn char_at_cursor(&self) -> Option<char> {
        self.char_at(self.cursor)
    }

    pub fn char_at(&self, cursor: Cursor) -> Option<char> {
        if cursor.row < self.buffer.len_lines() {
            let line = self.buffer.l(cursor.row);
            if cursor.col < line.len_chars() {
                Some(line.char(cursor.col))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn current_line<'a>(&'a self) -> ropey::RopeSlice<'a> {
        self.buffer.l(self.cursor.row)
    }

    pub fn current_line_after_cursor<'a>(&'a self) -> ropey::RopeSlice<'a> {
        self.current_line().slice(self.cursor.col..)
    }

    pub fn cursor_left(&mut self) {
        if self.cursor.col != 0 {
            self.cursor.col -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        self.cursor.col = min(
            self.buffer.l(self.cursor.row).len_chars(),
            self.cursor.col + 1,
        );
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row != 0 {
            self.cursor.row -= 1;
            self.cursor.col = min(self.buffer.l(self.cursor.row).len_chars(), self.cursor.col);
        }
    }

    pub fn cursor_down(&mut self) {
        self.cursor.row = min(self.buffer.len_lines() - 1, self.cursor.row + 1);
        self.cursor.col = min(self.buffer.l(self.cursor.row).len_chars(), self.cursor.col);
    }

    pub fn cursor_inc(&mut self) -> bool {
        if self.cursor.col < self.buffer.l(self.cursor.row).len_chars() {
            self.cursor_right();
            true
        } else if self.cursor.row + 1 < self.buffer.len_lines() {
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
            self.cursor.col = self.buffer.l(self.cursor.row).len_chars();
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
                col: self.buffer.l(cursor.row - 1).len_chars(),
            }
        })
    }

    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.row == self.buffer.len_lines() - 1
            && cursor.col == self.buffer.l(cursor.row).len_chars()
        {
            return None;
        }

        Some(if cursor.col < self.buffer.l(cursor.row).len_chars() {
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

    pub fn indent(&mut self, indent_width: usize) {
        self.cursor.col = 0;
        if self.cursor.row > 0 {
            let indent = indent::next_indent_level(
                &Cow::from(self.buffer.l(self.cursor.row - 1)),
                indent_width,
            );
            for _ in 0..indent_width * indent {
                self.insert(' ');
            }
        }
    }

    pub fn w(&mut self) {
        if self
            .char_at_cursor()
            .map(|c| ['{', '(', '['].iter().any(|&p| p == c))
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
                .map(|c| !c.is_alphanumeric() && !['{', '(', '['].iter().any(|&p| p == c))
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
        if self
            .char_at_cursor()
            .map(|c| ['}', ')', ']'].iter().any(|&p| p == c))
            == Some(true)
        {
            return;
        }
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) && self.cursor_inc()
        } {
            if self
                .char_at_cursor()
                .map(|c| ['}', ')', ']'].iter().any(|&p| p == c))
                == Some(true)
            {
                return;
            }
        }
        while {
            self.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true) && self.cursor_inc()
        } {
            if self
                .char_at_cursor()
                .map(|c| ['}', ')', ']'].iter().any(|&p| p == c))
                == Some(true)
            {
                return;
            }
        }
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
                col: self.buffer.l(self.cursor.row).len_chars(),
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
        let op = operation::DeleteRange::new(range);
        self.perform(op);
    }

    pub fn get_slice_by_range<'a>(&'a self, range: CursorRange) -> RopeSlice<'a> {
        let l = self.buffer.line_to_char(range.l().row) + range.l().col;
        let mut r = self.buffer.line_to_char(range.r().row) + range.r().col;

        if r < self.buffer.len_chars() && range.r().col == self.buffer.l(range.r().row).len_chars()
        {
            while r < self.buffer.len_chars() && self.buffer.char(r) == '\r' {
                r += 1;
            }

            if r < self.buffer.len_chars() && is_line_end(self.buffer.char(r)) {
                r += 1;
            }
        }

        self.buffer.slice(l..r)
    }

    pub fn get_string(&self) -> String {
        String::from(&self.buffer)
    }

    pub fn set_string(&mut self, s: String, clear_history: bool) {
        if clear_history {
            self.buffer = Rope::from(s);
            self.buffer_changed.inc();
            self.dirty_from = 0;
            self.redo.clear();
            self.history.clear();
            self.history_tmp.clear();
        } else {
            let op = operation::Set::new(s);
            self.perform(op);
        }
    }

    pub fn buffer(&self) -> &Rope {
        &self.buffer
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        assert!(cursor.row < self.buffer.len_lines());
        assert!(cursor.col <= self.buffer.l(cursor.row).len_chars());
        self.cursor = cursor;
    }

    fn arg(&mut self) -> OperationArg {
        OperationArg {
            buffer: &mut self.buffer,
            cursor: &mut self.cursor,
        }
    }

    fn perform<T: Operation + 'static>(&mut self, mut op: T) {
        if let Some(l) = op.perform(self.arg()) {
            self.dirty_from = min(self.dirty_from, l);
        }
        self.history_tmp.push(Box::new(op));
        self.redo.clear();
        self.buffer_changed.inc();
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
                if let Some(l) = op.undo(self.arg()) {
                    self.dirty_from = min(self.dirty_from, l);
                }
            }
            self.redo.push(ops);
            self.buffer_changed.inc();
        }
    }

    pub fn redo(&mut self) {
        if let Some(mut ops) = self.redo.pop() {
            for op in &mut ops {
                if let Some(l) = op.perform(self.arg()) {
                    self.dirty_from = min(self.dirty_from, l);
                }
            }
            self.history.push(ops);
            self.buffer_changed.inc();
        }
    }
}
