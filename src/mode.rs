use buffer::Buffer;
use buffer::Yank;
use clipboard;
use core::Core;
use core::Cursor;
use core::CursorRange;
use draw;
use indent;
use lsp::LSPClient;
use racer;
use shellexpand;
use std;
use std::cmp::{max, min};
use std::ffi::OsString;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::num::Wrapping;
use std::path::PathBuf;
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use termion;
use termion::event::{Event, Key, MouseButton, MouseEvent};
use text_object;

pub enum Transition {
    Nothing,
    Trans(Box<Mode>),
    RecordMacro(Box<Mode>),
    DoMacro,
    // Message, is commit dot macro?
    Return(Option<String>, bool),
    Exit,
}

impl<T: Mode + 'static> From<T> for Transition {
    fn from(mode: T) -> Transition {
        Transition::Trans(Box::new(mode))
    }
}

pub trait Mode {
    fn init(&mut self, &mut Buffer) {}
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition;
    fn draw(&mut self, core: &mut Buffer, term: &mut draw::Term);
}

pub struct Normal {
    message: String,
    frame: usize,
}

pub struct Completion {
    pub keyword: String,
    pub doc: String,
}

struct Prefix;
struct Insert {
    completion_index: Option<usize>,
    buf_update: Wrapping<usize>,
    racer_count: usize,
    current_racer_id: usize,
    completions: Vec<Completion>,
    snippet_completions: Vec<String>,
    racer_tx: mpsc::Sender<(usize, Vec<Completion>)>,
    racer_rx: mpsc::Receiver<(usize, Vec<Completion>)>,
}
impl Default for Insert {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Insert {
            completion_index: None,
            completions: Vec::new(),
            snippet_completions: Vec::new(),
            buf_update: Wrapping(0),
            current_racer_id: 0,
            racer_count: 1,
            racer_tx: tx,
            racer_rx: rx,
        }
    }
}
struct R;
struct S(CursorRange);
struct Find {
    to_right: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Action {
    Delete,
    Yank,
    Change,
}

impl Action {
    fn from_char(c: char) -> Option<Self> {
        match c {
            'd' => Some(Action::Delete),
            'y' => Some(Action::Yank),
            'c' => Some(Action::Change),
            _ => None,
        }
    }

