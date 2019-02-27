use crate::core::Cursor;
use crate::cursor;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Eq, PartialEq, Debug)]
pub enum CursorState {
    Hide,
    Show(Cursor, CursorShape),
}

impl fmt::Display for CursorShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CursorShape::Bar => write!(f, "{}", cursor::Bar),
            CursorShape::Block => write!(f, "{}", cursor::Block),
            CursorShape::Underline => write!(f, "{}", cursor::UnderLine),
        }
    }
}

impl fmt::Display for CursorState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let CursorState::Show(ref cursor, ref shape) = self {
            write!(
                f,
                "{}{}{}",
                termion::cursor::Goto(cursor.col as u16 + 1, cursor.row as u16 + 1),
                shape,
                termion::cursor::Show
            )
        } else {
            write!(f, "{}", termion::cursor::Hide)
        }
    }
}
