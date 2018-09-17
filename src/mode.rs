use buffer::Buffer;
use clipboard;
use core::Core;
use core::Cursor;
use core::CursorRange;
use draw;
use indent;
use racer;
use rustfmt;
use shellexpand;
use std;
use std::cmp::{max, min};
use std::num::Wrapping;
use std::path::PathBuf;
use termion;
use termion::event::{Event, Key, MouseButton, MouseEvent};

pub enum Transition {
    Nothing,
    Trans(Box<Mode>),
    Exit,
}

pub trait Mode {
    fn init(&mut self, &mut Buffer) {}
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition;
    fn draw(&self, core: &Buffer, term: &mut draw::Term);
}

pub struct Normal {
    message: String,
}
struct Prefix;
struct Insert {
    completion_index: Option<usize>,
    completion: Vec<(String, bool)>,
    buf_update: Wrapping<usize>,
}
impl Default for Insert {
    fn default() -> Self {
        Insert {
            completion_index: None,
            completion: Vec::new(),
            buf_update: Wrapping(0),
        }
    }
}
struct R;
struct Search;
struct Save {
    path: String,
}
struct Visual {
    cursor: Cursor,
    line_mode: bool,
}

impl Normal {
    pub fn new() -> Self {
        Self {
            message: String::new(),
        }
    }

    pub fn with_message(message: String) -> Self {
        Self { message }
    }
}

impl Mode for Normal {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Char('u')) => buf.core.undo(),
            Event::Key(Key::Char('U')) => buf.core.redo(),
            Event::Key(Key::Char('i')) => return Transition::Trans(Box::new(Insert::default())),
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
                return Transition::Trans(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('a')) => {
                buf.core.cursor_right();
                return Transition::Trans(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('A')) => {
                let mut c = buf.core.cursor();
                c.col = buf.core.current_line().len();
                buf.core.set_cursor(c);
                return Transition::Trans(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('r')) => return Transition::Trans(Box::new(R)),
            Event::Key(Key::Char('o')) => {
                let indent = indent::next_indent_level(buf.core.current_line());
                buf.core.insert_newline();
                for _ in 0..4 * indent {
                    buf.core.insert(' ');
                }
                return Transition::Trans(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('O')) => {
                buf.core.cursor_up();
                let indent = indent::next_indent_level(buf.core.current_line());
                buf.core.insert_newline();
                for _ in 0..4 * indent {
                    buf.core.insert(' ');
                }
                return Transition::Trans(Box::new(Insert::default()));
            }
            Event::Key(Key::Char('h')) => buf.core.cursor_left(),
            Event::Key(Key::Char('j')) => buf.core.cursor_down(),
            Event::Key(Key::Char('k')) => buf.core.cursor_up(),
            Event::Key(Key::Char('l')) => buf.core.cursor_right(),
            Event::Key(Key::Char('w')) => {
                while {
                    buf.core
                        .char_at_cursor()
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(true)
                        && buf.core.cursor_inc()
                } {}
                while {
                    buf.core
                        .char_at_cursor()
                        .map(|c| !c.is_alphanumeric())
                        .unwrap_or(true)
                        && buf.core.cursor_inc()
                } {}
            }
            Event::Key(Key::Char('b')) => {
                buf.core.cursor_dec();
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true)
                        && buf.core.cursor_dec()
                } {}
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true)
                        && buf.core.cursor_dec()
                } {}
                if buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
                    buf.core.cursor_inc();
                }
            }
            Event::Key(Key::Char('e')) => {
                buf.core.cursor_inc();
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true)
                        && buf.core.cursor_inc()
                } {}
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true)
                        && buf.core.cursor_inc()
                } {}
                if buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
                    buf.core.cursor_dec();
                }
            }
            Event::Key(Key::Char('G')) => {
                let row = buf.core.buffer().len() - 1;
                let col = buf.core.buffer()[row].len();
                buf.core.set_cursor(Cursor { row, col });
                buf.core.set_offset();
            }
            Event::Key(Key::Char('x')) => {
                buf.core.delete();
                buf.core.commit();
            }
            Event::Key(Key::Char('/')) => return Transition::Trans(Box::new(Search)),
            Event::Key(Key::Char('v')) => {
                return Transition::Trans(Box::new(Visual {
                    cursor: buf.core.cursor(),
                    line_mode: false,
                }))
            }
            Event::Key(Key::Char('V')) => {
                return Transition::Trans(Box::new(Visual {
                    cursor: buf.core.cursor(),
                    line_mode: true,
                }))
            }
            Event::Key(Key::Char('p')) => {
                if buf.yank.insert_newline {
                    buf.core.insert_newline();
                }

                for c in buf.yank.content.chars() {
                    buf.core.insert(c);
                }
                buf.core.commit();
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
            }
            Event::Key(Key::Char(' ')) => return Transition::Trans(Box::new(Prefix)),

            Event::Mouse(MouseEvent::Press(MouseButton::Left, x, y)) => {
                let col = x as usize - 1;
                let row = y as usize - 1;
                let cursor = Cursor { row, col };

                let mut term = draw::Term::new();
                let height = term.height;
                let width = term.width;
                buf.draw(term.view((0, 0), height, width));

                if let Some(c) = term.pos(cursor) {
                    buf.core.set_cursor(c);
                }
            }

            Event::Mouse(MouseEvent::Press(MouseButton::WheelUp, _, _)) => {
                if buf.core.row_offset < 3 {
                    buf.core.row_offset = 0;
                } else {
                    buf.core.row_offset -= 3;
                }
            }
            Event::Mouse(MouseEvent::Press(MouseButton::WheelDown, _, _)) => {
                buf.core.row_offset =
                    std::cmp::min(buf.core.row_offset + 3, buf.core.buffer().len() - 1);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height - 1, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = term.view((height - 1, 0), 1, width);
        footer.puts(
            &format!(
                "[Normal] ({} {}) [{}] {}",
                buf.core.cursor().row + 1,
                buf.core.cursor().col + 1,
                buf.path
                    .as_ref()
                    .map(|p| p.to_string_lossy())
                    .unwrap_or("*".into()),
                &self.message
            ),
            draw::CharStyle::Footer,
        );
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

    fn build_completion(&mut self, buf: &mut Buffer) {
        if self.buf_update != buf.core.buffer_changed {
            buf.racer_session
                .cache_file_contents("main.rs", buf.core.get_string());
            self.buf_update = buf.core.buffer_changed;
        }
        self.completion.clear();
        let prefix = Self::token(&buf.core);
        let semi_colon = {
            let i = buf.core.cursor().col;
            i > 0 && buf.core.current_line()[i - 1] == ':'
        };
        // racer
        if prefix.len() > 0 || semi_colon {
            let matches = racer::complete_from_file(
                "main.rs",
                racer::Location::from(racer::Coordinate::new(
                    buf.core.cursor().row as u32 + 1,
                    buf.core.cursor().col as u32,
                )),
                &buf.racer_session,
            );

            for m in matches {
                if prefix != m.matchstr {
                    self.completion.push((m.matchstr, false));
                }
            }
        }
        // snippet
        if !prefix.is_empty() {
            for keyword in buf.snippet.keys().filter(|k| k.starts_with(&prefix)) {
                self.completion.push((keyword.to_string(), true));
            }
        }

        if self.completion.is_empty() {
            self.completion_index = None;
        } else {
            if let Some(index) = self.completion_index {
                self.completion_index = Some(min(index, self.completion.len() - 1));
            }
        }
    }
}

