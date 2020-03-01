use crate::buffer::Buffer;
use crate::core::CoreBuffer;
use crate::draw;
use crate::mode::{Mode, Normal, Transition, TransitionReturn};
use futures::future::{FutureExt, LocalBoxFuture};

pub struct BufferMode<'a, B: CoreBuffer> {
    pub buf: Buffer<'a, B>,
    mode: Box<dyn Mode<B>>,
    is_recording: bool,
    dot_macro: Vec<termion::event::Event>,
    recording_macro: Vec<termion::event::Event>,
}

pub enum TabOperation {
    Nothing,
    Close,
    NewTab,
    ChangeTab(usize),
    StartRmate,
}

impl<'a, B: CoreBuffer> BufferMode<'a, B> {
    pub fn new(buf: Buffer<'a, B>) -> Self {
        Self {
            buf,
            mode: Box::new(Normal::default()),
            is_recording: false,
            dot_macro: Vec::new(),
            recording_macro: Vec::new(),
        }
    }

    pub fn event(&mut self, event: termion::event::Event) -> LocalBoxFuture<'_, TabOperation> {
        async move {
            if self.is_recording {
                self.recording_macro.push(event.clone());
            }
            match self.mode.event(&mut self.buf, event.clone()).await {
                Transition::Exit => {
                    return TabOperation::Close;
                }
                Transition::Trans(mut t) => {
                    t.init(&mut self.buf);
                    self.mode = t;
                }
                Transition::DoMacro => {
                    for event in self.dot_macro.clone() {
                        self.event(event).await;
                    }
                }
                Transition::Return(TransitionReturn {
                    message,
                    is_commit_dot_macro,
                }) => {
                    if self.is_recording && !self.recording_macro.is_empty() && is_commit_dot_macro
                    {
                        std::mem::swap(&mut self.dot_macro, &mut self.recording_macro);
                        self.recording_macro.clear();
                    }
                    self.is_recording = false;
                    let mut t = if let Some(s) = message {
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
                Transition::StartRmate => {
                    self.mode = Box::new(Normal::default());
                    return TabOperation::StartRmate;
                }
                Transition::Nothing => {}
            }
            TabOperation::Nothing
        }
        .boxed_local()
    }

    pub fn draw(&mut self, view: draw::TermView) -> draw::CursorState {
        self.mode.draw(&mut self.buf, view)
    }

    /// This method should be called every frame
    pub fn background_task_duration(&mut self, duration: std::time::Duration) {
        self.buf.extend_cache_duration(duration);
    }
}
