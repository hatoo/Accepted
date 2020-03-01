use std;
use std::cmp::{max, min};
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::ops::Bound;
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use shellexpand;
use termion;
use termion::event::{Event, Key, MouseButton, MouseEvent};

use crate::buffer::Buffer;
use crate::buffer::Yank;
use crate::clipboard;
use crate::config::types::keys;
use crate::core::Core;
use crate::core::CoreBuffer;
use crate::core::Cursor;
use crate::core::Id;
use crate::draw;
use crate::indent;
use crate::lsp_async::LSPCompletion;
use crate::parenthesis;
use crate::tabnine::TabNineCompletion;
use crate::text_object::{self, Action};

mod fuzzy;

pub struct TransitionReturn {
    pub message: Option<String>,
    pub is_commit_dot_macro: bool,
}

pub enum Transition<B: CoreBuffer> {
    Nothing,
    Trans(Box<dyn Mode<B>>),
    RecordMacro(Box<dyn Mode<B>>),
    DoMacro,
    // Message, is commit dot macro?
    Return(TransitionReturn),
    Exit,
    CreateNewTab,
    // 1-indexed
    ChangeTab(usize),
    StartRmate,
}

pub trait Mode<B: CoreBuffer> {
    fn init(&mut self, _buf: &mut Buffer<B>) {}
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B>;
    fn draw(&mut self, buf: &mut Buffer<B>, view: draw::TermView) -> draw::CursorState;
    fn into_transition(self) -> Transition<B>
    where
        Self: Sized + 'static,
    {
        Transition::Trans(Box::new(self))
    }
}

pub struct Normal {
    message: String,
    frame: usize,
}

struct Prefix;

struct Insert {
    completion_index: Option<usize>,
    buf_update: Id,
    completions: Vec<LSPCompletion>,
    tabnine_completions: Vec<TabNineCompletion>,
    snippet_completions: Vec<String>,
}

impl Default for Insert {
    fn default() -> Self {
        Insert {
            completion_index: None,
            completions: Vec::new(),
            snippet_completions: Vec::new(),
            tabnine_completions: Vec::new(),
            buf_update: Id::default(),
        }
    }
}

struct R;

struct S<R: RangeBounds<Cursor> + Clone>(R);

struct Find {
    to_right: bool,
}

struct TextObjectOperation {
    parser: text_object::TextObjectParser,
}

impl TextObjectOperation {
    fn new(action: Action) -> Self {
        Self {
            parser: text_object::TextObjectParser::new(action),
        }
    }
}

struct Search;

struct Save {
    path: String,
}

struct Visual {
    cursor: Cursor,
    line_mode: bool,
}

struct ViewProcess {
    row_offset: usize,
    pub buf: Vec<String>,
    pub reader: mpsc::Receiver<String>,
    pub process: process::Child,
    pub start: Instant,
    pub end: Option<Instant>,
    title: Option<String>,
}