impl Mode for Insert {
    fn init(&mut self, buf: &mut Buffer) {
        self.build_completion(buf);
    }
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Esc) => {
                buf.core.commit();
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Backspace) => {
                buf.core.cursor_dec();
                buf.core.delete();
            }
            Event::Key(Key::Delete) => {
                buf.core.delete();
            }
            Event::Key(Key::Char('\t')) => {
                let comp_len = self.completion.len();
                if comp_len > 0 {
                    if let Some(index) = self.completion_index {
                        self.completion_index = Some((index + 1) % comp_len);
                    } else {
                        self.completion_index = Some(0);
                    }
                } else {
                    buf.core.insert(' ');
                    while buf.core.cursor().col % 4 != 0 {
                        buf.core.insert(' ');
                    }
                }
            }
            Event::Key(Key::Char(c)) => {
                // Auto pair
                let pairs = [('(', ')'), ('{', '}'), ('[', ']'), ('"', '"')];

                if pairs.iter().any(|p| p.1 == c) && buf.core.char_at_cursor() == Some(c) {
                    buf.core.cursor_right();
                } else {
                    if c == '\n' && self.completion_index.is_some() {
                        let (key, is_snip) = &self.completion[self.completion_index.unwrap()];
                        let body = if *is_snip { &buf.snippet[key] } else { key };
                        Self::remove_token(&mut buf.core);
                        for c in body.chars() {
                            buf.core.insert(c);
                        }
                        buf.core.set_offset();
                        self.completion_index = None;
                    } else {
                        buf.core.insert(c);
                        let pair = pairs.iter().find(|p| p.0 == c);
                        if let Some((_, r)) = pair {
                            buf.core.insert(*r);
                            buf.core.cursor_left();
                        }

                        if c == '\n' {
                            let indent = indent::next_indent_level(
                                &buf.core.buffer()[buf.core.cursor().row - 1],
                            );
                            for _ in 0..4 * indent {
                                buf.core.insert(' ');
                            }
                            let pos = buf.core.cursor();
                            if ['}', ']']
                                .into_iter()
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
                }
            }
            _ => {}
        }
        self.build_completion(buf);
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Bar))
            .unwrap_or(draw::CursorState::Hide);

        let completion_height = height - cursor.map(|c| c.row).unwrap_or(0);
        let completion_width = width - cursor.map(|c| c.col).unwrap_or(0);

        if let Some(cursor) = cursor {
            if cursor.col + completion_width <= width && cursor.row + completion_height <= height {
                let mut view = term.view(cursor.to_tuple(), completion_height, completion_width);
                for (i, (s, _)) in self.completion.iter().take(completion_height).enumerate() {
                    for c in s.chars().take(completion_width - 1) {
                        if Some(i) == self.completion_index {
                            view.put(c, draw::CharStyle::Highlight, None);
                        } else {
                            view.put(c, draw::CharStyle::UI, None);
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
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Char(c)) => {
                core.replace(c);
                return Transition::Trans(Box::new(Normal::new()));
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
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
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Backspace) => {
                buf.search.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    return Transition::Trans(Box::new(Normal::new()));
                }
                buf.search.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
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
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Backspace) => {
                self.path.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    let path: String = shellexpand::tilde(&self.path).into();
                    buf.path = Some(PathBuf::from(path.clone()));
                    let message = if buf.save().unwrap().is_ok() {
                        format!("Saved to {}", path)
                    } else {
                        format!("Failed to save {}", path)
                    };
                    return Transition::Trans(Box::new(Normal::with_message(message)));
                }
                self.path.push(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
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
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Char(' ')) => {
                let src = buf.core.get_string();
                if let Some(formatted) = rustfmt::system_rustfmt(&src) {
                    if formatted != src {
                        buf.core.set_string(&formatted, false);
                    }
                }

                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Char('q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Char('s')) => {
                if let Some(ref path) = buf.path {
                    // Rustfmt
                    let mut content = buf.core.get_string();
                    if let Some(formatted) = rustfmt::system_rustfmt(&content) {
                        buf.core.set_string(&formatted, false);
                    }
                    let message = if buf.save().unwrap().is_ok() {
                        format!("Saved to {}", path.to_string_lossy())
                    } else {
                        format!("Failed to save {}", path.to_string_lossy())
                    };
                    return Transition::Trans(Box::new(Normal::with_message(message)));
                } else {
                    return Transition::Trans(Box::new(Save {
                        path: String::new(),
                    }));
                }
            }
            Event::Key(Key::Char('a')) => {
                if let Some(ref path) = buf.path {
                    return Transition::Trans(Box::new(Save {
                        path: path.to_string_lossy().into(),
                    }));
                } else {
                    return Transition::Trans(Box::new(Save {
                        path: String::new(),
                    }));
                }
            }
            Event::Key(Key::Char('y')) => {
                let result = clipboard::clipboard_copy(&buf.core.get_string());
                return Transition::Trans(Box::new(Normal::with_message(
                    if result {
                        "Copied"
                    } else {
                        "Failed to copy to clipboard"
                    }.into(),
                )));
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
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
    fn get_range(&self, to: Cursor, buf: &Vec<Vec<char>>) -> CursorRange {
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
                return Transition::Trans(Box::new(Normal::new()));
            }
            Event::Key(Key::Char('h')) => buf.core.cursor_left(),
            Event::Key(Key::Char('j')) => buf.core.cursor_down(),
            Event::Key(Key::Char('k')) => buf.core.cursor_up(),
            Event::Key(Key::Char('l')) => buf.core.cursor_right(),
            Event::Key(Key::Char('w')) => {
                while {
                    buf.core
                        .char_at_cursor()
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(true)
                        && buf.core.cursor_inc()
                } {}
                while {
                    buf.core
                        .char_at_cursor()
                        .map(|c| !c.is_alphanumeric())
                        .unwrap_or(true)
                        && buf.core.cursor_inc()
                } {}
            }
            Event::Key(Key::Char('b')) => {
                buf.core.cursor_dec();
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true)
                        && buf.core.cursor_dec()
                } {}
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true)
                        && buf.core.cursor_dec()
                } {}
                if buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
                    buf.core.cursor_inc();
                }
            }
            Event::Key(Key::Char('e')) => {
                buf.core.cursor_inc();
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true)
                        && buf.core.cursor_inc()
                } {}
                while {
                    buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) == Some(true)
                        && buf.core.cursor_inc()
                } {}
                if buf.core.char_at_cursor().map(|c| c.is_alphanumeric()) != Some(true) {
                    buf.core.cursor_dec();
                }
            }
            Event::Key(Key::Char('d')) => {
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                let s = if self.line_mode {
                    buf.core.get_string_by_range(range).trim_right().to_string()
                } else {
                    buf.core.get_string_by_range(range)
                };
                buf.core.set_cursor(range.l());
                buf.core.delete_from_cursor(range.r());
                buf.core.commit();
                buf.yank.insert_newline = self.line_mode;
                buf.yank.content = s;
                return Transition::Trans(Box::new(Normal::with_message("Deleted".into())));
            }
            Event::Key(Key::Char('y')) => {
                let range = self.get_range(buf.core.cursor(), buf.core.buffer());
                let s = if self.line_mode {
                    buf.core.get_string_by_range(range).trim_right().to_string()
                } else {
                    buf.core.get_string_by_range(range)
                };
                buf.core.set_cursor(range.l());
                buf.yank.insert_newline = self.line_mode;
                buf.yank.content = s;
                return Transition::Trans(Box::new(Normal::with_message("Yanked".into())));
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let range = self.get_range(buf.core.cursor(), buf.core.buffer());
        let cursor = buf.draw_with_selected(term.view((0, 0), height, width), Some(range));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);
    }
}
