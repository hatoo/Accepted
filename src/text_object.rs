use core::{Core, Cursor, CursorRange};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Prefix {
    Inner,
    A,
    None,
}

pub trait TextObject {
    fn get_range(&self, prefix: Prefix, core: &Core) -> Option<CursorRange>;
}

struct Word;
struct Quote(char);
struct Parens(char, char);

impl TextObject for Quote {
    fn get_range(&self, prefix: Prefix, core: &Core) -> Option<CursorRange> {
        match prefix {
            Prefix::A | Prefix::Inner => {
                let mut l = Cursor { row: 0, col: 0 };
                let mut t = Cursor { row: 0, col: 0 };
                let mut level = false;

                loop {
                    if core.char_at(t) == Some(self.0) {
                        level = !level;
                        if !level && t >= core.cursor() && l <= core.cursor() {
                            if prefix == Prefix::Inner {
                                let l = core.next_cursor(l)?;
                                let r = core.prev_cursor(t)?;
                                return if l <= r {
                                    Some(CursorRange(l, r))
                                } else {
                                    None
                                };
                            } else {
                                return Some(CursorRange(l, t));
                            }
                        }
                        l = t;
                    }

                    if t > core.cursor() && !level {
                        return None;
                    }

                    t = core.next_cursor(t)?;
                }
            }
            _ => None,
        }
    }
}

impl TextObject for Parens {
    fn get_range(&self, prefix: Prefix, core: &Core) -> Option<CursorRange> {
        match prefix {
            Prefix::A | Prefix::Inner => {
                let mut stack = Vec::new();
                let mut t = Cursor { row: 0, col: 0 };

                loop {
                    if core.char_at(t) == Some(self.0) {
                        stack.push(t);
                    } else if core.char_at(t) == Some(self.1) {
                        if let Some(l) = stack.pop() {
                            if l <= core.cursor() && t >= core.cursor() {
                                if prefix == Prefix::Inner {
                                    let l = core.next_cursor(l)?;
                                    let r = core.prev_cursor(t)?;
                                    return if l < r { Some(CursorRange(l, r)) } else { None };
                                } else {
                                    return Some(CursorRange(l, t));
                                }
                            }
                        }
                    }

                    if t > core.cursor() && stack.get(0).map(|&c| c > core.cursor()).unwrap_or(true)
                    {
                        return None;
                    }
                    t = core.next_cursor(t)?;
                }
            }
            _ => None,
        }
    }
}

impl TextObject for Word {
    fn get_range(&self, prefix: Prefix, core: &Core) -> Option<CursorRange> {
        Some(match prefix {
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
        })
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
            match c {
                'w' => {
                    self.object = Some(Box::new(Word));
                    return true;
                }
                '\'' | '"' => {
                    self.object = Some(Box::new(Quote(c)));
                    return true;
                }
                '{' | '}' => {
                    self.object = Some(Box::new(Parens('{', '}')));
                    return true;
                }
                '(' | ')' => {
                    self.object = Some(Box::new(Parens('(', ')')));
                    return true;
                }
                '[' | ']' => {
                    self.object = Some(Box::new(Parens('[', ']')));
                    return true;
                }
                _ => (),
            }
        }
        false
    }

    pub fn get_range(&self, core: &Core) -> Option<CursorRange> {
        self.object
            .as_ref()
            .and_then(|obj| obj.get_range(self.prefix, core))
    }
}