impl Drop for ViewProcess {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[derive(Default)]
struct Goto {
    row: Vec<char>,
}

impl ViewProcess {
    fn with_process(mut child: process::Child, title: Option<String>) -> Option<Self> {
        let now = Instant::now();
        let stdout = child.stdout.take()?;
        let stderr = child.stderr.take()?;
        let (tx, rx) = mpsc::channel();
        let tx1 = tx.clone();
        let tx2 = tx;

        thread::spawn(move || {
            let mut line = String::new();
            let mut stdout = BufReader::new(stdout);
            loop {
                line.clear();
                if stdout.read_line(&mut line).is_ok() && !line.is_empty() {
                    if tx1.send(line.trim_end().to_string()).is_err() {
                        return;
                    }
                } else {
                    return;
                }
            }
        });
        thread::spawn(move || {
            let mut line = String::new();
            let mut stderr = BufReader::new(stderr);
            loop {
                line.clear();
                if stderr.read_line(&mut line).is_ok() && !line.is_empty() {
                    if tx2.send(line.trim_end().to_string()).is_err() {
                        return;
                    }
                } else {
                    return;
                }
            }
        });
        Some(Self {
            row_offset: 0,
            buf: Vec::new(),
            reader: rx,
            process: child,
            start: now,
            end: None,
            title,
        })
    }
}

impl Default for Normal {
    fn default() -> Self {
        Self {
            message: String::new(),
            frame: 0,
        }
    }
}

impl Normal {
    pub fn with_message(message: String) -> Self {
        Self { message, frame: 0 }
    }
}

impl<B: CoreBuffer> Mode<B> for Normal {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Char('.')) => {
                return Transition::DoMacro;
            }
            Event::Key(Key::Char('u')) => {
                buf.core.undo();
                buf.show_cursor();
            }
            Event::Key(Key::Char('U')) => {
                buf.core.redo();
                buf.show_cursor();
            }
            Event::Key(Key::Char('i')) => {
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('I')) => {
                buf.core.set_cursor(Cursor {
                    row: buf.core.cursor().row,
                    col: 0,
                });
                while buf.core.char_at_cursor() == Some(' ') {
                    buf.core.cursor_right();
                }
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('S')) => {
                buf.core.set_cursor(Cursor {
                    row: buf.core.cursor().row,
                    col: 0,
                });
                // TODO bulk delete
                for _ in 0..buf.core.len_current_line() {
                    buf.core.delete();
                }
                buf.indent();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('C')) => {
                // TODO bulk delete
                while buf.core.char_at_cursor().is_some() {
                    buf.core.delete();
                }
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('a')) => {
                buf.core.cursor_right();
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('A')) => {
                let mut c = buf.core.cursor();
                c.col = buf.core.len_current_line();
                buf.core.set_cursor(c);
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('r')) => {
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(R));
            }
            Event::Key(Key::Char('s')) => {
                buf.core.delete();
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('o')) => {
                buf.core.insert_newline();
                buf.indent();
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('O')) => {
                buf.core.insert_newline_here();
                buf.indent();
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('h')) => {
                buf.core.cursor_left();
                buf.show_cursor();
            }
            Event::Key(Key::Char('j')) => {
                buf.core.cursor_down();
                buf.show_cursor();
            }
            Event::Key(Key::Char('k')) => {
                buf.core.cursor_up();
                buf.show_cursor();
            }
            Event::Key(Key::Char('l')) => {
                buf.core.cursor_right();
                buf.show_cursor();
            }
            Event::Key(Key::Char('w')) => {
                buf.core.w();
                buf.show_cursor();
            }
            Event::Key(Key::Char('b')) => {
                buf.core.b();
                buf.show_cursor();
            }
            Event::Key(Key::Char('e')) => {
                buf.core.e();
                buf.show_cursor();
            }
            Event::Key(Key::Char('f')) => {
                return Find { to_right: true }.into_transition();
            }
            Event::Key(Key::Char('F')) => {
                return Find { to_right: false }.into_transition();
            }
            Event::Key(Key::Char('0')) => {
                buf.core.set_cursor(Cursor {
                    row: buf.core.cursor().row,
                    col: 0,
                });
            }
            Event::Key(Key::Char('$')) => {
                buf.core.set_cursor(Cursor {
                    row: buf.core.cursor().row,
                    col: buf.core.len_current_line(),
                });
            }
            Event::Key(Key::Char('g')) => {
                buf.core.set_cursor(Cursor { row: 0, col: 0 });
                buf.show_cursor();
            }
            Event::Key(Key::Char('G')) => {
                let row = buf.core.core_buffer().len_lines() - 1;
                let col = buf.core.core_buffer().len_line(row);
                buf.core.set_cursor(Cursor { row, col });
                buf.show_cursor();
            }
            Event::Key(Key::Char('n')) => {
                if !buf.search.is_empty() {
                    let mut pos = buf.core.cursor();

                    let search = buf.search.iter().collect::<String>();
                    let ac = aho_corasick::AhoCorasick::new(vec![search]);

                    if let Some(p) = buf.core.next_cursor(pos) {
                        pos = p;
                    } else {
                        pos = Cursor { row: 0, col: 0 };
                    }

                    let pos_bytes = if let Some(Ok(m)) = ac
                        .stream_find_iter(iter_read::IterRead::new(
                            buf.core.core_buffer().bytes_range(pos..),
                        ))
                        .next()
                    {
                        Some(buf.core.core_buffer().cursor_to_bytes(pos) + m.start())
                    } else if let Some(Ok(m)) = ac
                        .stream_find_iter(iter_read::IterRead::new(
                            buf.core.core_buffer().bytes_range(..pos),
                        ))
                        .next()
                    {
                        Some(m.start())
                    } else {
                        None
                    };

                    if let Some(start) = pos_bytes {
                        buf.core
                            .set_cursor(buf.core.core_buffer().bytes_to_cursor(start));
                        buf.show_cursor();
                    }
                }
            }
            Event::Key(Key::Char('N')) => {
                // TODO: Use aho-corasick. Waiting reverse iterator of ropey.
                if !buf.search.is_empty() {
                    let search: String = buf.search.iter().collect();
                    let ac = aho_corasick::AhoCorasick::new(vec![search]);

                    let mut last_before = None;
                    let mut last = None;

                    for cursor in ac
                        .stream_find_iter(iter_read::IterRead::new(
                            buf.core.core_buffer().bytes_range(..),
                        ))
                        .filter_map(|m| m.ok())
                        .map(|m| buf.core.core_buffer().bytes_to_cursor(m.start()))
                    {
                        match cursor.cmp(&buf.core.cursor()) {
                            std::cmp::Ordering::Equal | std::cmp::Ordering::Less => {
                                last_before = Some(cursor);
                            }
                            std::cmp::Ordering::Greater => {
                                if last_before.is_some() {
                                    break;
                                } else {
                                    last = Some(cursor);
                                }
                            }
                        }
                    }
                    if let Some(cursor) = last_before.or(last) {
                        buf.core.set_cursor(cursor);
                        buf.show_cursor();
                    }
                }
            }
            Event::Key(Key::Char('x')) => {
                buf.core.delete();
                buf.core.commit();
                buf.show_cursor();
            }
            Event::Key(Key::Char('/')) => return Search.into_transition(),
            Event::Key(Key::Char('v')) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: false,
                }
                .into_transition();
            }
            Event::Key(Key::Char('V')) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: true,
                }
                .into_transition();
            }
            Event::Key(Key::Char('p')) => {
                if buf.yank.insert_newline {
                    buf.core.insert_newline();
                } else {
                    buf.core.cursor_right();
                }

                for c in buf.yank.content.chars() {
                    buf.core.insert(c);
                }
                buf.core.commit();
                buf.show_cursor();
            }
            Event::Key(Key::Char('P')) => {
                if buf.yank.insert_newline {
                    buf.core.insert_newline_here();
                }

                for c in buf.yank.content.chars() {
                    buf.core.insert(c);
                }
                buf.core.commit();
                buf.show_cursor();
            }
            Event::Key(Key::Ctrl('p')) => {
                if let Ok(s) = clipboard::clipboard_paste() {
                    for c in s.chars() {
                        buf.core.insert(c);
                    }
                    buf.core.commit();
                } else {
                    self.message = "Failed to paste from clipboard".to_string();
                }
                buf.show_cursor();
            }
            Event::Key(Key::Char(' ')) => {
                return Prefix.into_transition();
            }
            Event::Key(Key::Char('z')) => {
                buf.show_cursor_middle();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::Left, x, y)) => {
                let col = x as usize - 1;
                let row = y as usize - 1;
                let cursor = Cursor { row, col };

                let mut term = draw::Term::default();
                let height = term.height;
                let width = term.width;
                buf.draw(term.view((0, 0), height, width));

                if let Some(c) = term.pos(cursor) {
                    if buf.core.cursor() == c {
                        // Double click!
                        buf.core.b();
                        let cursor = buf.core.cursor();
                        buf.core.e();

                        return Visual {
                            cursor,
                            line_mode: false,
                        }
                        .into_transition();
                    } else {
                        buf.core.set_cursor(c);
                    }
                }
            }
            Event::Mouse(MouseEvent::Hold(_, _)) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: false,
                }
                .into_transition();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                buf.scroll_up();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                buf.scroll_down();
            }
            Event::Key(Key::Char(c)) if c.is_digit(10) => {
                if let Some(i) = c.to_digit(10) {
                    return Transition::ChangeTab(i as usize);
                }
            }
            _ => {
                if let Event::Key(Key::Char(c)) = event {
                    if let Some(action) = Action::from_char(c) {
                        return Transition::RecordMacro(Box::new(TextObjectOperation::new(action)));
                    }
                }
            }
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height();
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height - 1, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height - 1, 0), 1, width);
        if let Some(message) = buf.compiler_message_on_cursor() {
            footer.puts(message, draw::styles::FOOTER);
        } else {
            footer.puts(
                &format!(
                    "[Normal] ({} {}) [{}]",
                    buf.core.cursor().row + 1,
                    buf.core.cursor().col + 1,
                    buf.path()
                        .map(Path::to_string_lossy)
                        .unwrap_or_else(|| "*".into()),
                ),
                draw::styles::FOOTER,
            );
            if !self.message.is_empty() {
                footer.puts(&format!(" {}", &self.message,), draw::styles::FOOTER);
            }

            if buf.is_compiling() {
                let animation = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                let a = animation[self.frame % animation.len()];
                footer.puts(&format!(" {}Compiling ...", a), draw::styles::FOOTER);
            } else if let Some(success) = buf.last_compile_success() {
                let msg = if success {
                    " [Compile: Success]"
                } else {
                    " [Compile: Failed]"
                };
                footer.puts(msg, draw::styles::FOOTER);
            }
            footer.puts(
                &format!(" {} bytes", buf.core.core_buffer().len_bytes()),
                draw::styles::FOOTER,
            );
        }
        self.frame = (std::num::Wrapping(self.frame) + std::num::Wrapping(1)).0;

        if buf.lsp.is_some() {
            footer.puts(" [LSP]", draw::styles::FOOTER_BLUE);
        }

        if buf.tabnine.is_some() {
            footer.puts(" [TabNine]", draw::styles::FOOTER_BLUE);
        }

        cursor
    }
}