    fn to_char(self) -> char {
        match self {
            Action::Delete => 'd',
            Action::Yank => 'y',
            Action::Change => 'c',
        }
    }
}

struct TextObjectOperation {
    action: Action,
    parser: text_object::TextObjectParser,
}

impl TextObjectOperation {
    fn new(action: Action) -> Self {
        Self {
            action,
            parser: text_object::TextObjectParser::default(),
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
}

impl ViewProcess {
    fn with_process(mut child: process::Child) -> Option<Self> {
        let now = Instant::now();
        let stdout = child.stdout.take()?;
        let stderr = child.stderr.take()?;
        let (tx, rx) = mpsc::channel();
        let tx1 = tx.clone();
        let tx2 = tx.clone();

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

impl Mode for Normal {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
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
                let mut i = 0;
                {
                    let line = buf.core.current_line();
                    while i < line.len() && line[i] == ' ' {
                        i += 1;
                    }
                }
                let mut c = buf.core.cursor();
                c.col = i;
                buf.core.set_cursor(c);
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('S')) => {
                let mut c = buf.core.cursor();
                c.col = 0;
                buf.core.set_cursor(c);
                for _ in 0..buf.core.current_line().len() {
                    buf.core.delete()
                }
                buf.core.indent();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('C')) => {
                while buf.core.char_at_cursor().is_some() {
                    buf.core.delete()
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
                c.col = buf.core.current_line().len();
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
                buf.core.indent();
                buf.show_cursor();
                return Transition::RecordMacro(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('O')) => {
                buf.core.insert_newline_here();
                buf.core.indent();
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
                return Find { to_right: true }.into();
            }
            Event::Key(Key::Char('F')) => {
                return Find { to_right: false }.into();
            }
            Event::Key(Key::Char('g')) => {
                buf.core.set_cursor(Cursor { row: 0, col: 0 });
                buf.show_cursor();
            }
            Event::Key(Key::Char('G')) => {
                let row = buf.core.buffer().len() - 1;
                let col = buf.core.buffer()[row].len();
                buf.core.set_cursor(Cursor { row, col });
                buf.show_cursor();
            }
            Event::Key(Key::Char('n')) => {
                if !buf.search.is_empty() {
                    let orig_pos = buf.core.cursor();
                    if !buf.core.cursor_inc() {
                        buf.core.set_cursor(Cursor { row: 0, col: 0 });
                    }

                    loop {
                        let matched = buf.core.current_line_after_cursor().len()
                            >= buf.search.len()
                            && &buf.core.current_line_after_cursor()[..buf.search.len()]
                                == buf.search.as_slice();
                        if matched || buf.core.cursor() == orig_pos {
                            buf.show_cursor();
                            break;
                        }

                        if !buf.core.cursor_inc() {
                            buf.core.set_cursor(Cursor { row: 0, col: 0 });
                        }
                    }
                }
            }
            Event::Key(Key::Char('N')) => {
                if !buf.search.is_empty() {
                    let last_pos = Cursor {
                        row: buf.core.buffer().len() - 1,
                        col: buf.core.buffer().last().unwrap().len(),
                    };
                    let orig_pos = buf.core.cursor();
                    if !buf.core.cursor_dec() {
                        buf.core.set_cursor(last_pos);
                    }

                    loop {
                        let matched = buf.core.current_line_after_cursor().len()
                            >= buf.search.len()
                            && &buf.core.current_line_after_cursor()[..buf.search.len()]
                                == buf.search.as_slice();
                        if matched || buf.core.cursor() == orig_pos {
                            buf.show_cursor();
                            break;
                        }

                        if !buf.core.cursor_dec() {
                            buf.core.set_cursor(last_pos);
                        }
                    }
                }
            }
            Event::Key(Key::Char('x')) => {
                buf.core.delete();
                buf.core.commit();
                buf.show_cursor();
            }
            Event::Key(Key::Char('/')) => return Search.into(),
            Event::Key(Key::Char('v')) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: false,
                }
                .into();
            }
            Event::Key(Key::Char('V')) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: true,
                }
                .into();
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
                if let Some(s) = clipboard::clipboard_paste() {
                    for c in s.chars() {
                        buf.core.insert(c);
                    }
                    buf.core.commit();
                } else {
                    self.message = "Failed to paste from clipboard".into();
                }
                buf.show_cursor();
            }
            Event::Key(Key::Char(' ')) => {
                return Prefix.into();
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
                    buf.core.set_cursor(c);
                }
            }
            Event::Mouse(MouseEvent::Hold(_, _)) => {
                return Visual {
                    cursor: buf.core.cursor(),
                    line_mode: false,
                }
                .into();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                buf.scroll_up();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                buf.scroll_down();
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

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height - 1, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height - 1, 0), 1, width);
        if let Some(message) = buf.rustc_message() {
            footer.puts(message, draw::CharStyle::Footer);
        } else {
            footer.puts(
                &format!(
                    "[Normal] ({} {}) [{}] {}",
                    buf.core.cursor().row + 1,
                    buf.core.cursor().col + 1,
                    buf.path
                        .as_ref()
                        .map(|p| p.to_string_lossy())
                        .unwrap_or_else(|| "*".into()),
                    &self.message
                ),
                draw::CharStyle::Footer,
            );
            if buf.is_compiling() {
                let animation = [
                    '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏',
                ];
                let a = animation[self.frame % animation.len()];
                footer.puts(&format!(" {}Compiling ...", a), draw::CharStyle::Footer);
            }
        }
        self.frame = (std::num::Wrapping(self.frame) + std::num::Wrapping(1)).0;
    }
}

