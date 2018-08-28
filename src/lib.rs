extern crate termion;
extern crate unicode_width;

mod buffer;
mod core;
mod cursor;
mod mode;

use std::cmp::{max, min};
use std::io::{stdin, stdout, Write};

use buffer::Buffer;
use core::Core;
use mode::{Mode, Normal, Transition};

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

    pub fn draw(&self) -> Vec<u8> {
        self.mode.draw(&self.buf)
    }
}