#[cfg(test)]
mod test_insert {
    use super::Core;
    use super::Insert;
    use crate::core::buffer::RopeyCoreBuffer;
    use crate::core::Cursor;

    #[test]
    fn test_token() {
        let mut core = Core::<RopeyCoreBuffer>::from_reader("token".as_bytes()).unwrap();
        core.set_cursor(Cursor { row: 0, col: 5 });
        assert_eq!(Insert::token(&core), "token".to_string());
        let mut core = Core::<RopeyCoreBuffer>::from_reader("tokenblah".as_bytes()).unwrap();
        core.set_cursor(Cursor { row: 0, col: 5 });
        assert_eq!(Insert::token(&core), "token".to_string());
    }

    #[test]
    fn test_remove_token() {
        let mut core = Core::<RopeyCoreBuffer>::from_reader("token".as_bytes()).unwrap();
        core.set_cursor(Cursor { row: 0, col: 5 });
        Insert::remove_token(&mut core);
        assert_eq!(core.get_string(), "".to_string());
    }
}

impl Insert {
    fn token<B: CoreBuffer>(core: &Core<B>) -> String {
        let mut cursor = core.cursor();

        while cursor.col > 0
            && core
                .core_buffer()
                .char_at(Cursor {
                    row: cursor.row,
                    col: cursor.col - 1,
                })
                .map(|c| c.is_alphabetic() || c == '_')
                .unwrap_or(false)
        {
            cursor.col -= 1;
        }

        core.get_string_range(cursor..core.cursor())
    }

    fn remove_token<B: CoreBuffer>(core: &mut Core<B>) {
        while {
            let cursor = core.cursor();
            cursor.col > 0
                && core
                    .core_buffer()
                    .char_at(Cursor {
                        row: cursor.row,
                        col: cursor.col - 1,
                    })
                    .map(|c| c.is_alphabetic() || c == '_')
                    .unwrap_or(false)
        } {
            core.cursor_left();
            core.delete();
        }
    }

    fn completion_len(&self) -> usize {
        self.completions.len() + self.tabnine_completions.len() + self.snippet_completions.len()
    }

    fn get_completion<B: CoreBuffer>(&self, buf: &Buffer<B>) -> Option<String> {
        let index = self.completion_index?;
        if index < self.completions.len() {
            Some(self.completions[index].keyword.clone())
        } else if index < self.completions.len() + self.tabnine_completions.len() {
            Some(
                self.tabnine_completions[index - self.completions.len()]
                    .keyword
                    .clone(),
            )
        } else {
            Some(
                buf.snippet[&self.snippet_completions
                    [index - self.completions.len() - self.tabnine_completions.len()]]
                    .clone(),
            )
        }
    }

    fn remove_old_prefix<B: CoreBuffer>(&self, core: &mut Core<B>) {
        if let Some(index) = self.completion_index {
            if index < self.completions.len() {
                Self::remove_token(core);
            } else if index < self.completions.len() + self.tabnine_completions.len() {
                let len = self.tabnine_completions[index - self.completions.len()]
                    .old_prefix
                    .chars()
                    .count();

                while core.cursor_dec() {
                    if core.char_at_cursor() == Some(' ') {
                        core.delete();
                    } else {
                        core.cursor_inc();
                        break;
                    }
                }

                for _ in 0..len {
                    core.cursor_dec();
                    core.delete();
                }
            } else {
                Self::remove_token(core);
            }
        }
    }

    fn poll<B: CoreBuffer>(&mut self, buf: &mut Buffer<B>) {
        if let Some(lsp) = buf.lsp.as_mut() {
            if let Some(mut completions) = lsp.poll() {
                let token = Self::token(&buf.core);
                completions.retain(|s| s.keyword != token);
                self.completions = completions;
            }
        }

        if let Some(tabnine) = buf.tabnine.as_mut() {
            if let Some(completion) = tabnine.poll() {
                self.tabnine_completions = completion;
            }
        }

        if self.completion_len() == 0 {
            self.completion_index = None;
        } else if let Some(index) = self.completion_index {
            self.completion_index = Some(index % self.completion_len());
        }
    }

