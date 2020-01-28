use super::Cursor;
use std::io;
use std::io::Error;

use ropey::Rope;

mod ropey_core_buffer;

use crate::core::CursorRange;
pub use ropey_core_buffer::RopeyCoreBuffer;

pub trait CoreBuffer: Default + ToString {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self>;
    fn len_lines(&self) -> usize;
    fn char_at(&self, cursor: Cursor) -> Option<char>;
    fn insert_char(&mut self, cursor: Cursor, c: char);
    fn delete_range(&mut self, cursor_range: CursorRange);
}
