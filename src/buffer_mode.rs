use crate::buffer::Buffer;
use crate::draw;
use crate::mode::{Mode, Normal, Transition};

pub struct BufferMode<'a> {
    pub buf: Buffer<'a>,
    mode: Box<Mode>,
    is_recording: bool,
    dot_macro: Vec<termion::event::Event>,
    recording_macro: Vec<termion::event::Event>,
}

pub enum TabOperation {
    Nothing,
    Close,
    NewTab,
    ChangeTab(usize),
}

impl<'a> BufferMode<'a> {
    pub fn new(buf: Buffer<'a>) -> Self {
        Self {
            buf,
            mode: Box::new(Normal::default()),
            is_recording: false,
            dot_macro: Vec::new(),
            recording_macro: Vec::new(),
        }
    }

    pub fn buf(&self) -> &Buffer {
        &self.buf
    }

    pub fn event(&mut self, event: termion::event::Event) -> TabOperation {
        if self.is_recording {
            self.recording_macro.push(event.clone());
        }
        match self.mode.event(&mut self.buf, event.clone()) {
            Transition::Exit => {
                return TabOperation::Close;
            }
            Transition::Trans(mut t) => {
                t.init(&mut self.buf);
                self.mode = t;
            }
            Transition::DoMacro => {
                for event in self.dot_macro.clone() {
                    self.event(event);
                }
            }
            Transition::Return(s, is_commit_macro) => {
                if self.is_recording && !self.recording_macro.is_empty() && is_commit_macro {
                    std::mem::swap(&mut self.dot_macro, &mut self.recording_macro);
                    self.recording_macro.clear();
                }
                self.is_recording = false;
                let mut t = if let Some(s) = s {
                    Box::new(Normal::with_message(s))
                } else {
                    Box::new(Normal::default())
                };
                t.init(&mut self.buf);
                self.mode = t;
            }
            Transition::RecordMacro(mut t) => {
                self.is_recording = true;
                self.recording_macro.clear();
                self.recording_macro.push(event);
                t.init(&mut self.buf);
                self.mode = t;
            }
            Transition::CreateNewTab => {
                self.mode = Box::new(Normal::default());
                return TabOperation::NewTab;
            }
            Transition::ChangeTab(i) => {
                self.mode = Box::new(Normal::default());
                return TabOperation::ChangeTab(i);
            }
            Transition::Nothing => {}
        }
        TabOperation::Nothing
    }

    pub fn draw(&mut self, view: draw::TermView) -> draw::CursorState {
        self.mode.draw(&mut self.buf, view)
    }

    /// This method should be called every frame
    pub fn background_task_duration(&mut self, duration: std::time::Duration) {
        self.buf.extend_cache_duration(duration);
    }
}