    fn build_completion<B: CoreBuffer>(&mut self, buf: &mut Buffer<B>) {
        if self.buf_update == buf.core.buffer_changed() {
            return;
        }
        self.buf_update = buf.core.buffer_changed();
        let prefix = Self::token(&buf.core);
        let start_completion = {
            let cursor = buf.core.cursor();
            cursor.col > 0
                && buf
                    .core
                    .core_buffer()
                    .char_at(Cursor {
                        row: cursor.row,
                        col: cursor.col - 1,
                    })
                    .map(|c| c == ':' || c == '.')
                    .unwrap_or(false)
        };
        if !prefix.is_empty() || start_completion {
            if let Some(lsp) = buf.lsp.as_ref() {
                // LSP
                lsp.request_completion(buf.core.get_string(), buf.core.cursor());
            }
            if let Some(tabnine) = buf.tabnine.as_ref() {
                // TabNine
                tabnine.request_completion(buf);
            }
        }
        // snippet
        let prefix = Self::token(&buf.core);
        self.snippet_completions.clear();
        if !prefix.is_empty() {
            for keyword in buf.snippet.keys().filter(|k| k.starts_with(&prefix)) {
                self.snippet_completions.push(keyword.to_string());
            }
        }

        if self.completion_len() == 0 {
            self.completion_index = None;
        } else if let Some(index) = self.completion_index {
            self.completion_index = Some(min(index, self.completion_len() - 1));
        }
    }
}

impl<B: CoreBuffer> Mode<B> for Insert {
    fn init(&mut self, buf: &mut Buffer<B>) {
        // Flush completion
        if let Some(lsp) = buf.lsp.as_mut() {
            lsp.poll();
        }
        if let Some(tabnine) = buf.tabnine.as_mut() {
            tabnine.poll();
        }
        self.build_completion(buf);
    }
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                buf.core.commit();
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: true,
                });
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                buf.scroll_up();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                buf.scroll_down();
            }
            Event::Key(Key::Backspace) => {
                if buf.core.cursor() != (Cursor { col: 0, row: 0 }) {
                    buf.core.cursor_dec();
                    let c = buf.core.char_at_cursor();
                    buf.core.delete();
                    if buf.core.char_at_cursor().is_some()
                        && buf.core.char_at_cursor()
                            == parenthesis::PARENTHESIS_PAIRS
                                .iter()
                                .find(|t| c == Some(t.0))
                                .map(|t| t.1)
                    {
                        buf.core.delete();
                    }
                    buf.show_cursor();
                }
            }
            Event::Key(Key::Delete) => {
                buf.core.delete();
                buf.show_cursor();
            }
            Event::Key(Key::Char('\t')) => {
                if self.completion_len() > 0 {
                    if let Some(index) = self.completion_index {
                        self.completion_index = Some((index + 1) % self.completion_len());
                    } else {
                        self.completion_index = Some(0);
                    }
                } else {
                    if buf.hard_tab() {
                        buf.core.insert('\t');
                    } else {
                        buf.core.insert(' ');
                        while buf.core.cursor().col % buf.indent_width() != 0 {
                            buf.core.insert(' ');
                        }
                    }
                }
                return Transition::Nothing;
            }
            Event::Unsupported(ref v) if v.as_slice() == [27, 91, 90] => {
                // Shift Tab
                if self.completion_len() > 0 {
                    if let Some(index) = self.completion_index {
                        self.completion_index =
                            Some((index + self.completion_len() - 1) % self.completion_len());
                    } else {
                        self.completion_index = Some(self.completion_len() - 1);
                    }
                }
                return Transition::Nothing;
            }
            Event::Key(Key::Char('\n')) => {
                if self.completion_index.is_some() {
                    let body = &self.get_completion(buf).unwrap();
                    self.remove_old_prefix(&mut buf.core);
                    for c in body.chars() {
                        buf.core.insert(c);
                    }
                    buf.show_cursor();
                    self.completion_index = None;
                } else {
                    let indent_width = buf.indent_width();
                    buf.core.insert('\n');
                    let line = buf.core.get_string_range(
                        Cursor {
                            row: buf.core.cursor().row - 1,
                            col: 0,
                        }..Cursor {
                            row: buf.core.cursor().row - 1,
                            col: buf.core.core_buffer().len_line(buf.core.cursor().row - 1),
                        },
                    );
                    let indent = indent::next_indent_level(line.as_str(), indent_width);
                    if buf.hard_tab() {
                        for _ in 0..indent {
                            buf.core.insert('\t');
                        }
                    } else {
                        for _ in 0..indent_width * indent {
                            buf.core.insert(' ');
                        }
                    }
                    let pos = buf.core.cursor();
                    if ['}', ']', ')']
                        .iter()
                        .any(|&c| buf.core.char_at_cursor() == Some(c))
                    {
                        buf.core.insert('\n');
                        let i = if indent == 0 { 0 } else { indent - 1 };
                        for _ in 0..indent_width * i {
                            buf.core.insert(' ');
                        }
                    }
                    buf.core.set_cursor(pos);
                }
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                // Auto pair
                let pairs = [('(', ')'), ('{', '}'), ('[', ']'), ('"', '"')];

                if pairs.iter().any(|p| p.1 == c) && buf.core.char_at_cursor() == Some(c) {
                    buf.core.cursor_right();
                } else {
                    buf.core.insert(c);
                    let pair = pairs.iter().find(|p| p.0 == c);
                    if let Some((_, r)) = pair {
                        buf.core.insert(*r);
                        buf.core.cursor_left();
                    }
                }
            }
            _ => {}
        }
        self.build_completion(buf);
        buf.show_cursor();
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        self.poll(buf);
        let height = view.height();
        let width = view.width();
        let mut cursor = buf.draw(view.view((0, 0), height, width));
        let res = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Bar))
            .unwrap_or(draw::CursorState::Hide);

        if let Some(cursor) = cursor.as_mut() {
            cursor.row += 1;
        }
        let completion_height = height - cursor.map(|c| c.row).unwrap_or(0);
        let completion_width = width - cursor.map(|c| c.col).unwrap_or(0);

        if let Some(cursor) = cursor {
            if cursor.col + completion_width <= width && cursor.row + completion_height <= height {
                let mut view = view.view(cursor.into_tuple(), completion_height, completion_width);
                for i in 0..min(completion_height, self.completion_len()) {
                    let is_selected = Some(i) == self.completion_index;
                    if i < self.completions.len() {
                        let c = &self.completions[i];
                        for c in c.keyword.chars() {
                            if is_selected {
                                view.put_inline(c, draw::styles::HIGHLIGHT, None);
                            } else {
                                view.put_inline(c, draw::styles::UI, None);
                            }
                        }
                        view.put_inline(' ', draw::styles::DEFAULT, None);
                        for c in c.doc.chars() {
                            view.put_inline(c, draw::styles::SELECTED, None);
                        }
                    } else if i < self.completions.len() + self.tabnine_completions.len() {
                        let i = i - self.completions.len();
                        let c = &self.tabnine_completions[i];
                        for c in c.keyword.chars() {
                            if is_selected {
                                view.put_inline(c, draw::styles::HIGHLIGHT, None);
                            } else {
                                view.put_inline(c, draw::styles::UI, None);
                            }
                        }
                        view.put_inline(' ', draw::styles::DEFAULT, None);
                        for c in c.doc.chars() {
                            view.put_inline(c, draw::styles::SELECTED, None);
                        }
                    } else {
                        let i = i - self.completions.len() - self.tabnine_completions.len();
                        for c in self.snippet_completions[i].chars() {
                            if is_selected {
                                view.put_inline(c, draw::styles::HIGHLIGHT, None);
                            } else {
                                view.put_inline(c, draw::styles::UI, None);
                            }
                        }
                        view.put_inline(' ', draw::styles::DEFAULT, None);
                        view.puts("snippet", draw::styles::SELECTED);
                    }
                    view.newline();
                }
            }
        }

        res
    }
}

