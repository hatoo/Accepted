use std;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::cmp::{max, min};
use std::io;
use std::io::Read;
use std::num::Wrapping;
use std::ops::RangeBounds;

use ropey::{self, Rope, RopeSlice};

use crate::indent;
use crate::parenthesis;
use crate::ropey_util::{is_line_end, RopeExt};

use self::operation::{Operation, OperationArg};

pub mod buffer;
pub mod operation;

pub use buffer::CoreBuffer;

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
    pub fn into_tuple(self) -> (usize, usize) {
        (self.row, self.col)
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.into_tuple().cmp(&other.into_tuple()))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        self.into_tuple().cmp(&other.into_tuple())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CursorRange(Cursor, Cursor);

impl CursorRange {
    pub fn new(l: Cursor, r: Cursor) -> Self {
        CursorRange(l, r)
    }

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

    pub fn contains(&self, cursor: Cursor) -> bool {
        min(self.0, self.1) <= cursor && cursor <= max(self.0, self.1)
    }
}

#[derive(Debug)]
pub struct Core<B: buffer::CoreBuffer> {
    buffer: Rope,
    core_buffer: B,
    cursor: Cursor,
    history: Vec<Vec<Box<dyn Operation<B>>>>,
    history_tmp: Vec<Box<dyn Operation<B>>>,
    redo: Vec<Vec<Box<dyn Operation<B>>>>,
    buffer_changed: Id,
    pub dirty_from: usize,
}

impl<B: buffer::CoreBuffer> Default for Core<B> {
    fn default() -> Self {
        Self {
            buffer: Rope::default(),
            core_buffer: B::default(),
            cursor: Cursor { row: 0, col: 0 },
            history: Vec::new(),
            history_tmp: Vec::new(),
            redo: Vec::new(),
            buffer_changed: Id(Wrapping(1)),
            /// Lines after this are modified
            dirty_from: 0,
        }
    }
}

impl<B: buffer::CoreBuffer> Core<B> {
    pub fn from_reader<T: Read>(reader: T) -> io::Result<Self> {
        Ok(Self {
            buffer: Rope::default(),
            core_buffer: B::from_reader(reader)?,
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
        self.core_buffer.char_at(self.cursor)
    }

    pub fn len_current_line(&self) -> usize {
        self.core_buffer.len_line(self.cursor.row)
    }

    pub fn cursor_left(&mut self) {
        if self.cursor.col != 0 {
            self.cursor.col -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        self.cursor.col = min(
            self.core_buffer.len_line(self.cursor.row),
            self.cursor.col + 1,
        );
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row != 0 {
            self.cursor.row -= 1;
            self.cursor.col = min(self.core_buffer.len_line(self.cursor.row), self.cursor.col);
        }
    }

    pub fn cursor_down(&mut self) {
        self.cursor.row = min(self.core_buffer.len_lines() - 1, self.cursor.row + 1);
        self.cursor.col = min(self.core_buffer.len_line(self.cursor.row), self.cursor.col);
    }

    pub fn cursor_inc(&mut self) -> bool {
        if self.cursor.col < self.core_buffer.len_line(self.cursor.row) {
            self.cursor_right();
            true
        } else if self.cursor.row + 1 < self.core_buffer.len_lines() {
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
            self.cursor.col = self.core_buffer.len_line(self.cursor.row);
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
                col: self.core_buffer.len_line(cursor.row - 1),
            }
        })
    }

    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.row == self.buffer.len_lines() - 1
            && cursor.col == self.core_buffer.len_line(cursor.row)
        {
            return None;
        }

        Some(if cursor.col < self.core_buffer.len_line(cursor.row) {
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
            .map(|c| parenthesis::PARENTHESIS_LEFTS.iter().any(|&p| p == c))
            == Some(true)
        {
            self.cursor_inc();
        } else {
            while {
                self.char_at_cursor()
                    .map(char::is_alphanumeric)
                    .unwrap_or(true)
                    && self.cursor_inc()
            } {}
        }
        while {
            self.char_at_cursor()
                .map(|c| {
                    !c.is_alphanumeric() && !parenthesis::PARENTHESIS_LEFTS.iter().any(|&p| p == c)
                })
                .unwrap_or(true)
                && self.cursor_inc()
        } {}
    }

    pub fn b(&mut self) {
        self.cursor_dec();
        while {
            self.char_at_cursor().map(char::is_alphanumeric) != Some(true) && self.cursor_dec()
        } {}
        while {
            self.char_at_cursor().map(char::is_alphanumeric) == Some(true) && self.cursor_dec()
        } {}
        if self.char_at_cursor().map(char::is_alphanumeric) != Some(true) {
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
            self.char_at_cursor().map(char::is_alphanumeric) != Some(true) && self.cursor_inc()
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
            self.char_at_cursor().map(char::is_alphanumeric) == Some(true) && self.cursor_inc()
        } {
            if self
                .char_at_cursor()
                .map(|c| ['}', ')', ']'].iter().any(|&p| p == c))
                == Some(true)
            {
                return;
            }
        }
        if self.char_at_cursor().map(char::is_alphanumeric) != Some(true) {
            self.cursor_dec();
        }
    }

    pub fn insert(&mut self, c: char) {
        let op = operation::InsertChar {
            cursor: self.cursor,
            c,
        };
        self.perform(op);
    }

    // o
    pub fn insert_newline(&mut self) {
        let op = operation::InsertChar {
            cursor: Cursor {
                row: self.cursor.row,
                col: self.core_buffer.len_line(self.cursor.row),
            },
            c: '\n',
        };
        self.perform(op);
    }

    // O
    pub fn insert_newline_here(&mut self) {
        let op = operation::InsertChar {
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
        self.perform(operation::DeleteRange::new(self.cursor..=self.cursor));
        self.perform(operation::InsertChar {
            cursor: self.cursor,
            c,
        });
    }

    pub fn delete(&mut self) {
        let op = operation::DeleteRange::new(self.cursor..=self.cursor);
        self.perform(op);
    }

    pub fn delete_range<R: RangeBounds<Cursor>>(&mut self, range: R) {
        let op = operation::DeleteRange::new(range);
        self.perform(op);
    }

    pub fn get_string(&self) -> String {
        self.core_buffer.to_string()
    }

    pub fn get_string_range<R: RangeBounds<Cursor>>(&self, range: R) -> String {
        self.core_buffer.get_range(range)
    }

    pub fn set_string(&mut self, s: String, clear_history: bool) {
        if clear_history {
            self.core_buffer = B::from_reader(s.as_bytes()).unwrap();
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

    pub fn core_buffer(&self) -> &B {
        &self.core_buffer
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        assert!(cursor.row < self.core_buffer.len_lines());
        assert!(cursor.col <= self.core_buffer.len_line(cursor.row));
        self.cursor = cursor;
    }

    fn arg(&mut self) -> OperationArg<B> {
        OperationArg {
            core_buffer: &mut self.core_buffer,
            cursor: &mut self.cursor,
        }
    }

    fn perform<T: Operation<B> + 'static>(&mut self, mut op: T) {
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
