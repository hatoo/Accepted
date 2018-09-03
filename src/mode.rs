use buffer::Buffer;
use cursor;
use draw;
use std::io::Write;
use termion;

pub enum Transition {
    Nothing,
    Trans(Box<Mode>),
    Exit,
}

fn windows_size() -> (usize, usize) {
    let (cols, rows) = termion::terminal_size().unwrap();
    (rows as usize, cols as usize)
}

fn refresh_screen<T: Write>(w: &mut T) {
    write!(w, "{}{}", termion::clear::All, termion::cursor::Goto(1, 1));
}

pub trait Mode {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition;
    fn draw(&self, core: &Buffer, db: &mut draw::DoubleBuffer);
}

pub struct Normal;
struct Insert;
struct R;
struct Search;

impl Mode for Normal {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        use termion::event::{Event, Key, MouseEvent};
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Ctrl('q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Char('i')) => return Transition::Trans(Box::new(Insert)),
            Event::Key(Key::Char('a')) => {
                core.cursor_right();
                return Transition::Trans(Box::new(Insert));
            }
            Event::Key(Key::Char('r')) => return Transition::Trans(Box::new(R)),
            Event::Key(Key::Char('o')) => {
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

    fn draw(&self, core: &Buffer, db: &mut draw::DoubleBuffer) {
        let height = db.back.height;
        let width = db.back.width;
        let cursor = core.draw(db.view((0, 0), height, width));
        db.back.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for Insert {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        use termion::event::{Event, Key, MouseEvent};
        let core = &mut buf.core;
        match event {
            Event::Key(Key::Ctrl('q')) => {
                return Transition::Exit;
            }
            Event::Key(Key::Esc) => {
                return Transition::Trans(Box::new(Normal));
            }
            Event::Key(Key::Backspace) => {
                core.backspase();
            }
            Event::Key(Key::Char(c)) => {
                core.insert(c);
            }
            _ => {}
        }
        Transition::Nothing
    }

    fn draw(&self, core: &Buffer, db: &mut draw::DoubleBuffer) {
        let height = db.back.height;
        let width = db.back.width;
        let cursor = core.draw(db.view((0, 0), height, width));
        db.back.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Bar))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for R {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        use termion::event::{Event, Key, MouseEvent};
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

    fn draw(&self, core: &Buffer, db: &mut draw::DoubleBuffer) {
        let height = db.back.height;
        let width = db.back.width;
        let cursor = core.draw(db.view((0, 0), height, width));
        db.back.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Underline))
            .unwrap_or(draw::CursorState::Hide);
    }
}

impl Mode for Search {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        use termion::event::{Event, Key, MouseEvent};
        let core = &mut buf.core;
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

    fn draw(&self, core: &Buffer, db: &mut draw::DoubleBuffer) {
        let height = db.back.height - 1;
        let width = db.back.width;
        let cursor = core.draw(db.view((0, 0), height, width));
        db.back.cursor = cursor
            .map(|c| draw::CursorState::Show(c, draw::CursorShape::Block))
            .unwrap_or(draw::CursorState::Hide);

        let mut footer = db.view((height, 0), 1, width);
        footer.put('/', draw::CharStyle::Default);
        for &c in &core.search {
            footer.put(c, draw::CharStyle::Default);
        }
    }
}