impl<B: CoreBuffer> Mode<B> for R {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char(c)) => {
                core.replace(c);
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: true,
                });
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height();
        let width = view.width();
        buf.draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Underline))
            .unwrap_or(draw::CursorState::Hide)
    }
}

impl<B: CoreBuffer> Mode<B> for Search {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Backspace) => {
                buf.search.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    return Transition::Return(TransitionReturn {
                        message: None,
                        is_commit_dot_macro: false,
                    });
                }
                buf.search.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height() - 1;
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height, 0), 1, width);
        footer.put('/', draw::styles::DEFAULT, None);
        for &c in &buf.search {
            footer.put(c, draw::styles::DEFAULT, None);
        }

        cursor
    }
}

impl<B: CoreBuffer> Mode<B> for Save {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Backspace) => {
                self.path.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    let path: String = shellexpand::tilde(&self.path).to_string();
                    buf.set_storage(PathBuf::from(path.clone()));
                    let message = if buf.save(false) {
                        format!("Saved to {}", path)
                    } else {
                        format!("Failed to save {}", path)
                    };
                    return Normal::with_message(message).into_transition();
                }
                self.path.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        if view.height() < 2 {
            return draw::CursorState::Hide;
        }
        let height = view.height() - 2;
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height, 0), 2, width);
        footer.puts(
            &std::env::current_dir().unwrap().to_string_lossy(),
            draw::styles::UI,
        );
        footer.newline();
        footer.puts("> ", draw::styles::UI);
        footer.puts(&self.path, draw::styles::UI);

        cursor
    }
}

impl<B: CoreBuffer> Mode<B> for Prefix {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char(' ')) => {
                if let Err(msg) = buf.format() {
                    return Transition::Return(TransitionReturn {
                        message: Some(msg.into_owned()),
                        is_commit_dot_macro: false,
                    });
                }
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char('q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Char('g')) => {
                return Goto::default().into_transition();
            }
            Event::Key(Key::Char('s')) => {
                if let Some(path) = buf.path().map(|p| p.to_string_lossy().into_owned()) {
                    let _ = buf.format();
                    let message = if buf.save(false) {
                        format!("Saved to {}", path)
                    } else {
                        format!("Failed to save {}", path)
                    };
                    return Transition::Return(TransitionReturn {
                        message: Some(message),
                        is_commit_dot_macro: false,
                    });
                } else {
                    return Save {
                        path: String::new(),
                    }
                    .into_transition();
                }
            }
            Event::Key(Key::Char('a')) => {
                if let Some(path) = buf.path() {
                    return Save {
                        path: path.to_string_lossy().to_string(),
                    }
                    .into_transition();
                } else {
                    return Save {
                        path: String::new(),
                    }
                    .into_transition();
                }
            }
            Event::Key(Key::Char('y')) => {
                let result = clipboard::clipboard_copy(&buf.core.get_string()).is_ok();
                return Transition::Return(TransitionReturn {
                    message: Some(
                        if result {
                            "Copied"
                        } else {
                            "Failed to copy to clipboard"
                        }
                        .to_string(),
                    ),
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char('l')) => {
                buf.restart_completer();
                return Transition::Return(TransitionReturn {
                    message: Some(
                        if buf.lsp.is_some() {
                            "LSP Restarted"
                        } else {
                            "Failed to restart LSP"
                        }
                        .to_string(),
                    ),
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char('t')) | Event::Key(Key::Char('T')) => {
                let is_optimize = event == Event::Key(Key::Char('T'));
                let result: Result<(process::Child, Option<String>), &'static str> = (|| {
                    let _ = buf.format();
                    buf.save(is_optimize);
                    buf.wait_compile_message();
                    let path = buf.path().ok_or("Save First")?;
                    crate::env::set_env(path);
                    let test_command = buf
                        .get_config::<keys::TestCommand>()
                        .ok_or("test_command is undefined")
                        .map(|c| c.clone())
                        .or_else(|e| {
                            // Detect shebang
                            let first_line = buf.core.core_buffer().get_range(
                                Cursor { row: 0, col: 0 }..Cursor {
                                    row: 0,
                                    col: buf.core.core_buffer().len_line(0),
                                },
                            );
                            if first_line.starts_with("#!") {
                                let mut v = first_line
                                    .trim_start_matches("#!")
                                    .split_whitespace()
                                    .map(|s| shellexpand::full(s).map(|s| s.into_owned()))
                                    .collect::<Result<Vec<_>, _>>()
                                    .map_err(|_| "Failed to expand shebang")?;
                                v.push(path.to_string_lossy().into_owned());

                                Ok(crate::config::types::Command {
                                    program: v[0].clone(),
                                    args: v[1..].to_vec(),
                                })
                            } else {
                                Err(e)
                            }
                        })?;
                    let prog = &test_command.program;
                    let prog =
                        shellexpand::full(prog).map_err(|_| "Failed to expand test_command")?;
                    let args = test_command
                        .args
                        .iter()
                        .map(|s| shellexpand::full(s).map(|s| s.into_owned()))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| "Failed to Expand test_command")?;
                    let input = clipboard::clipboard_paste()
                        .map_err(|_| "Failed to paste from clipboard")?;
                    let mut child = process::Command::new(prog.into_owned())
                        .args(args.iter())
                        .stdout(process::Stdio::piped())
                        .stderr(process::Stdio::piped())
                        .stdin(process::Stdio::piped())
                        .spawn()
                        .map_err(|_| "Failed to spawn")?;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = write!(stdin, "{}", input);
                    }
                    Ok((child, buf.path().and_then(|p| test_command.summary(p).ok())))
                })(
                );
                match result {
                    Err(err) => {
                        return Normal::with_message(err.to_string()).into_transition();
                    }
                    Ok((child, title)) => {
                        if let Some(next_state) = ViewProcess::with_process(child, title) {
                            return next_state.into_transition();
                        } else {
                            return Normal::with_message("Failed to test".to_string())
                                .into_transition();
                        }
                    }
                }
            }
            Event::Key(Key::Char('c')) => {
                return Transition::CreateNewTab;
            }
            Event::Key(Key::Char(c)) if c.is_digit(10) => {
                if let Some(i) = c.to_digit(10) {
                    return Transition::ChangeTab(i as usize);
                }
            }
            Event::Key(Key::Char('r')) => {
                return Transition::StartRmate;
            }
            Event::Key(Key::Char('f')) => {
                return fuzzy::FuzzyOpen::default().into_transition();
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height() - 1;
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height, 0), 1, width);
        footer.puts("Prefix", draw::styles::FOOTER_HIGHLIGHT);
        footer.puts(
            " ... [Esc: Return] [q: Quit] [s: Save] [a: save As ...] [<Space> Format]",
            draw::styles::FOOTER,
        );

        cursor
    }
}

