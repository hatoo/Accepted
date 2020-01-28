use crate::ropey_util::RopeExt;
use ropey::Rope;
use std::io;

use super::CoreBuffer;
use super::Cursor;
use super::CursorRange;

#[derive(Default)]
pub struct RopeyCoreBuffer(Rope);

impl CoreBuffer for RopeyCoreBuffer {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self> {
        Ok(RopeyCoreBuffer(Rope::from_reader(reader)?))
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

    fn get_range(&self, cursor_range: CursorRange) -> String {
        let from = self.0.line_to_char(cursor_range.l().row) + cursor_range.l().col;
        let to = self.0.line_to_char(cursor_range.r().row) + cursor_range.r().col;

        self.0.slice(from..=to).to_string()
    }

    fn delete_range(&mut self, cursor_range: CursorRange) {
        let from = self.0.line_to_char(cursor_range.l().row) + cursor_range.l().col;
        let to = self.0.line_to_char(cursor_range.r().row) + cursor_range.r().col;

        self.0.remove(from..=to);
    }
}

impl ToString for RopeyCoreBuffer {
    fn to_string(&self) -> String {
        String::from(&self.0)
    }
}
