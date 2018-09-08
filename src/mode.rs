use buffer::Buffer;
use draw;
use termion;
use termion::event::{Event, Key};

pub enum Transition {
    Nothing,
    Trans(Box<Mode>),
    Exit,
}

pub trait Mode {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition;
    fn draw(&self, core: &Buffer, term: &mut draw::Term);
}

pub struct Normal;
struct Insert;
struct R;
struct Search;

impl Mode for Normal {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Char('Q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Char('i')) => return Transition::Trans(Box::new(Insert)),
            Event::Key(Key::Char('I')) => {
                let mut i = 0;
                {
                    let line = core.current_line();
                    while i < line.len() && line[i] == ' ' {
                        i += 1;
                    }
                }
                core.cursor.col = i;
                return Transition::Trans(Box::new(Insert));
            }
            Event::Key(Key::Char('a')) => {
                core.cursor_right();
                return Transition::Trans(Box::new(Insert));
            }
            Event::Key(Key::Char('r')) => return Transition::Trans(Box::new(R)),
            Event::Key(Key::Char('o')) => {
                core.insert_newline();
                return Transition::Trans(Box::new(Insert));
            }
            Event::Key(Key::Char('O')) => {
                core.cursor_up();
                core.insert_newline();
                return Transition::Trans(Box::new(Insert));
            }
            Event::Key(Key::Char('h')) => core.cursor_left(),
            Event::Key(Key::Char('j')) => core.cursor_down(),
            Event::Key(Key::Char('k')) => core.cursor_up(),
            Event::Key(Key::Char('l')) => core.cursor_right(),
            Event::Key(Key::Char('/')) => return Transition::Trans(Box::new(Search)),
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, buf: &Buffer, term: &mut draw::Term) {
        let height = term.height;
        let width = term.width;
        let cursor = buf.draw(term.view((0, 0), height, width));
        term.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for Insert {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Trans(Box::new(Normal));
            }
            Event::Key(Key::Backspace) => {
                core.backspase();
            }
            Event::Key(Key::Char('\t')) => {
                core.insert(' ');
                while core.cursor.col % 4 != 0 {
                    core.insert(' ');
                }
            }
            Event::Key(Key::Char(c)) => {
                core.insert(c);
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
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Bar))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for R {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Esc) => {
                return Transition::Trans(Box::new(Normal));
            }
            Event::Key(Key::Char(c)) => {
                core.replace(c);
                return Transition::Trans(Box::new(Normal));
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
                return Transition::Trans(Box::new(Normal));
            }
            Event::Key(Key::Backspace) => {
                buf.search.pop();
            }
            Event::Key(Key::Char(c)) => {
                if c == '\n' {
                    return Transition::Trans(Box::new(Normal));
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
        footer.put('/', draw::CharStyle::Default);
        for &c in &buf.search {
            footer.put(c, draw::CharStyle::Default);
        }
    }
}
