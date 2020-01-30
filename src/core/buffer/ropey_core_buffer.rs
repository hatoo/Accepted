use ropey::Rope;
use std::io;

use super::CoreBuffer;
use super::Cursor;
use std::io::Error;
use std::ops::{Bound, RangeBounds};

mod ropey_util;

use ropey_util::RopeExt;

#[derive(Default)]
pub struct RopeyCoreBuffer(Rope);

impl CoreBuffer for RopeyCoreBuffer {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self> {
        Ok(RopeyCoreBuffer(Rope::from_reader(reader)?))
    }

    fn len_bytes(&self) -> usize {
        self.0.len_bytes()
    }

    fn len_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn len_line(&self, idx_line: usize) -> usize {
        self.0.l(idx_line).len_chars()
    }

    fn char_at(&self, cursor: Cursor) -> Option<char> {
        if cursor.row < self.0.len_lines() {
            let line = self.0.l(cursor.row);
            if cursor.col < line.len_chars() {
                Some(line.char(cursor.col))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn insert_char(&mut self, cursor: Cursor, c: char) {
        let i = self.0.line_to_char(cursor.row) + cursor.col;
        self.0.insert_char(i, c);
    }

    fn insert(&mut self, cursor: Cursor, s: &str) {
        let i = self.0.line_to_char(cursor.row) + cursor.col;
        self.0.insert(i, s);
    }

    fn get_range<R: RangeBounds<Cursor>>(&self, range: R) -> String {
        let ropey_range = map_range(range, |c| self.0.line_to_char(c.row) + c.col);

        self.0.slice(ropey_range).to_string()
    }

    fn delete_range<R: RangeBounds<Cursor>>(&mut self, range: R) {
        let ropey_range = map_range(range, |c| self.0.line_to_char(c.row) + c.col);

        self.0.remove(ropey_range);
    }

    fn bytes_range<'a, R: RangeBounds<Cursor>>(
        &'a self,
        range: R,
    ) -> Box<dyn Iterator<Item = u8> + 'a> {
        let ropey_range = map_range(range, |c| self.0.line_to_char(c.row) + c.col);

        Box::new(self.0.slice(ropey_range).bytes())
    }

    fn cursor_to_bytes(&self, cursor: Cursor) -> usize {
        self.0
            .char_to_byte(self.0.line_to_char(cursor.row) + cursor.col)
    }

    fn bytes_to_cursor(&self, bytes_idx: usize) -> Cursor {
        let row = self.0.byte_to_line(bytes_idx);
        let col = self.0.byte_to_char(bytes_idx) - self.0.line_to_char(row);

        Cursor { row, col }
    }

    fn write_to<W: io::Write>(&self, write: &mut W) -> Result<(), Error> {
        self.0.write_to(write)
    }
}

impl ToString for RopeyCoreBuffer {
    fn to_string(&self) -> String {
        String::from(&self.0)
    }
}

fn map_bound<T1, T2, F: Fn(&T1) -> T2>(bound: Bound<&T1>, f: F) -> Bound<T2> {
    match bound {
        Bound::Unbounded => Bound::Unbounded,
        Bound::Excluded(t) => Bound::Excluded(f(t)),
        Bound::Included(t) => Bound::Included(f(t)),
    }
}

fn map_range<T1: Sized, T2: Sized, R1: RangeBounds<T1>, F: Fn(&T1) -> T2>(
    range: R1,
    f: F,
) -> (Bound<T2>, Bound<T2>) {
    (
        map_bound(range.start_bound(), &f),
        map_bound(range.end_bound(), &f),
    )
}
