use std::cmp::min;
use std::fmt::Debug;

use crate::core::CoreBuffer;

use crate::core::{Cursor, CursorRange};
use crate::ropey_util::{is_line_end, RopeExt};

pub struct OperationArg<'a, B: CoreBuffer> {
    pub core_buffer: &'a mut B,
    pub cursor: &'a mut Cursor,
}

pub trait Operation<B: CoreBuffer>: Debug {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize>;
    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize>;
}

#[derive(Debug)]
pub struct InsertChar {
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

impl<B: CoreBuffer> Operation<B> for InsertChar {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        arg.core_buffer.insert_char(self.cursor, self.c);
        *arg.cursor = if self.c == '\n' {
            Cursor {
                row: self.cursor.row + 1,
                col: 0,
            }
        } else {
            Cursor {
                row: self.cursor.row,
                col: self.cursor.col + 1,
            }
        };
        Some(self.cursor.row)
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        arg.core_buffer
            .delete_range(CursorRange(self.cursor, self.cursor));
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl<B: CoreBuffer> Operation<B> for Replace {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        /*
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        if self.cursor.col < arg.buffer.l(self.cursor.row).len_chars() {
            self.orig = Some(arg.buffer.l(self.cursor.row).char(self.cursor.col));
            arg.buffer.remove(i..=i);
        }
        arg.buffer.insert_char(i, self.c);
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
        */
        unimplemented!()
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        /*
        let i = arg.buffer.line_to_char(self.cursor.row) + self.cursor.col;
        arg.buffer.remove(i..=i);
        if let Some(orig) = self.orig {
            arg.buffer.insert_char(i, orig);
        }
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
        */
        unimplemented!()
    }
}

impl<B: CoreBuffer> Operation<B> for Delete {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        /*
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
        */
        unimplemented!()
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        /*
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
        */
        unimplemented!()
    }
}

impl<B: CoreBuffer> Operation<B> for DeleteRange {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        self.orig = Some(arg.core_buffer.get_range(self.range));
        arg.core_buffer.delete_range(self.range);
        *arg.cursor = self.range.l();
        Some(self.range.l().row)
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        arg.core_buffer
            .insert(self.range.l(), self.orig.as_ref().unwrap().as_str());
        *arg.cursor = self.range.l();
        Some(self.range.l().row)
    }
}

impl<B: CoreBuffer> Operation<B> for Set {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        self.from = Some(arg.core_buffer.to_string());
        *arg.core_buffer = B::from_reader(self.to.as_bytes()).unwrap();

        let end = Cursor {
            row: arg.core_buffer.len_lines() - 1,
            col: arg.core_buffer.len_line(arg.core_buffer.len_lines() - 1),
        };

        *arg.cursor = std::cmp::min(arg.cursor.clone(), end);
        Some(0)
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        *arg.core_buffer = B::from_reader(self.from.as_ref().unwrap().as_bytes()).unwrap();
        let end = Cursor {
            row: arg.core_buffer.len_lines() - 1,
            col: arg.core_buffer.len_line(arg.core_buffer.len_lines() - 1),
        };

        *arg.cursor = std::cmp::min(arg.cursor.clone(), end);
        Some(0)
    }
}

#[cfg(test)]
mod test {
    use super::InsertChar;
    use crate::core::buffer::RopeyCoreBuffer;
    use crate::core::operation::{DeleteRange, Set};
    use crate::core::{Core, CoreBuffer};
    use crate::core::{Cursor, CursorRange};
    use crate::text_object::Action::Delete;

    #[test]
    fn test_operation_insert_char() {
        let mut core = Core::<RopeyCoreBuffer>::from_reader("".as_bytes()).unwrap();

        core.perform(InsertChar {
            cursor: Cursor { row: 0, col: 0 },
            c: 'A',
        });
        assert_eq!(core.get_string(), "A".to_string());
        assert_eq!(core.cursor, Cursor { row: 0, col: 1 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "".to_string());

        core.perform(InsertChar {
            cursor: Cursor { row: 0, col: 0 },
            c: '\n',
        });
        assert_eq!(core.cursor, Cursor { row: 1, col: 0 });
        assert_eq!(core.get_string(), "\n".to_string());
        assert_eq!(core.core_buffer.len_lines(), 2);
    }

    #[test]
    fn test_operation_delete_range() {
        let mut core = Core::<RopeyCoreBuffer>::from_reader("12345678".as_bytes()).unwrap();

        core.perform(DeleteRange::new(CursorRange(
            Cursor { row: 0, col: 1 },
            Cursor { row: 0, col: 2 },
        )));
        assert_eq!(core.get_string(), "145678".to_string());
        assert_eq!(core.cursor(), Cursor { row: 0, col: 1 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "12345678");
        assert_eq!(core.cursor(), Cursor { row: 0, col: 1 });

        let mut core = Core::<RopeyCoreBuffer>::from_reader("123\n".as_bytes()).unwrap();

        core.perform(DeleteRange::new(CursorRange(
            Cursor { row: 0, col: 0 },
            Cursor { row: 0, col: 3 },
        )));
        assert_eq!(core.get_string(), "".to_string());
        assert_eq!(core.cursor(), Cursor { row: 0, col: 0 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "123\n");
        assert_eq!(core.cursor(), Cursor { row: 0, col: 0 });

        let mut core = Core::<RopeyCoreBuffer>::from_reader("123\n456".as_bytes()).unwrap();

        core.perform(DeleteRange::new(CursorRange(
            Cursor { row: 0, col: 3 },
            Cursor { row: 0, col: 3 },
        )));
        assert_eq!(core.get_string(), "123456".to_string());
        assert_eq!(core.cursor(), Cursor { row: 0, col: 3 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "123\n456");
        assert_eq!(core.cursor(), Cursor { row: 0, col: 3 });
    }

    #[test]
    fn test_operation_set() {
        let mut core = Core::<RopeyCoreBuffer>::from_reader("123456".as_bytes()).unwrap();

        core.cursor_mut().col = 4;
        core.perform(Set::new("abc".to_string()));

        assert_eq!(core.get_string(), "abc".to_string());
        assert_eq!(core.cursor(), Cursor { col: 3, row: 0 });

        core.commit();
        core.undo();

        assert_eq!(core.get_string(), "123456".to_string());
    }
}
