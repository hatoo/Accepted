use crate::core::Cursor;
use std::cmp::min;
use std::fmt::Debug;

pub struct OperationArg<'a> {
    pub buffer: &'a mut Vec<Vec<char>>,
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
pub struct Set {
    to: Vec<Vec<char>>,
    from: Option<Vec<Vec<char>>>,
}

impl Set {
    pub fn new(mut to: Vec<Vec<char>>) -> Self {
        if to.is_empty() {
            to = vec![Vec::new()];
        }
        Self { to, from: None }
    }
}

impl Operation for Insert {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        let mut cursor = self.cursor;
        if self.c == '\n' {
            let rest: Vec<char> = arg.buffer[cursor.row].drain(cursor.col..).collect();

            arg.buffer.insert(cursor.row + 1, rest);
            cursor.row += 1;
            cursor.col = 0;
        } else {
            arg.buffer[cursor.row].insert(cursor.col, self.c);
            cursor.col += 1;
        }
        *arg.cursor = cursor;
        Some(self.cursor.row)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        if self.c == '\n' {
            let mut line = arg.buffer.remove(self.cursor.row + 1);
            arg.buffer[self.cursor.row].append(&mut line);
        } else {
            arg.buffer[self.cursor.row].remove(self.cursor.col);
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for Replace {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        if self.cursor.col < arg.buffer[self.cursor.row].len() {
            self.orig = Some(arg.buffer[self.cursor.row][self.cursor.col]);
            arg.buffer[self.cursor.row][self.cursor.col] = self.c;
        } else {
            arg.buffer[self.cursor.row].push(self.c);
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        if let Some(orig) = self.orig {
            arg.buffer[self.cursor.row][self.cursor.col] = orig;
        } else {
            arg.buffer[self.cursor.row].pop();
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for Delete {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        if self.cursor.col < arg.buffer[self.cursor.row].len() {
            self.orig = Some(arg.buffer[self.cursor.row].remove(self.cursor.col));
            self.done = true;
        } else if self.cursor.row + 1 < arg.buffer.len() {
            let mut line = arg.buffer.remove(self.cursor.row + 1);
            arg.buffer[self.cursor.row].append(&mut line);
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

        if let Some(orig) = self.orig {
            arg.buffer[self.cursor.row].insert(self.cursor.col, orig);
        } else {
            let line = arg.buffer[self.cursor.row]
                .drain(self.cursor.col..)
                .collect();
            arg.buffer.insert(self.cursor.row + 1, line);
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl Operation for Set {
    fn perform(&mut self, arg: OperationArg) -> Option<usize> {
        if self.from.is_none() {
            self.from = Some(arg.buffer.clone());
        }

        arg.buffer.clone_from(&self.to);
        arg.cursor.row = min(arg.buffer.len() - 1, arg.cursor.row);
        arg.cursor.col = min(arg.buffer[arg.cursor.row].len(), arg.cursor.col);
        Some(0)
    }

    fn undo(&mut self, arg: OperationArg) -> Option<usize> {
        arg.buffer.clone_from(self.from.as_ref().unwrap());
        arg.cursor.row = min(arg.buffer.len() - 1, arg.cursor.row);
        arg.cursor.col = min(arg.buffer[arg.cursor.row].len(), arg.cursor.col);
        Some(0)
    }
}
