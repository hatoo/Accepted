use core::Cursor;
use cursor;
use std;
use std::fmt;
use std::io::Write;
use termion;
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharStyle {
    Default,
    Highlight,
}

impl fmt::Display for CharStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CharStyle::Default => write!(f, "{}", termion::color::Fg(termion::color::Reset)),
            CharStyle::Highlight => {
                write!(f, "{}", termion::color::Fg(termion::color::Rgb(255, 0, 0)))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tile {
    Empty,
    Char(char, CharStyle),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Eq, PartialEq, Debug)]
pub enum CursorState {
    Hide,
    Show(Cursor, CursorShape),
}

impl fmt::Display for CursorShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CursorShape::Bar => write!(f, "{}", cursor::Bar),
            CursorShape::Block => write!(f, "{}", cursor::Block),
            CursorShape::Underline => write!(f, "{}", cursor::UnderLine),
        }
    }
}

impl fmt::Display for CursorState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let CursorState::Show(ref cursor, ref shape) = self {
            write!(
                f,
                "{}{}{}",
                termion::cursor::Goto(cursor.col as u16 + 1, cursor.row as u16 + 1),
                shape,
                termion::cursor::Show
            )
        } else {
            write!(f, "{}", termion::cursor::Hide)
        }
    }
}

#[derive(Debug)]
pub struct Term {
    pub height: usize,
    pub width: usize,
    pub cursor: CursorState,
    buf: Vec<Vec<Tile>>,
}

#[derive(Debug)]
pub struct DoubleBuffer {
    front: Term,
    pub back: Term,
}

#[derive(Debug)]
pub struct View<'a> {
    parent: &'a mut Term,
    orig: (usize, usize),
    height: usize,
    width: usize,
}

pub struct SimpleWriter<'a> {
    view: View<'a>,
    pub cursor: Cursor,
}

impl Term {
    fn new() -> Self {
        let (cols, rows) = termion::terminal_size().unwrap();
        let height = rows as usize;
        let width = cols as usize;
        Term {
            height,
            width,
            cursor: CursorState::Hide,
            buf: vec![vec![Tile::Char(' ', CharStyle::Default); width]; height],
        }
    }

    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> View {
        assert!(orig.0 + height <= self.height);
        assert!(orig.1 + width <= self.width);

        View {
            parent: self,
            orig,
            height,
            width,
        }
    }

    fn render(&self) -> Vec<Vec<(char, CharStyle)>> {
        self.buf
            .iter()
            .map(|line| {
                let mut res: Vec<(char, CharStyle)> = Vec::new();
                for ref t in line {
                    match t {
                        &Tile::Char(c, s) => {
                            res.push((*c, *s));
                        }
                        Tile::Empty => {}
                    }
                }

                while res.last() == Some(&(' ', CharStyle::Default)) {
                    res.pop();
                }

                res
            }).collect()
    }
}

impl DoubleBuffer {
    pub fn new() -> Self {
        Self {
            front: Term::new(),
            back: Term::new(),
        }
    }

    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> View {
        self.back.view(orig, height, width)
    }

    pub fn present<T: Write>(&mut self, out: &mut T) {
        let mut edit = false;
        if self.front.height != self.back.height || self.front.width != self.back.width {
            write!(
                out,
                "{}{}",
                termion::clear::All,
                termion::cursor::Goto(1, 1)
            );

            let mut current_style = CharStyle::Default;
            write!(out, "{}", current_style);
            for (i, line) in self.back.render().into_iter().enumerate() {
                for (c, s) in line {
                    if current_style != s {
                        current_style = s;
                        write!(out, "{}", current_style);
                    }
                    write!(out, "{}", c);
                }
                if i < self.back.height - 1 {
                    write!(out, "\r\n");
                }
            }
            edit = true;
        } else {
            let mut current_style = CharStyle::Default;
            write!(out, "{}", current_style);

            for (i, (f, b)) in self
                .front
                .render()
                .into_iter()
                .zip(self.back.render().into_iter())
                .enumerate()
            {
                if f != b {
                    edit = true;
                    write!(out, "{}", termion::cursor::Goto(1, i as u16 + 1));

                    for (c, s) in b {
                        if current_style != s {
                            current_style = s;
                            write!(out, "{}", current_style);
                        }
                        write!(out, "{}", c);
                    }
                    write!(out, "{}", termion::clear::UntilNewline);
                }
            }
        }

        if edit || self.front.cursor != self.back.cursor {
            write!(out, "{}", self.back.cursor);
        }
        std::mem::swap(&mut self.front, &mut self.back);
        self.back = Term::new();
    }
}

impl<'a> View<'a> {
    pub fn put(&mut self, row: usize, col: usize, c: char, style: CharStyle) {
        assert!(row < self.height);
        assert!(col < self.width);
        let w = c.width().unwrap_or(0);

        if w > 0 {
            assert!(col + w < self.width);
            self.parent.buf[self.orig.0 + row][self.orig.1 + col] = Tile::Char(c, style);
            for x in 1..w {
                self.parent.buf[self.orig.0 + row][self.orig.1 + col + x] = Tile::Empty;
            }
        }
    }
}

impl<'a> SimpleWriter<'a> {
    pub fn new(view: View<'a>) -> Self {
        Self {
            cursor: Cursor {
                row: view.orig.0,
                col: view.orig.1,
            },
            view,
        }
    }

    pub fn is_out(&self) -> bool {
        self.cursor.row >= self.view.orig.0 + self.view.height
    }

    pub fn newline(&mut self) -> Option<Cursor> {
        if self.is_out() {
            None
        } else {
            let prev = self.cursor;
            self.cursor.row += 1;
            self.cursor.col = self.view.orig.1;
            Some(prev)
        }
    }

    pub fn put(&mut self, c: char, style: CharStyle) -> Option<Cursor> {
        if self.is_out() {
            return None;
        }

        let prev = self.cursor;
        let w = c.width().unwrap_or(0);
        if w > 0 {
            if self.cursor.col + w >= self.view.orig.1 + self.view.width {
                self.newline();
                if self.is_out() {
                    return None;
                }
            }
            self.view.parent.buf[self.cursor.row][self.cursor.col] = Tile::Char(c, style);
            self.cursor.col += 1;
            for _ in 1..w {
                self.view.parent.buf[self.cursor.row][self.cursor.col] = Tile::Empty;
                self.cursor.col += 1;
            }
            Some(prev)
        } else {
            Some(prev)
        }
    }
}