impl Insert {
    fn token(core: &Core) -> String {
        let line = core.current_line();
        let mut i = core.cursor().col;

        while i > 0 && (line[i - 1].is_alphanumeric() || line[i - 1] == '_') {
            i -= 1;
        }

        line[i..core.cursor().col].iter().collect::<String>()
    }

    fn remove_token(core: &mut Core) {
        let mut i = core.cursor().col;
        while i > 0 && {
            let c = core.current_line()[i - 1];
            c.is_alphanumeric() || c == '_'
        } {
            core.cursor_left();
            core.delete();
            i -= 1;
        }
    }

    fn completion_len(&self) -> usize {
        self.completions.len() + self.snippet_completions.len()
    }

    fn get_completion(&self, buf: &Buffer) -> Option<String> {
        let index = self.completion_index?;
        if index < self.completions.len() {
            Some(self.completions[index].keyword.clone())
        } else {
            Some(buf.snippet[&self.snippet_completions[index - self.completions.len()]].clone())
        }
    }

    fn poll(&mut self, buf: &Buffer) {
        if let Some(lsp) = buf.lsp.as_ref() {
            if let Some(mut completions) = lsp.poll() {
                let token = Self::token(&buf.core);
                completions.retain(|s| s.keyword != token);
                self.completions = completions;
            }
        } else {
            while let Ok((id, snips)) = self.racer_rx.try_recv() {
                if id > self.current_racer_id {
                    self.completions = snips;
                    self.current_racer_id = id;
                }
            }
        }
        if self.completion_len() == 0 {
            self.completion_index = None;
        } else if let Some(index) = self.completion_index {
            self.completion_index = Some(index % self.completion_len());
        }
    }

