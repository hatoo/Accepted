use crate::buffer::Buffer;
use crate::buffer_mode::BufferMode;
use crate::config::ConfigWithDefault;
use crate::draw;
use crate::syntax::SyntaxParent;

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

    pub fn draw(&mut self, view: draw::TermView) -> draw::CursorState {
        self.buffer_mode_mut().draw(view)
    }
}
