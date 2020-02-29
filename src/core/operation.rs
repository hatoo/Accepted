use std::fmt::Debug;

use crate::core::CoreBuffer;

use crate::core::Cursor;
use std::ops::{Bound, RangeBounds};

pub struct OperationArg<'a, B: CoreBuffer> {
    pub core_buffer: &'a mut B,
    pub cursor: &'a mut Cursor,
}

pub trait Operation<B: CoreBuffer>: Debug + Send {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize>;
    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize>;
}

#[derive(Debug)]
pub struct InsertChar {
    pub cursor: Cursor,
    pub c: char,
}

#[derive(Debug)]
pub struct DeleteRange {
    range: (Bound<Cursor>, Bound<Cursor>),
    orig: Option<String>,
}

impl DeleteRange {
    pub fn new<R: RangeBounds<Cursor>>(range: R) -> Self {
        Self {
            range: (range.start_bound().cloned(), range.end_bound().cloned()),
            orig: None,
        }
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
        arg.core_buffer.delete_range(self.cursor..=self.cursor);
        *arg.cursor = self.cursor;
        Some(self.cursor.row)
    }
}

impl<B: CoreBuffer> Operation<B> for DeleteRange {
    fn perform(&mut self, arg: OperationArg<B>) -> Option<usize> {
        self.orig = Some(arg.core_buffer.get_range(self.range));
        arg.core_buffer.delete_range(self.range);
        *arg.cursor = match self.range.start_bound() {
            Bound::Included(&c) => c,
            Bound::Excluded(&c) => c,
            Bound::Unbounded => Cursor { row: 0, col: 0 },
        };
        Some(match self.range.start_bound() {
            Bound::Included(c) => c.row,
            Bound::Excluded(c) => c.row,
            Bound::Unbounded => 0,
        })
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        let l = match self.range.start_bound() {
            Bound::Included(&c) => c,
            Bound::Excluded(&c) => c,
            Bound::Unbounded => Cursor { row: 0, col: 0 },
        };

        arg.core_buffer
            .insert(l, self.orig.as_ref().unwrap().as_str());
        *arg.cursor = l;
        Some(l.row)
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

        *arg.cursor = std::cmp::min(*arg.cursor, end);
        Some(0)
    }

    fn undo(&mut self, arg: OperationArg<B>) -> Option<usize> {
        *arg.core_buffer = B::from_reader(self.from.as_ref().unwrap().as_bytes()).unwrap();
        let end = Cursor {
            row: arg.core_buffer.len_lines() - 1,
            col: arg.core_buffer.len_line(arg.core_buffer.len_lines() - 1),
        };

        *arg.cursor = std::cmp::min(*arg.cursor, end);
        Some(0)
    }
}

#[cfg(test)]
mod test {
    use super::InsertChar;
    use crate::core::buffer::RopeyCoreBuffer;
    use crate::core::operation::{DeleteRange, Set};
    use crate::core::Cursor;
    use crate::core::{Core, CoreBuffer};

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

        core.perform(DeleteRange::new(
            Cursor { row: 0, col: 1 }..=Cursor { row: 0, col: 2 },
        ));
        assert_eq!(core.get_string(), "145678".to_string());
        assert_eq!(core.cursor(), Cursor { row: 0, col: 1 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "12345678");
        assert_eq!(core.cursor(), Cursor { row: 0, col: 1 });

        let mut core = Core::<RopeyCoreBuffer>::from_reader("123\n".as_bytes()).unwrap();

        core.perform(DeleteRange::new(
            Cursor { row: 0, col: 0 }..=Cursor { row: 0, col: 3 },
        ));
        assert_eq!(core.get_string(), "".to_string());
        assert_eq!(core.cursor(), Cursor { row: 0, col: 0 });
        core.commit();
        core.undo();
        assert_eq!(core.get_string(), "123\n");
        assert_eq!(core.cursor(), Cursor { row: 0, col: 0 });

        let mut core = Core::<RopeyCoreBuffer>::from_reader("123\n456".as_bytes()).unwrap();

        core.perform(DeleteRange::new(
            Cursor { row: 0, col: 3 }..=Cursor { row: 0, col: 3 },
        ));
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

        core.set_cursor(Cursor { row: 0, col: 4 });
        core.perform(Set::new("abc".to_string()));

        assert_eq!(core.get_string(), "abc".to_string());
        assert_eq!(core.cursor(), Cursor { col: 3, row: 0 });

        core.commit();
        core.undo();

        assert_eq!(core.get_string(), "123456".to_string());
    }
}