    fn build_completion(&mut self, buf: &mut Buffer) {
        if self.buf_update == buf.core.buffer_changed {
            return;
        }
        self.buf_update = buf.core.buffer_changed;
        let prefix = Self::token(&buf.core);
        let start_completion = {
            let i = buf.core.cursor().col;
            i > 0 && {
                let c = buf.core.current_line()[i - 1];
                c == ':' || c == '.'
            }
        };
        let id = self.racer_count;
        self.racer_count += 1;
        let tx = self.racer_tx.clone();
        let cursor = buf.core.cursor();
        let src = buf.core.get_string();
        if !prefix.is_empty() || start_completion {
            if let Some(lsp) = buf.lsp.as_mut() {
                // LSP
                lsp.request_completion(buf.core.get_string(), buf.core.cursor());
            } else {
                // racer
                thread::spawn(move || {
                    // racer sometimes crash
                    let cache = racer::FileCache::default();
                    let session = racer::Session::new(&cache);

                    session.cache_file_contents("main.rs", src);

                    let completion = racer::complete_from_file(
                        "main.rs",
                        racer::Location::from(racer::Coordinate::new(
                            cursor.row as u32 + 1,
                            cursor.col as u32,
                        )),
                        &session,
                    )
                    .map(|m| Completion {
                        keyword: m.matchstr,
                        doc: m.docs,
                    })
                    .filter(|s| s.keyword != prefix)
                    .collect();

                    let _ = tx.send((id, completion));
                });
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

impl Mode for Insert {
    fn init(&mut self, buf: &mut Buffer) {
        // Flush completion
        if let Some(lsp) = buf.lsp.as_mut() {
            lsp.poll();
        }
        self.build_completion(buf);
    }
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                buf.core.commit();
                return Transition::Return(None, true);
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                buf.scroll_up();
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                buf.scroll_down();
            }
            Event::Key(Key::Backspace) => {
                let parens = [('{', '}'), ('(', ')'), ('[', ']')];
                buf.core.cursor_dec();
                let c = buf.core.char_at_cursor();
                buf.core.delete();
                if buf.core.char_at_cursor().is_some()
                    && buf.core.char_at_cursor()
                        == parens.iter().find(|t| c == Some(t.0)).map(|t| t.1)
                {
                    buf.core.delete();
                }
                buf.show_cursor();
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
                    buf.core.insert(' ');
                    while buf.core.cursor().col % 4 != 0 {
                        buf.core.insert(' ');
                    }
                }
                return Transition::Nothing;
            }
            Event::Unsupported(v) => {
                // Shift Tab
                if v == [27, 91, 90] {
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
            }
            Event::Key(Key::Char('\n')) => {
                if self.completion_index.is_some() {
                    let body = &self.get_completion(buf).unwrap();
                    Self::remove_token(&mut buf.core);
                    for c in body.chars() {
                        buf.core.insert(c);
                    }
                    // buf.core.set_offset();
                    buf.show_cursor();
                    self.completion_index = None;
                } else {
                    buf.core.insert('\n');
                    let indent =
                        indent::next_indent_level(&buf.core.buffer()[buf.core.cursor().row - 1]);
                    for _ in 0..4 * indent {
                        buf.core.insert(' ');
                    }
                    let pos = buf.core.cursor();
                    if ['}', ']', ')']
                        .iter()
                        .any(|&c| buf.core.char_at_cursor() == Some(c))
                    {
                        buf.core.insert('\n');
                        let i = if indent == 0 { 0 } else { indent - 1 };
                        for _ in 0..4 * i {
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

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        self.poll(buf);
        let height = term.height;
        let width = term.width;
        let mut cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Bar))
            .unwrap_or(draw::CursorState::Hide);

        if let Some(cursor) = cursor.as_mut() {
            cursor.row += 1;
        }
        let completion_height = height - cursor.map(|c| c.row).unwrap_or(0);
        let completion_width = width - cursor.map(|c| c.col).unwrap_or(0);

        if let Some(cursor) = cursor {
            if cursor.col + completion_width <= width && cursor.row + completion_height <= height {
                let mut view = term.view(cursor.to_tuple(), completion_height, completion_width);
                for i in 0..min(completion_height, self.completion_len()) {
                    let is_selected = Some(i) == self.completion_index;
                    if i < self.completions.len() {
                        let c = &self.completions[i];
                        for c in c.keyword.chars() {
                            if is_selected {
                                view.put_inline(c, draw::CharStyle::Highlight, None);
                            } else {
                                view.put_inline(c, draw::CharStyle::UI, None);
                            }
                        }
                        view.put_inline(' ', draw::CharStyle::Default, None);
                        for c in c.doc.chars() {
                            view.put_inline(c, draw::CharStyle::Selected, None);
                        }
                    } else {
                        let i = i - self.completions.len();
                        for c in self.snippet_completions[i].chars() {
                            if is_selected {
                                view.put_inline(c, draw::CharStyle::Highlight, None);
                            } else {
                                view.put_inline(c, draw::CharStyle::UI, None);
                            }
                        }
                    }
                    view.newline();
                }
            }
        }
    }
}

impl Mode for R {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Char(c)) => {
                core.replace(c);
                return Transition::Return(None, true);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Underline))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for Search {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Backspace) => {
                buf.search.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    return Transition::Return(None, false);
                }
                buf.search.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height - 1;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height, 0), 1, width);
        footer.put('/', draw::CharStyle::Default, None);
        for &c in &buf.search {
            footer.put(c, draw::CharStyle::Default, None);
        }
    }
}

impl Mode for Save {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Backspace) => {
                self.path.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    let path: String = shellexpand::tilde(&self.path).into();
                    buf.path = Some(PathBuf::from(path.clone()));
                    let message = if buf.save(false) {
                        format!("Saved to {}", path)
                    } else {
                        format!("Failed to save {}", path)
                    };
                    return Normal::with_message(message).into();
                }
                self.path.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height - 2;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height, 0), 2, width);
        footer.puts(
            &std::env::current_dir().unwrap().to_string_lossy(),
            draw::CharStyle::UI,
        );
        footer.newline();
        footer.puts("> ", draw::CharStyle::UI);
        footer.puts(&self.path, draw::CharStyle::UI);
    }
}

