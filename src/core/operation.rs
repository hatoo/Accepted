use crate::core::{Cursor, CursorRange};
use crate::ropey_util::{is_line_end, RopeExt};
use ropey::Rope;
use std::cmp::min;
use std::fmt::Debug;

pub struct OperationArg<'a> {
    pub buffer: &'a mut Rope,
    pub cursor: &'a mut Cursor,
}

pub trait Operation: Debug {
    fn perform(&mut self, arg: OperationArg) -> Option<usize>;
    fn undo(&mut self, arg: OperationArg) -> Option<usize>;
}

#[derive(Debug)]
pub struct Insert {
    pub cursor: Cursor,
    pub c: char,
}

#[derive(Debug)]
pub struct Replace {
    pub cursor: Cursor,
    pub c: char,
    pub orig: Option<char>,
}

impl Replace {
    pub fn new(cursor: Cursor, c: char) -> Self {
        Self {
            cursor,
            c,
            orig: None,
        }
    }
}

#[derive(Debug)]
pub struct Delete {
    pub cursor: Cursor,
    pub orig: Option<char>,
    pub done: bool,
}

impl Delete {
    pub fn new(cursor: Cursor) -> Self {
        Self {
            cursor,
            orig: None,
            done: false,
        }
    }
}

#[derive(Debug)]
pub struct DeleteRange {
    pub range: CursorRange,
    orig: Option<String>,
}

impl DeleteRange {
    pub fn new(range: CursorRange) -> Self {
        Self { range, orig: None }
    }
}

#[derive(Debug)]
pub struct Set {
    to: String,
    from: Option<String>,
}

impl Set {
    pub fn new(to: String) -> Self {
        Self { to, from: None }
    }
}

impl Operation for Insert {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        arg.buffer.insert_char(i, self.c);
        let mut cursor = self.cursor;
        if self.c == '\n' {
            cursor.row += 1;
            cursor.col = 0;
        } else {
            cursor.col += 1;
        }
        *arg.cursor = cursor;
        Some(self.cursor.row)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        arg.buffer.remove(i..=i);
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for Replace {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        if self.cursor.col < arg.buffer.l(self.cursor.row).len_chars() {
            self.orig = Some(arg.buffer.l(self.cursor.row).char(self.cursor.col));
            arg.buffer.remove(i..=i);
        }
        arg.buffer.insert_char(i, self.c);
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        arg.buffer.remove(i..=i);
        if let Some(orig) = self.orig {
            arg.buffer.insert_char(i, orig);
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for Delete {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;

        if self.cursor.col < arg.buffer.l(self.cursor.row).len_chars() {
            self.orig = Some(arg.buffer.char(i));
            arg.buffer.remove(i..=i);
            self.done = true;
        } else if self.cursor.row + 1 < arg.buffer.len_lines() {
            while i < arg.buffer.len_chars() && is_line_end(arg.buffer.char(i)) {
                arg.buffer.remove(i..=i);
            }
            self.done = true;
        } else {
            self.done = false;
        }
        *arg.cursor = self.cursor;
        if self.done {
            Some(self.cursor.row)
        } else {
            None
        }
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        if !self.done {
            return None;
        }

        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;

        if let Some(orig) = self.orig {
            arg.buffer.insert_char(i, orig);
        } else {
            arg.buffer.insert_char(i, '\n');
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for DeleteRange {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        let l = arg.buffer.line_to_char(self.range.l().row) + self.range.l().col;
        let mut r = arg.buffer.line_to_char(self.range.r().row) + self.range.r().col;

        if r < arg.buffer.len_chars() {
            if self.range.r().col == arg.buffer.l(self.range.r().row).len_chars() {
                while r < arg.buffer.len_chars() && arg.buffer.char(r) == '\r' {
                    r += 1;
                }

                if r < arg.buffer.len_chars() && is_line_end(arg.buffer.char(r)) {
                    r += 1;
                }
            } else {
                r += 1;
            }
        }

        self.orig = Some(String::from(arg.buffer.slice(l..r)));
        arg.buffer.remove(l..r);
        *arg.cursor = self.range.l();
        Some(self.range.l().row)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        let l = arg.buffer.line_to_char(self.range.l().row) + self.range.l().col;

        arg.buffer.insert(l, self.orig.as_ref().unwrap().as_str());
        *arg.cursor = self.range.l();
        Some(self.range.l().row)
    }
}
impl Operation for Set {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        if self.from.is_none() {
            self.from = Some(String::from(arg.buffer.slice(..)));
        }

        *arg.buffer = Rope::from(self.to.as_str());
        arg.cursor.row = min(arg.buffer.len_lines() - 1, arg.cursor.row);
        arg.cursor.col = min(arg.buffer.l(arg.cursor.row).len_chars(), arg.cursor.col);
        Some(0)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        *arg.buffer = Rope::from(self.from.as_ref().unwrap().as_str());
        arg.cursor.row = min(arg.buffer.len_lines() - 1, arg.cursor.row);
        arg.cursor.col = min(arg.buffer.l(arg.cursor.row).len_chars(), arg.cursor.col);
        Some(0)
    }
}
