use crate::buffer::Buffer;
use crate::buffer_mode::BufferMode;
use crate::buffer_mode::TabOperation;
use crate::config::ConfigWithDefault;
use crate::draw;
use crate::rmate::{start_server, RmateSave, RmateStorage};
use crate::syntax::SyntaxParent;
use std::cmp::min;
use std::sync::mpsc;
use std::thread;
use unicode_width::UnicodeWidthChar;

use termion::event::Event;

pub struct BufferTab<'a> {
    syntax_parent: &'a SyntaxParent,
    config: &'a ConfigWithDefault,

    buffers: Vec<BufferMode<'a>>,
    index: usize,
    rmate: Option<mpsc::Receiver<RmateSave>>,
}

impl<'a> BufferTab<'a> {
    pub fn new(syntax_parent: &'a SyntaxParent, config: &'a ConfigWithDefault) -> Self {
        Self {
            syntax_parent,
            config,
            buffers: vec![BufferMode::new(Buffer::new(syntax_parent, config))],
            index: 0,
            rmate: None,
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
            TabOperation::ChangeTab(i) => {
                if i >= 1 && i <= self.buffers.len() {
                    self.index = i - 1;
                }
            }
            TabOperation::StartRmate => {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = start_server(tx);
                });
                self.rmate = Some(rx);
            }
            TabOperation::Nothing => {}
        }

        false
    }

    pub fn draw(&mut self, mut view: draw::TermView) -> draw::CursorState {
        {
            if let Some(rmate) = self.rmate.as_ref() {
                match rmate.try_recv() {
                    Ok(rmate) => {
                        let rmate: RmateStorage = rmate.into();
                        let mut buffer = Buffer::new(self.syntax_parent, self.config);
                        buffer.open(rmate);
                        self.buffers.push(BufferMode::new(buffer));
                        self.index = self.buffers.len() - 1;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.rmate = None;
                    }
                    _ => {}
                }
            }
        }
        const TITLE_LEN: usize = 5;

        let cursor =
            self.buffer_mode_mut()
                .draw(view.view((0, 0), view.height() - 1, view.width()));
        let mut footer = view.view((view.height() - 1, 0), 1, view.width());

        if self.rmate.is_some() {
            footer.puts("R", draw::styles::HIGHLIGHT);
            footer.puts(" ", draw::styles::DEFAULT);
        }

        for i in 0..self.buffers.len() {
            let title = if let Some(path) = self.buffers[i].buf().path() {
                path.file_name()
                    .map(|o| o.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                "*".to_string()
            };

            let mut msg = String::new();
            let mut w = 0;
            let mut is_long = false;

            for c in title.chars() {
                let c_w = c.width().unwrap_or(0);
                if w + c_w <= TITLE_LEN {
                    msg.push(c);
                    w += c_w;
                } else {
                    is_long = true;
                }
            }

            for _ in w..TITLE_LEN {
                msg.push(' ');
            }

            if is_long {
                msg.push('…');
            }

            if self.index == i {
                footer.puts(&format!(" {} {}", i + 1, msg), draw::styles::TAB_BAR);
            } else {
                footer.puts(&format!(" {} {}", i + 1, msg), draw::styles::DEFAULT);
            }
        }

        footer.put(' ', draw::styles::UI, None);
        // footer.put('t', draw::styles::DEFAULT, None).is_some() ;

        cursor
    }
}