impl Visual {
    fn get_range<B: CoreBuffer>(&self, to: Cursor, buf: &B) -> (Bound<Cursor>, Bound<Cursor>) {
        if self.line_mode {
            let mut l = min(self.cursor, to);
            let mut r = max(self.cursor, to);

            l.col = 0;
            r.col = buf.len_line(r.row);

            if r.row == buf.len_lines() - 1 {
                (Bound::Included(l), Bound::Excluded(r))
            } else {
                (Bound::Included(l), Bound::Included(r))
            }
        } else {
            let l = min(self.cursor, to);
            let r = max(self.cursor, to);

            if r >= buf.end_cursor() {
                (Bound::Included(l), Bound::Excluded(r))
            } else {
                (Bound::Included(l), Bound::Included(r))
            }
        }
    }
}

impl<B: CoreBuffer> Mode<B> for Visual {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char('h')) => {
                buf.core.cursor_left();
                buf.show_cursor();
            }
            Event::Key(Key::Char('j')) => {
                buf.core.cursor_down();
                buf.show_cursor();
            }
            Event::Key(Key::Char('k')) => {
                buf.core.cursor_up();
                buf.show_cursor();
            }
            Event::Key(Key::Char('l')) => {
                buf.core.cursor_right();
                buf.show_cursor();
            }
            Event::Key(Key::Char('w')) => {
                buf.core.w();
                buf.show_cursor();
            }
            Event::Key(Key::Char('b')) => {
                buf.core.b();
                buf.show_cursor();
            }
            Event::Key(Key::Char('e')) => {
                buf.core.e();
                buf.show_cursor();
            }
            Event::Key(Key::Char('g')) => {
                buf.core.set_cursor(Cursor { row: 0, col: 0 });
                buf.show_cursor();
            }
            Event::Key(Key::Char('G')) => {
                buf.core.set_cursor(buf.core.core_buffer().end_cursor());
                buf.show_cursor();
            }
            Event::Key(Key::Char('d'))
            | Event::Key(Key::Char('x'))
            | Event::Key(Key::Char('s')) => {
                let to_insert = event == Event::Key(Key::Char('s'));
                let range = self.get_range(buf.core.cursor(), buf.core.core_buffer());
                let s = if self.line_mode {
                    buf.core
                        .get_string_range(range.clone())
                        .trim_end()
                        .to_string()
                } else {
                    buf.core.get_string_range(range.clone())
                };
                buf.core.delete_range(range.clone());
                if to_insert && self.line_mode {
                    buf.core.insert_newline_here();
                    buf.indent();
                }
                buf.core.commit();
                buf.yank.insert_newline = self.line_mode;
                buf.yank.content = s;

                buf.show_cursor();
                return if to_insert {
                    Insert::default().into_transition()
                } else {
                    Transition::Return(TransitionReturn {
                        message: Some("Deleted".to_string()),
                        is_commit_dot_macro: true,
                    })
                };
            }
            Event::Key(Key::Char('p')) | Event::Key(Key::Ctrl('p')) => {
                let is_clipboard = event == Event::Key(Key::Ctrl('p'));
                let range = self.get_range(buf.core.cursor(), buf.core.core_buffer());
                buf.core.delete_range(range);
                if is_clipboard {
                    if let Ok(s) = clipboard::clipboard_paste() {
                        for c in s.chars() {
                            buf.core.insert(c);
                        }
                    }
                } else {
                    for c in buf.yank.content.chars() {
                        buf.core.insert(c);
                    }
                }
                buf.core.commit();
                buf.show_cursor();
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: true,
                });
            }
            Event::Key(Key::Char('y')) | Event::Key(Key::Ctrl('y')) => {
                let is_clipboard = event == Event::Key(Key::Ctrl('y'));
                let range = self.get_range(buf.core.cursor(), buf.core.core_buffer());
                let s = if self.line_mode {
                    buf.core
                        .get_string_range(range.clone())
                        .trim_end()
                        .to_string()
                } else {
                    buf.core.get_string_range(range.clone())
                };
                buf.core.set_cursor(min(self.cursor, buf.core.cursor()));
                if is_clipboard {
                    if clipboard::clipboard_copy(&s).is_ok() {
                        return Transition::Return(TransitionReturn {
                            message: Some("Yanked".to_string()),
                            is_commit_dot_macro: false,
                        });
                    } else {
                        return Transition::Return(TransitionReturn {
                            message: Some("Yank failed".to_string()),
                            is_commit_dot_macro: false,
                        });
                    }
                } else {
                    buf.yank.insert_newline = self.line_mode;
                    buf.yank.content = s;
                }
                return Transition::Return(TransitionReturn {
                    message: Some("Yanked".to_string()),
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char('S')) => {
                let range = self.get_range(buf.core.cursor(), buf.core.core_buffer());
                return S(range).into_transition();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::Left, x, y)) => {
                let col = x as usize - 1;
                let row = y as usize - 1;
                let cursor = Cursor { row, col };

                let mut term = draw::Term::default();
                let height = term.height;
                let width = term.width;
                buf.draw(term.view((0, 0), height, width));

                if let Some(c) = term.pos(cursor) {
                    buf.core.set_cursor(c);
                }
                return Normal::default().into_transition();
            }
            Event::Mouse(MouseEvent::Hold(x, y)) => {
                let col = x as usize - 1;
                let row = y as usize - 1;
                let cursor = Cursor { row, col };

                let mut term = draw::Term::default();
                let height = term.height;
                let width = term.width;
                buf.draw(term.view((0, 0), height, width));

                if let Some(c) = term.pos(cursor) {
                    buf.core.set_cursor(c);
                }
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height();
        let width = view.width();
        let range = self.get_range(buf.core.cursor(), buf.core.core_buffer());
        buf.draw_with_selected(view.view((0, 0), height, width), Some(range))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide)
    }
}

