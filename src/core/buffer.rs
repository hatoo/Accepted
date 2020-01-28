use super::Cursor;
use std::io;
use std::io::Error;

use ropey::Rope;

mod ropey_core_buffer;

pub use ropey_core_buffer::RopeyCoreBuffer;

pub trait CoreBuffer: Default {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self>;
    fn len_lines(&self) -> usize;
    fn line<'a>(&'a self, line_idx: usize) -> Box<dyn ExactSizeIterator<Item = char> + 'a>;
    fn char_at(&self, cursor: Cursor) -> Option<char>;
}

pub trait Line {
    fn len(&self) -> usize;
}