impl Mode for Prefix {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Char(' ')) => {
                buf.core.rustfmt();
                return Transition::Return(None, false);
            }
            Event::Key(Key::Char('q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Char('s')) => {
                if let Some(path) = buf.path.as_ref().map(|p| p.to_string_lossy().into_owned()) {
                    // Rustfmt
                    buf.core.rustfmt();
                    let message = if buf.save(false) {
                        format!("Saved to {}", path)
                    } else {
                        format!("Failed to save {}", path)
                    };
                    return Transition::Return(Some(message), false);
                } else {
                    return Save {
                        path: String::new(),
                    }
                    .into();
                }
            }
            Event::Key(Key::Char('a')) => {
                if let Some(ref path) = buf.path {
                    return Save {
                        path: path.to_string_lossy().into(),
                    }
                    .into();
                } else {
                    return Save {
                        path: String::new(),
                    }
                    .into();
                }
            }
            Event::Key(Key::Char('y')) => {
                let result = clipboard::clipboard_copy(&buf.core.get_string());
                return Transition::Return(
                    Some(
                        if result {
                            "Copied"
                        } else {
                            "Failed to copy to clipboard"
                        }
                        .into(),
                    ),
                    false,
                );
            }
            Event::Key(Key::Char('l')) => {
                buf.lsp = LSPClient::start();
                return Transition::Return(
                    Some(
                        if buf.lsp.is_some() {
                            "LSP Restarted"
                        } else {
                            "Failed to restart LSP"
                        }
                        .into(),
                    ),
                    false,
                );
            }
            Event::Key(Key::Char('t')) | Event::Key(Key::Char('T')) => {
                let is_optimize = event == Event::Key(Key::Char('T'));
                if let Some(path) = buf.path.as_ref().cloned() {
                    buf.core.rustfmt();
                    buf.save(is_optimize);
                    buf.wait_rustc_thread();
                    if let Some(stem) = path.file_stem() {
                        let mut prog = OsString::from("./");
                        prog.push(stem);
                        if let Ok(mut child) = process::Command::new(&prog)
                            .stdout(process::Stdio::piped())
                            .stderr(process::Stdio::piped())
                            .stdin(process::Stdio::piped())
                            .spawn()
                        {
                            if let Some(input) = clipboard::clipboard_paste() {
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = write!(stdin, "{}", input);
                                }
                                if let Some(next_state) = ViewProcess::with_process(child) {
                                    return next_state.into();
                                } else {
                                    return Normal::with_message("Failed to test".into()).into();
                                }
                            } else {
                                return Normal::with_message("Failed to paste".into()).into();
                            }
                        } else {
                            return Normal::with_message(format!("Failed to run {:?}", prog)).into();
                        }
                    } else {
                        return Normal::with_message("Failed to run".into()).into();
                    }
                } else {
                    return Normal::with_message("Save first".into()).into();
                }
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height - 1;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height, 0), 1, width);
        footer.puts(
            "Prefix ... [Esc: Return] [q: Quit] [s: Save] [a: save As ...] [<Space> Rustfmt]",
            draw::CharStyle::Footer,
        );
    }
}

impl Visual {
    fn get_range(&self, to: Cursor, buf: &[Vec<char>]) -> CursorRange {
        if self.line_mode {
            let mut l = min(self.cursor, to);
            let mut r = max(self.cursor, to);

            l.col = 0;
            r.col = buf[r.row].len();

            CursorRange(l, r)
        } else {
            CursorRange(self.cursor, to)
        }
    }
}

