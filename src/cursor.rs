use std::fmt;

pub struct Block;

pub struct Bar;

pub struct UnderLine;

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1b[\x30 q")
    }
}

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1b[\x36 q")
    }
}

impl fmt::Display for UnderLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1b[\x34 q")
    }
}
