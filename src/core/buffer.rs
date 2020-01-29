use super::Cursor;
use std::io;
use std::io::Error;

use ropey::Rope;

mod ropey_core_buffer;

use failure::_core::ops::RangeBounds;
pub use ropey_core_buffer::RopeyCoreBuffer;

pub trait CoreBuffer: Default + ToString {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self>;
    fn len_lines(&self) -> usize;
    fn len_line(&self, idx_line: usize) -> usize;
    fn char_at(&self, cursor: Cursor) -> Option<char>;
    fn insert_char(&mut self, cursor: Cursor, c: char) {
        self.insert(cursor, c.to_string().as_str());
    }
    fn insert(&mut self, cursor: Cursor, s: &str);

    fn get_range<R: RangeBounds<Cursor>>(&self, range: R) -> String;
    fn delete_range<R: RangeBounds<Cursor>>(&mut self, range: R);
}
