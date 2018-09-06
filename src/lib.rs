extern crate termion;
extern crate unicode_width;

mod buffer;
mod core;
mod cursor;
pub mod draw;
mod mode;

use buffer::Buffer;
use core::Core;
use mode::{Mode, Normal, Transition};
use std::io;
use std::path::Path;

pub struct BufferMode {
    buf: Buffer,
    mode: Box<Mode>,
}

impl BufferMode {
    pub fn new() -> Self {
        Self {
            buf: Buffer::new(),
            mode: Box::new(Normal),
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.mode = Box::new(Normal);
        self.buf.open(path)
    }

    pub fn event(&mut self, event: termion::event::Event) -> bool {
        match self.mode.event(&mut self.buf, event) {
            Transition::Exit => {
                return true;
            }
            Transition::Trans(t) => {
                self.mode = t;
            }
            _ => {}
        }
        false
    }

    pub fn draw(&self, term: &mut draw::Term) {
        self.mode.draw(&self.buf, term)
    }
}