impl Mode for Visual {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
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
                let row = buf.core.buffer().len() - 1;
                let col = buf.core.buffer()[row].len();
                buf.core.set_cursor(Cursor { row, col });
                buf.show_cursor();
            }
            Event::Key(Key::Char('d'))
            | Event::Key(Key::Char('x'))
            | Event::Key(Key::Char('s')) => {
                let to_insert = event == Event::Key(Key::Char('s'));
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                let s = if self.line_mode {
                    buf.core.get_string_by_range(range).trim_end().to_string()
                } else {
                    buf.core.get_string_by_range(range)
                };
                let delete_to_end = range.r().row == buf.core.buffer().len() - 1;
                buf.core.delete_range(range);
                if to_insert && range.l().row != range.r().row {
                    if !delete_to_end {
                        buf.core.insert_newline_here();
                    }
                    buf.core.indent();
                }
                buf.core.commit();
                buf.yank.insert_newline = self.line_mode;
                buf.yank.content = s;

                buf.show_cursor();
                return if to_insert {
                    Insert::default().into()
                } else {
                    Transition::Return(Some("Deleted".into()), true)
                };
            }
            Event::Key(Key::Char('p')) | Event::Key(Key::Ctrl('p')) => {
                let is_clipboard = event == Event::Key(Key::Ctrl('p'));
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                buf.core.delete_range(range);
                if is_clipboard {
                    if let Some(s) = clipboard::clipboard_paste() {
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
                return Transition::Return(None, true);
            }
            Event::Key(Key::Char('y')) | Event::Key(Key::Ctrl('y')) => {
                let is_clipboard = event == Event::Key(Key::Ctrl('y'));
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                let s = if self.line_mode {
                    buf.core.get_string_by_range(range).trim_end().to_string()
                } else {
                    buf.core.get_string_by_range(range)
                };
                buf.core.set_cursor(range.l());
                if is_clipboard {
                    clipboard::clipboard_copy(&s);
                } else {
                    buf.yank.insert_newline = self.line_mode;
                    buf.yank.content = s;
                }
                return Transition::Return(Some("Yanked".into()), false);
            }
            Event::Key(Key::Char('S')) => {
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                return S(range).into();
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
                return Normal::default().into();
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

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let range = self.get_range(buf.core.cursor(), buf.core.buffer());
        let cursor = buf.draw_with_selected(term.view((0, 0), height, width), Some(range));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for ViewProcess {
    fn event(&mut self, _buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => Normal::default().into(),
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

    fn draw(&mut self, _buf: &mut Buffer, term: &mut draw::Term) {
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

        let height = term.height;
        let width = term.width;
        term.cursor = draw::CursorState::Hide;
        {
            let mut view = term.view((0, 0), height - 1, width);
            for line in &self.buf[self.row_offset..] {
                view.puts(line, draw::CharStyle::Default);
                view.newline();
            }
            if let Some(end) = self.end {
                view.puts(
                    &format!("{:?}", end - self.start),
                    draw::CharStyle::Highlight,
                );
            }
        }
        {
            let mut view = term.view((height - 1, 0), 1, width);
            view.puts("Esc to return", draw::CharStyle::Footer);
        }
    }
}

impl Mode for TextObjectOperation {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        if event == Event::Key(Key::Esc) {
            return Transition::Return(None, false);
        }
        if let Event::Key(Key::Char(c)) = event {
            if c == self.action.to_char() {
                // Yank current line
                buf.yank = Yank {
                    insert_newline: true,
                    content: buf.core.current_line().iter().collect(),
                };
                match self.action {
                    // dd
                    Action::Delete => {
                        let range = CursorRange(
                            Cursor {
                                row: buf.core.cursor().row,
                                col: 0,
                            },
                            Cursor {
                                row: buf.core.cursor().row,
                                col: buf.core.current_line().len(),
                            },
                        );
                        buf.core.delete_range(range);
                        buf.core.commit();
                        return Transition::Return(None, true);
                    }
                    Action::Yank => {
                        return Transition::Return(None, false);
                    }
                    Action::Change => {
                        let pos = buf.core.cursor();
                        buf.core.set_cursor(Cursor {
                            row: pos.row,
                            col: 0,
                        });
                        for _ in 0..buf.core.current_line().len() {
                            buf.core.delete();
                        }
                        buf.core.commit();
                        buf.core.indent();
                        return Insert::default().into();
                    }
                }
            }

            if c == 'j' || c == 'k' {
                let range = if c == 'j' {
                    if buf.core.cursor().row == buf.core.buffer().len() - 1 {
                        return Transition::Return(None, false);
                    }
                    let next_line = buf.core.buffer()[buf.core.cursor().row + 1].len();
                    CursorRange(
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
                        return Transition::Return(None, false);
                    }
                    CursorRange(
                        Cursor {
                            row: buf.core.cursor().row - 1,
                            col: 0,
                        },
                        Cursor {
                            row: buf.core.cursor().row,
                            col: buf.core.current_line().len(),
                        },
                    )
                };

                buf.yank = Yank {
                    insert_newline: true,
                    content: buf.core.get_string_by_range(range).trim_end().to_string(),
                };
                match self.action {
                    // dj or dk
                    Action::Delete => {
                        buf.core.delete_range(range);
                        buf.core.commit();
                        return Transition::Return(None, true);
                    }
                    Action::Yank => {
                        return Transition::Return(None, false);
                    }
                    Action::Change => {
                        buf.core.delete_range(range);
                        buf.core.insert_newline_here();
                        buf.core.commit();
                        buf.core.indent();
                        return Insert::default().into();
                    }
                }
            }

            if self.parser.parse(c) {
                if let Some(range) = self.parser.get_range(&buf.core) {
                    match self.action {
                        Action::Delete => {
                            buf.core.delete_range(range);
                            buf.core.commit();
                            return Transition::Return(None, true);
                        }
                        Action::Change => {
                            let range = self.parser.get_range(&buf.core).unwrap();
                            buf.core.delete_range(range);
                            buf.core.commit();
                            return Insert::default().into();
                        }
                        Action::Yank => {
                            buf.yank = Yank {
                                insert_newline: false,
                                content: buf.core.get_string_by_range(range),
                            };
                            return Transition::Return(None, false);
                        }
                    }
                } else {
                    return Transition::Return(None, true);
                }
            }
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height - 1;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height, 0), 1, width);

        match self.action {
            Action::Change => {
                footer.puts("Change ", draw::CharStyle::Footer);
            }
            Action::Delete => {
                footer.puts("Delete ", draw::CharStyle::Footer);
            }
            Action::Yank => {
                footer.puts("Yank ", draw::CharStyle::Footer);
            }
        }
    }
}
impl Mode for S {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                let l = self.0.l();
                let r = self.0.r();

                let pairs = [('(', ')'), ('{', '}'), ('[', ']')];
                let (cl, cr) = pairs
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

                return Transition::Return(None, false);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let range = self.0;
        let cursor = buf.draw_with_selected(term.view((0, 0), height, width), Some(range));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for Find {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Return(None, false);
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                let cursor = buf.core.cursor();
                let range: Box<dyn Iterator<Item = usize>> = if self.to_right {
                    Box::new(cursor.col + 1..buf.core.current_line().len())
                } else {
                    Box::new((0..cursor.col).rev())
                };

                for i in range {
                    if buf.core.current_line()[i] == c {
                        buf.core.set_cursor(Cursor {
                            row: cursor.row,
                            col: i,
                        });
                        break;
                    }
                }
                return Transition::Return(None, false);
            }
            _ => {}
        }
        Transition::Nothing
    }
    fn draw(&mut self, buf: &mut Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height - 1, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height - 1, 0), 1, width);
        if self.to_right {
            footer.puts("find ->", draw::CharStyle::Footer);
        } else {
            footer.puts("find <-", draw::CharStyle::Footer);
        }
    }
}
