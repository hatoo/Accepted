use crate::buffer::Buffer;
use crate::buffer_mode::BufferMode;
use crate::buffer_mode::TabOperation;
use crate::config::ConfigWithDefault;
use crate::draw;
use crate::syntax::SyntaxParent;
use std::cmp::min;

use termion::event::Event;

pub struct BufferTab<'a> {
    syntax_parent: &'a SyntaxParent,
    config: &'a ConfigWithDefault,

    buffers: Vec<BufferMode<'a>>,
    index: usize,
}

impl<'a> BufferTab<'a> {
    pub fn new(syntax_parent: &'a SyntaxParent, config: &'a ConfigWithDefault) -> Self {
        Self {
            syntax_parent,
            config,
            buffers: vec![BufferMode::new(Buffer::new(syntax_parent, config))],
            index: 0,
        }
    }

    pub fn buffer_mode(&self) -> &BufferMode<'a> {
        &self.buffers[self.index]
    }

    pub fn buffer_mode_mut(&mut self) -> &mut BufferMode<'a> {
        &mut self.buffers[self.index]
    }

    pub fn event(&mut self, event: Event) -> bool {
        match self.buffer_mode_mut().event(event) {
            TabOperation::Close => {
                if self.buffers.len() <= 1 {
                    return true;
                } else {
                    self.buffers.remove(self.index);
                    self.index = min(self.buffers.len() - 1, self.index);
                }
            }
            TabOperation::NewTab => {
                self.buffers.push(BufferMode::new(Buffer::new(
                    self.syntax_parent,
                    self.config,
                )));
                self.index = self.buffers.len() - 1;
            }
            TabOperation::Nothing => {}
        }

        false
    }

    pub fn draw(&mut self, view: draw::TermView) -> draw::CursorState {
        let cursor =
            self.buffer_mode_mut()
                .draw(view.view((0, 0), view.height() - 1, view.width()));
        let mut footer = view.view((view.height() - 1, 0), 1, view.width());

        for i in 0..self.buffers.len() {
            if self.index == i {
                footer.puts(&format!("[{}]", i + 1), draw::styles::FOOTER);
            } else {
                footer.puts(&format!(" {} ", i + 1), draw::styles::FOOTER);
            }
        }

        cursor
    }
}
