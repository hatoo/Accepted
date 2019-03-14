use crate::buffer::Buffer;
use crate::buffer_mode::BufferMode;
use crate::buffer_mode::TabOperation;
use crate::config::ConfigWithDefault;
use crate::draw;
use crate::draw::CharStyle;
use crate::rmate::{start_server, RmateSave, RmateStorage};
use crate::storage::Storage;
use crate::syntax::SyntaxParent;
use std::cmp::min;
use std::sync::mpsc;
use std::thread;
use unicode_width::UnicodeWidthChar;

use termion::event::Event;
use termion::event::MouseButton;
use termion::event::MouseEvent;

struct TabLine {
    buf: Vec<Option<(char, CharStyle)>>,
    tab: Vec<Option<usize>>,
    cursor: usize,
}

impl TabLine {
    fn new(width: usize) -> Self {
        Self {
            buf: vec![None; width],
            tab: vec![None; width],
            cursor: 0,
        }
    }

    fn put(&mut self, c: char, style: CharStyle, tab_num: Option<usize>) {
        if let Some(w) = c.width() {
            if self.cursor + w < self.buf.len() {
                self.buf[self.cursor] = Some((c, style));
                self.tab[self.cursor] = tab_num;
                self.cursor += 1;
                for _ in 1..w {
                    self.tab[self.cursor] = tab_num;
                    self.cursor += 1;
                }
            }
        }
    }

    fn puts(&mut self, s: &str, style: CharStyle, tab_num: Option<usize>) {
        for c in s.chars() {
            self.put(c, style, tab_num);
        }
    }
}

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

    fn new_buffer_mode(&self) -> BufferMode<'a> {
        BufferMode::new(Buffer::new(self.syntax_parent, self.config))
    }

    pub fn open<S: Storage + 'static>(&mut self, s: S) {
        if self.is_empty() {
            self.buffers.clear();
        }

        let mut buffer_mode = self.new_buffer_mode();
        buffer_mode.buf.open(s);
        self.buffers.push(buffer_mode);
    }

    pub fn buffer_mode(&self) -> &BufferMode<'a> {
        &self.buffers[self.index]
    }

    pub fn buffer_mode_mut(&mut self) -> &mut BufferMode<'a> {
        &mut self.buffers[self.index]
    }

    // true if just started
    pub fn is_empty(&self) -> bool {
        self.buffers.len() == 1
            && self.buffer_mode().buf.storage().is_none()
            && self.buffer_mode().buf.core.get_string() == ""
    }

    pub fn event(&mut self, event: Event) -> bool {
        if let Event::Mouse(MouseEvent::Press(MouseButton::Left, col, row)) = event {
            let (width, height) = termion::terminal_size().unwrap();
            if row == height {
                let tab_line = self.draw_tab_line(width as usize);
                let col = col as usize - 1;

                if let Some(i) = tab_line.tab[col] {
                    self.index = i;
                }
            }
        }

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

    fn draw_tab_line(&self, width: usize) -> TabLine {
        const TITLE_LEN: usize = 5;
        let mut footer = TabLine::new(width);

        if self.rmate.is_some() {
            footer.puts("R", draw::styles::HIGHLIGHT, None);
            footer.puts(" ", draw::styles::DEFAULT, None);
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
                footer.puts(
                    &format!(" {} {}", i + 1, msg),
                    draw::styles::TAB_BAR,
                    Some(i),
                );
            } else {
                footer.puts(
                    &format!(" {} {}", i + 1, msg),
                    draw::styles::DEFAULT,
                    Some(i),
                );
            }
        }

        footer.put(' ', draw::styles::UI, None);

        footer
    }

    pub fn draw(&mut self, mut view: draw::TermView) -> draw::CursorState {
        {
            if let Some(rmate) = self.rmate.as_ref() {
                match rmate.try_recv() {
                    Ok(rmate) => {
                        let rmate: RmateStorage = rmate.into();
                        self.open(rmate);
                        self.index = self.buffers.len() - 1;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.rmate = None;
                    }
                    _ => {}
                }
            }
        }
        let width = view.width();

        let cursor =
            self.buffer_mode_mut()
                .draw(view.view((0, 0), view.height() - 1, view.width()));
        let mut footer = view.view((view.height() - 1, 0), 1, view.width());
        let tab_line = self.draw_tab_line(width);

        for tile in tab_line.buf {
            if let Some((c, style)) = tile {
                footer.put(c, style, None);
            }
        }

        cursor
    }
}
