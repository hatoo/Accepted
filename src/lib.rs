extern crate shellexpand;
extern crate syntect;
extern crate termion;
extern crate unicode_width;

mod buffer;
mod core;
mod cursor;
pub mod draw;
mod mode;
mod rustfmt;
pub mod syntax;
pub mod theme;

pub use buffer::Buffer;
use core::Core;
use mode::{Mode, Normal, Transition};

pub struct BufferMode<'a> {
    buf: Buffer<'a>,
    mode: Box<Mode>,
}

impl<'a> BufferMode<'a> {
    pub fn new(buf: Buffer<'a>) -> Self {
        Self {
            buf,
            mode: Box::new(Normal::new()),
        }
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
