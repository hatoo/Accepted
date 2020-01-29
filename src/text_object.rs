use crate::core::CoreBuffer;
use crate::core::{Core, Cursor, CursorRange};
use std::ops::Bound;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Action {
    Delete,
    Yank,
    Change,
}

impl Action {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'd' => Some(Action::Delete),
            'y' => Some(Action::Yank),
            'c' => Some(Action::Change),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Action::Delete => 'd',
            Action::Yank => 'y',
            Action::Change => 'c',
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TextObjectPrefix {
    Inner,
    A,
    None,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Prefix {
    TextObjectPrefix(TextObjectPrefix),
    Find { inclusive: bool },
}

pub trait TextObject<B: CoreBuffer> {
    fn get_range(
        &self,
        action: Action,
        prefix: TextObjectPrefix,
        core: &Core<B>,
    ) -> (Bound<Cursor>, Bound<Cursor>);
}

struct Word;

struct Quote(char);

struct Parens(char, char);

impl<B: CoreBuffer> TextObject<B> for Quote {
    fn get_range(
        &self,
        _: Action,
        prefix: TextObjectPrefix,
        core: &Core<B>,
    ) -> (Bound<Cursor>, Bound<Cursor>) {
        /*
        match prefix {
            TextObjectPrefix::A | TextObjectPrefix::Inner => {
                let mut l = Cursor { row: 0, col: 0 };
                let mut t = Cursor { row: 0, col: 0 };
                let mut level = false;

                loop {
                    if core.char_at(t) == Some(self.0) {
                        level = !level;
                        if !level && t >= core.cursor() && l <= core.cursor() {
                            if prefix == TextObjectPrefix::Inner {
                                let l = core.next_cursor(l)?;
                                let r = core.prev_cursor(t)?;
                                return if l <= r {
                                    Some(CursorRange::new(l, r))
                                } else {
                                    None
                                };
                            } else {
                                return Some(CursorRange::new(l, t));
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
        */
        unimplemented!()
    }
}

impl<B: CoreBuffer> TextObject<B> for Parens {
    fn get_range(
        &self,
        _: Action,
        prefix: TextObjectPrefix,
        core: &Core<B>,
    ) -> (Bound<Cursor>, Bound<Cursor>) {
        match prefix {
            TextObjectPrefix::A | TextObjectPrefix::Inner => {
                let mut stack = Vec::new();
                let mut t = Cursor { row: 0, col: 0 };

                loop {
                    if core.char_at(t) == Some(self.0) {
                        stack.push(t);
                    } else if core.char_at(t) == Some(self.1) {
                        if let Some(l) = stack.pop() {
                            if l <= core.cursor() && t >= core.cursor() {
                                if prefix == TextObjectPrefix::Inner {
                                    return if l < t {
                                        (Bound::Excluded(l), Bound::Excluded(t))
                                    } else {
                                        (
                                            Bound::Included(Cursor { row: 0, col: 0 }),
                                            Bound::Excluded(Cursor { row: 0, col: 0 }),
                                        )
                                    };
                                } else {
                                    return (Bound::Included(l), Bound::Included(t));
                                }
                            }
                        }
                    }

                    if t > core.cursor() && stack.get(0).map(|&c| c > core.cursor()).unwrap_or(true)
                    {
                        return (
                            Bound::Included(Cursor { row: 0, col: 0 }),
                            Bound::Excluded(Cursor { row: 0, col: 0 }),
                        );
                    }
                    if let Some(next) = core.next_cursor(t) {
                        t = next;
                    } else {
                        return (
                            Bound::Included(Cursor { row: 0, col: 0 }),
                            Bound::Excluded(Cursor { row: 0, col: 0 }),
                        );
                    }
                }
            }
            _ => (
                Bound::Included(Cursor { row: 0, col: 0 }),
                Bound::Excluded(Cursor { row: 0, col: 0 }),
            ),
        }
    }
}

impl<B: CoreBuffer> TextObject<B> for Word {
    fn get_range(
        &self,
        action: Action,
        prefix: TextObjectPrefix,
        core: &Core<B>,
    ) -> (Bound<Cursor>, Bound<Cursor>) {
        match prefix {
            TextObjectPrefix::None => {
                let l = core.cursor();
                let mut r = l;
                while r.col < core.len_line(r.row)
                    && core
                        .char_at(r)
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(false)
                {
                    r.col += 1;
                }
                if action != Action::Change {
                    while r.col < core.len_line(r.row) && core.char_at(r) == Some(' ') {
                        r.col += 1;
                    }
                }
                (Bound::Included(l), Bound::Excluded(r))
            }
            TextObjectPrefix::A | TextObjectPrefix::Inner => {
                let mut l = core.cursor();
                let mut r = l;

                while l.col > 0
                    && core
                        .char_at(Cursor {
                            row: l.row,
                            col: l.col - 1,
                        })
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(false)
                {
                    l.col -= 1;
                }

                while r.col < core.len_line(r.row)
                    && core
                        .char_at(r)
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(false)
                {
                    r.col += 1;
                }

                if action != Action::Change && prefix != TextObjectPrefix::Inner {
                    while r.col < core.len_line(r.row)
                        && core.char_at(r).map(|c| c == ' ').unwrap_or(false)
                    {
                        r.col += 1;
                    }
                }

                (Bound::Included(l), Bound::Excluded(r))
            }
        }
    }
}

pub struct TextObjectParser {
    pub action: Action,
    pub prefix: Prefix,
}

impl TextObjectParser {
    pub fn new(action: Action) -> Self {
        Self {
            action,
            prefix: Prefix::TextObjectPrefix(TextObjectPrefix::None),
        }
    }
}

impl TextObjectParser {
    pub fn parse<B: CoreBuffer>(
        &mut self,
        c: char,
        core: &Core<B>,
    ) -> Option<(Bound<Cursor>, Bound<Cursor>)> {
        if let Prefix::TextObjectPrefix(_) = self.prefix {
            match c {
                'a' => {
                    self.prefix = Prefix::TextObjectPrefix(TextObjectPrefix::A);
                }
                'i' => {
                    self.prefix = Prefix::TextObjectPrefix(TextObjectPrefix::Inner);
                }
                'f' | 't' => {
                    if let Prefix::TextObjectPrefix { .. } = self.prefix {
                        self.prefix = Prefix::Find {
                            inclusive: c == 'f',
                        };
                        return None;
                    }
                }
                _ => (),
            }
        }

        match self.prefix {
            Prefix::Find { inclusive } => {
                let find = c;
                let l = core.cursor();
                let mut r = l;
                while r.col < core.len_line(r.row) && core.char_at(r) != Some(find) {
                    r.col += 1;
                }

                if r.col == core.len_line(r.row) {
                    // Nothing
                    return Some((Bound::Included(l), Bound::Excluded(l)));
                }

                if inclusive {
                    Some((Bound::Included(l), Bound::Included(r)))
                } else {
                    Some((Bound::Included(l), Bound::Excluded(r)))
                }
            }
            Prefix::TextObjectPrefix(text_object_prefix) => match c {
                'w' => Some(Word.get_range(self.action, text_object_prefix, core)),
                '\'' | '"' => Some(Quote(c).get_range(self.action, text_object_prefix, core)),
                '{' | '}' => {
                    Some(Parens('{', '}').get_range(self.action, text_object_prefix, core))
                }
                '(' | ')' => {
                    Some(Parens('(', ')').get_range(self.action, text_object_prefix, core))
                }
                '[' | ']' => {
                    Some(Parens('[', ']').get_range(self.action, text_object_prefix, core))
                }
                _ => None,
            },
        }
    }
}
