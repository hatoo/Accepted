use ropey::{Rope, RopeSlice};

pub trait RopeSliceExt {
    fn trim_end(self) -> Self;
}

pub trait RopeExt {
    fn l(&self, line_idx: usize) -> RopeSlice;
}

pub fn is_line_end(c: char) -> bool {
    [
        '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}', '\u{0085}', '\u{2028}', '\u{2029}',
    ]
    .contains(&c)
}

impl<'a> RopeSliceExt for RopeSlice<'a> {
    fn trim_end(self) -> Self {
        let mut i = self.len_chars();
        while i > 0 && is_line_end(self.char(i - 1)) {
            i -= 1;
        }

        self.slice(..i)
    }
}

impl RopeExt for Rope {
    fn l(&self, line_idx: usize) -> RopeSlice {
        self.line(line_idx).trim_end()
    }
}
