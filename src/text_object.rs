use core::{Core, Cursor, CursorRange};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Prefix {
    Inner,
    A,
    None,
}

pub trait TextObject {
    fn get_range(&self, prefix: Prefix, core: &Core) -> CursorRange;
}

struct Word;

impl TextObject for Word {
    fn get_range(&self, prefix: Prefix, core: &Core) -> CursorRange {
        match prefix {
            Prefix::None => {
                let l = core.cursor();
                let line = core.current_line();
                let mut i = l.col;
                while i + 1 < line.len() && line[i + 1].is_alphanumeric() {
                    i += 1;
                }
                CursorRange(l, Cursor { row: l.row, col: i })
            }
            Prefix::A | Prefix::Inner => {
                let pos = core.cursor();
                let line = core.current_line();
                let mut l = pos.col;
                let mut r = pos.col;

                while l > 1 && line[l - 1].is_alphanumeric() {
                    l -= 1;
                }

                while r + 1 < line.len() && line[r + 1].is_alphanumeric() {
                    r += 1;
                }

                CursorRange(
                    Cursor {
                        row: pos.row,
                        col: l,
                    },
                    Cursor {
                        row: pos.row,
                        col: r,
                    },
                )
            }
        }
    }
}

pub struct TextObjectParser {
    pub prefix: Prefix,
    pub object: Option<Box<TextObject>>,
}

impl Default for TextObjectParser {
    fn default() -> Self {
        Self {
            prefix: Prefix::None,
            object: None,
        }
    }
}

impl TextObjectParser {
    pub fn parse(&mut self, c: char) -> bool {
        match c {
            'a' => {
                self.prefix = Prefix::A;
            }
            'i' => {
                self.prefix = Prefix::Inner;
            }
            _ => (),
        }

        if self.object.is_none() {
            if c == 'w' {
                self.object = Some(Box::new(Word));
                return true;
            }
        }
        false
    }

    pub fn get_range(&self, core: &Core) -> Option<CursorRange> {
        self.object
            .as_ref()
            .map(|obj| obj.get_range(self.prefix, core))
    }
}
