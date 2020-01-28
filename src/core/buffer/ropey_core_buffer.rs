use crate::ropey_util::RopeExt;
use ropey::Rope;
use std::io;

use super::CoreBuffer;
use super::Cursor;

#[derive(Default)]
pub struct RopeyCoreBuffer(Rope);

impl CoreBuffer for RopeyCoreBuffer {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self> {
        Ok(RopeyCoreBuffer(Rope::from_reader(reader)?))
    }

    fn len_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn line<'a>(&'a self, line_idx: usize) -> Box<dyn ExactSizeIterator<Item = char> + 'a> {
        Box::new(self.0.l(line_idx).chars())
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
}
