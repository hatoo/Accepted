use super::Cursor;
use std::io;
use std::ops::RangeBounds;

mod ropey_core_buffer;

pub use ropey_core_buffer::RopeyCoreBuffer;

pub trait CoreBuffer: Default + ToString + Send {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self>;
    fn len_bytes(&self) -> usize;
    fn len_lines(&self) -> usize;
    fn len_line(&self, idx_line: usize) -> usize;
    fn char_at(&self, cursor: Cursor) -> Option<char>;
    fn insert_char(&mut self, cursor: Cursor, c: char) {
        self.insert(cursor, c.to_string().as_str());
    }
    fn insert(&mut self, cursor: Cursor, s: &str);

    fn get_range<R: RangeBounds<Cursor>>(&self, range: R) -> String;
    fn delete_range<R: RangeBounds<Cursor>>(&mut self, range: R);
    fn bytes_range<'a, R: RangeBounds<Cursor>>(
        &'a self,
        range: R,
    ) -> Box<dyn Iterator<Item = u8> + 'a>;

    fn cursor_to_bytes(&self, cursor: Cursor) -> usize;
    fn bytes_to_cursor(&self, bytes_idx: usize) -> Cursor;

    fn write_to<W: io::Write>(&self, write: &mut W) -> io::Result<()>;

    fn end_cursor(&self) -> Cursor {
        let row = self.len_lines() - 1;
        let col = self.len_line(row);

        Cursor { row, col }
    }
}