impl<B: CoreBuffer> Mode<B> for ViewProcess {
    fn event(&mut self, _buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => Normal::default().into_transition(),
            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                if self.row_offset <= 3 {
                    self.row_offset = 0;
                } else {
                    self.row_offset -= 3;
                }
                Transition::Nothing
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                self.row_offset = min(self.buf.len() - 1, self.row_offset + 3);
                Transition::Nothing
            }
            _ => Transition::Nothing,
        }
    }

    fn draw(&mut self, _buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        if self.end.is_none() {
            if let Ok(Some(_)) = self.process.try_wait() {
                self.end = Some(Instant::now());
            }
        }
        let mut read_cnt = 32;
        while let Ok(line) = self.reader.try_recv() {
            if read_cnt == 0 {
                break;
            }
            self.buf.push(line);
            read_cnt -= 1;
        }

        let height = view.height();
        let width = view.width();
        {
            let mut view = view.view((0, 0), height - 1, width);
            if let Some(title) = self.title.as_ref() {
                view.puts(&title, draw::styles::HIGHLIGHT);
                view.newline();
            }
            for line in &self.buf[self.row_offset..] {
                view.puts(line, draw::styles::DEFAULT);
                view.newline();
                if view.is_out() {
                    break;
                }
            }
            if let Some(end) = self.end {
                view.puts(&format!("{:?}", end - self.start), draw::styles::HIGHLIGHT);
            }
        }
        {
            let mut view = view.view((height - 1, 0), 1, width);
            view.puts("Esc to return", draw::styles::FOOTER);
        }
        draw::CursorState::Hide
    }
}

impl<B: CoreBuffer> Mode<B> for TextObjectOperation {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        if event == Event::Key(Key::Esc) {
            return Transition::Return(TransitionReturn {
                message: None,
                is_commit_dot_macro: false,
            });
        }
        if let Event::Key(Key::Char(c)) = event {
            if c == self.parser.action.to_char() {
                // Yank current line
                buf.yank = Yank {
                    insert_newline: true,
                    content: buf.core.get_string_range(
                        Cursor {
                            row: buf.core.cursor().row,
                            col: 0,
                        }..Cursor {
                            row: buf.core.cursor().row,
                            col: buf.core.len_current_line(),
                        },
                    ),
                };
                match self.parser.action {
                    // dd
                    Action::Delete => {
                        let range =
                            if buf.core.cursor().row == buf.core.core_buffer().len_lines() - 1 {
                                if let Some(c) = buf.core.prev_cursor(Cursor {
                                    col: 0,
                                    row: buf.core.cursor().row,
                                }) {
                                    (
                                        Bound::Included(c),
                                        Bound::Excluded(Cursor {
                                            row: buf.core.cursor().row,
                                            col: buf.core.len_current_line(),
                                        }),
                                    )
                                } else {
                                    (
                                        Bound::Included(Cursor {
                                            row: buf.core.cursor().row,
                                            col: 0,
                                        }),
                                        Bound::Excluded(Cursor {
                                            row: buf.core.cursor().row,
                                            col: buf.core.len_current_line(),
                                        }),
                                    )
                                }
                            } else {
                                (
                                    Bound::Included(Cursor {
                                        row: buf.core.cursor().row,
                                        col: 0,
                                    }),
                                    Bound::Included(Cursor {
                                        row: buf.core.cursor().row,
                                        col: buf.core.len_current_line(),
                                    }),
                                )
                            };
                        buf.core.delete_range(range);
                        buf.core.commit();
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: true,
                        });
                    }
                    Action::Yank => {
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: false,
                        });
                    }
                    Action::Change => {
                        let pos = buf.core.cursor();
                        buf.core.set_cursor(Cursor {
                            row: pos.row,
                            col: 0,
                        });
                        for _ in 0..buf.core.core_buffer().len_line(pos.row) {
                            buf.core.delete();
                        }
                        buf.core.commit();
                        buf.indent();
                        return Insert::default().into_transition();
                    }
                }
            }

            if c == 'j' || c == 'k' {
                let (l, r) = if c == 'j' {
                    if buf.core.cursor().row == buf.core.core_buffer().len_lines() - 1 {
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: false,
                        });
                    }
                    let next_line = buf.core.core_buffer().len_line(buf.core.cursor().row + 1);
                    (
                        Cursor {
                            row: buf.core.cursor().row,
                            col: 0,
                        },
                        Cursor {
                            row: buf.core.cursor().row + 1,
                            col: next_line,
                        },
                    )
                } else {
                    if buf.core.cursor().row == 0 {
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: false,
                        });
                    }

                    (
                        Cursor {
                            row: buf.core.cursor().row - 1,
                            col: 0,
                        },
                        Cursor {
                            row: buf.core.cursor().row,
                            col: buf.core.len_current_line(),
                        },
                    )
                };

                let range = if r.row == buf.core.core_buffer().len_lines() - 1 {
                    (Bound::Included(l), Bound::Excluded(r))
                } else {
                    (Bound::Included(l), Bound::Included(r))
                };

                buf.yank = Yank {
                    insert_newline: true,
                    content: buf.core.get_string_range(l..r),
                };
                match self.parser.action {
                    // dj or dk
                    Action::Delete => {
                        buf.core.delete_range(range);
                        buf.core.commit();
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: true,
                        });
                    }
                    Action::Yank => {
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: false,
                        });
                    }
                    Action::Change => {
                        buf.core.delete_range(range);
                        buf.core.insert_newline_here();
                        buf.core.commit();
                        buf.indent();
                        return Insert::default().into_transition();
                    }
                }
            }

            if let Some(range) = self.parser.parse(c, &buf.core) {
                let range_str = buf.core.get_string_range(range);
                if !range_str.is_empty() {
                    buf.yank = Yank {
                        insert_newline: false,
                        content: range_str,
                    };
                    match self.parser.action {
                        Action::Delete => {
                            buf.core.delete_range(range);
                            buf.core.commit();
                            return Transition::Return(TransitionReturn {
                                message: None,
                                is_commit_dot_macro: true,
                            });
                        }
                        Action::Change => {
                            buf.core.delete_range(range);
                            buf.core.commit();
                            return Insert::default().into_transition();
                        }
                        Action::Yank => {
                            return Transition::Return(TransitionReturn {
                                message: None,
                                is_commit_dot_macro: false,
                            });
                        }
                    }
                } else {
                    return Transition::Return(TransitionReturn {
                        message: None,
                        is_commit_dot_macro: true,
                    });
                }
            }
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height() - 1;
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height, 0), 1, width);

        match self.parser.action {
            Action::Change => {
                footer.puts("Change ", draw::styles::FOOTER);
            }
            Action::Delete => {
                footer.puts("Delete ", draw::styles::FOOTER);
            }
            Action::Yank => {
                footer.puts("Yank ", draw::styles::FOOTER);
            }
        }

        cursor
    }
}

impl<B: CoreBuffer, R: RangeBounds<Cursor> + Clone> Mode<B> for S<R> {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                let l = match self.0.start_bound() {
                    Bound::Excluded(&c) => buf.core.next_cursor(c).unwrap_or(c),
                    Bound::Included(&c) => c,
                    Bound::Unbounded => Cursor { row: 0, col: 0 },
                };
                let r = match self.0.end_bound() {
                    Bound::Excluded(&c) => buf.core.prev_cursor(c).unwrap_or(c),
                    Bound::Included(&c) => c,
                    Bound::Unbounded => Cursor { row: 0, col: 0 },
                };

                let (cl, cr) = parenthesis::PARENTHESIS_PAIRS
                    .iter()
                    .map(|(l, r)| (*l, *r))
                    .find(|&(l, r)| c == l || c == r)
                    .unwrap_or((c, c));

                buf.core.set_cursor(r);
                buf.core.cursor_inc();
                buf.core.insert(cr);
                buf.core.set_cursor(l);
                buf.core.insert(cl);
                buf.core.commit();

                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height();
        let width = view.width();
        let range = self.0.clone();
        buf.draw_with_selected(view.view((0, 0), height, width), Some(range))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide)
    }
}

impl<B: CoreBuffer> Mode<B> for Find {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                let cursor = buf.core.cursor();
                let range: Box<dyn Iterator<Item = usize>> = if self.to_right {
                    Box::new(cursor.col + 1..buf.core.len_current_line())
                } else {
                    Box::new((0..cursor.col).rev())
                };

                for i in range {
                    if buf.core.core_buffer().char_at(Cursor {
                        row: cursor.row,
                        col: i,
                    }) == Some(c)
                    {
                        buf.core.set_cursor(Cursor {
                            row: cursor.row,
                            col: i,
                        });
                        break;
                    }
                }
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            _ => {}
        }
        Transition::Nothing
    }
    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height();
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height - 1, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height - 1, 0), 1, width);
        if self.to_right {
            footer.puts("find ->", draw::styles::FOOTER);
        } else {
            footer.puts("find <-", draw::styles::FOOTER);
        }

        cursor
    }
}

impl<B: CoreBuffer> Mode<B> for Goto {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(TransitionReturn {
                    message: None,
                    is_commit_dot_macro: false,
                });
            }
            Event::Key(Key::Backspace) => {
                self.row.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    if let Ok(mut row) = self.row.iter().collect::<String>().parse::<usize>() {
                        if row > 0 {
                            row -= 1;
                        }
                        row = min(row, buf.core.core_buffer().len_lines() - 1);

                        buf.core.set_cursor(Cursor { row, col: 0 });
                        buf.show_cursor();
                        return Transition::Return(TransitionReturn {
                            message: None,
                            is_commit_dot_macro: false,
                        });
                    } else {
                        return Transition::Return(TransitionReturn {
                            message: Some("[Goto] Parse failed".to_string()),
                            is_commit_dot_macro: false,
                        });
                    }
                } else {
                    self.row.push(c);
                }
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        let height = view.height() - 1;
        let width = view.width();
        let cursor = buf
            .draw(view.view((0, 0), height, width))
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = view.view((height, 0), 1, width);
        footer.puts("Goto: ", draw::styles::DEFAULT);
        for &c in &self.row {
            footer.put(c, draw::styles::DEFAULT, None);
        }

        cursor
    }
}
