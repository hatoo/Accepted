use std;
use std::cell::RefCell;
use std::io::{self, Write};

use termion;
use unicode_width::UnicodeWidthChar;

use crate::compiler::CompilerOutput;
use crate::core::Cursor;

pub mod char_style;
pub mod cursor;

pub use self::char_style::{
    styles, CharModification, CharStyle, Color, DiffStyle, StyleWithColorType,
};
pub use self::cursor::{CursorShape, CursorState};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tile {
    Empty,
    Char(char, CharStyle, Option<Cursor>),
}

#[derive(Debug)]
pub struct Term {
    pub height: usize,
    pub width: usize,
    pub cursor: CursorState,
    buf: RefCell<Vec<Vec<Tile>>>,
}

#[derive(Debug)]
pub struct TermView<'a> {
    parent: &'a Term,
    orig: (usize, usize),
    height: usize,
    width: usize,
    pub bg: Option<Color>,
    pub cursor: Cursor,
}

pub struct LinenumView<'a> {
    view: TermView<'a>,
    current_linenum: usize,
    width: usize,
    rustc_outputs: &'a [CompilerOutput],
}

#[derive(Debug, Default)]
pub struct DoubleBuffer {
    front: Term,
    pub back: Term,
}

impl<'a> LinenumView<'a> {
    pub fn new(
        current_linenum: usize,
        max_linenum: usize,
        rustc_outputs: &'a [CompilerOutput],
        view: TermView<'a>,
    ) -> Self {
        let width = format!("{}", max_linenum + 1).len() + 2;
        let mut res = Self {
            view,
            width,
            current_linenum,
            rustc_outputs,
        };
        res.put_linenum();
        res
    }

    pub fn prefix_width(max_linenum: usize) -> usize {
        format!("{}", max_linenum + 1).len() + 2
    }

    fn put_linenum(&mut self) {
        let s = format!("{}", self.current_linenum + 1);
        let w = s.len();
        for c in s.chars() {
            self.view.put(c, styles::UI, None);
        }

        if let Some(o) = self
            .rustc_outputs
            .iter()
            .find(|o| o.line == self.current_linenum)
        {
            for _ in 0..self.width - w - 1 {
                self.view.put(' ', styles::UI, None);
            }
            self.view
                .put(o.level.chars().next().unwrap(), styles::HIGHLIGHT, None);
        } else {
            for _ in 0..self.width - w {
                self.view.put(' ', styles::UI, None);
            }
        }
    }

    pub fn cursor(&self) -> Option<Cursor> {
        if !self.view.is_out() {
            Some(self.view.cursor)
        } else {
            None
        }
    }

    pub fn cause_newline(&self, c: char) -> bool {
        self.view.cause_newline(c)
    }

    fn put_space(&mut self) {
        for _ in 0..self.width {
            self.view.put(' ', styles::UI, None);
        }
    }

    pub fn put(&mut self, c: char, style: CharStyle, pos: Option<Cursor>) -> Option<Cursor> {
        if self.view.cause_newline(c) {
            self.view.newline();
            self.put_space();
        }
        self.view.put(c, style, pos)
    }

    pub fn newline(&mut self) {
        self.current_linenum += 1;
        self.view.newline();
        self.put_linenum();
    }
}

impl Default for Term {
    fn default() -> Self {
        let (cols, rows) = termion::terminal_size().unwrap();
        let height = rows as usize;
        let width = cols as usize;
        Term {
            height,
            width,
            cursor: CursorState::Hide,
            buf: RefCell::new(vec![
                vec![Tile::Char(' ', styles::DEFAULT, None); width];
                height
            ]),
        }
    }
}

impl Term {
    pub fn pos(&self, cursor: Cursor) -> Option<Cursor> {
        for x in (0..=cursor.col).rev() {
            if let Tile::Char(_, _, Some(c)) = self.buf.borrow()[cursor.row][x] {
                return Some(c);
            }
        }
        None
    }

    pub fn view(&self, orig: (usize, usize), height: usize, width: usize) -> TermView {
        assert!(orig.0 + height <= self.height);
        assert!(orig.1 + width <= self.width);

        TermView {
            parent: self,
            orig,
            height,
            width,
            bg: None,
            cursor: Cursor {
                row: orig.0,
                col: orig.1,
            },
        }
    }

    fn render(&self) -> Vec<Vec<(char, CharStyle)>> {
        self.buf
            .borrow()
            .iter()
            .map(|line| {
                let mut res: Vec<(char, CharStyle)> = Vec::new();
                for t in line {
                    match t {
                        Tile::Char(c, s, _) => {
                            res.push((*c, *s));
                        }
                        Tile::Empty => {}
                    }
                }

                while res.last() == Some(&(' ', styles::DEFAULT)) {
                    res.pop();
                }

                res
            })
            .collect()
    }
}

impl<'a> TermView<'a> {
    pub fn view(&self, orig: (usize, usize), height: usize, width: usize) -> TermView {
        let new_orig = (self.orig.0 + orig.0, self.orig.1 + orig.1);

        assert!(new_orig.0 + height <= self.parent.height);
        assert!(new_orig.1 + width <= self.parent.width);

        Self {
            parent: &self.parent,
            orig: new_orig,
            height,
            width,
            bg: self.bg,
            cursor: Cursor {
                row: new_orig.0,
                col: new_orig.1,
            },
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn is_out(&self) -> bool {
        self.cursor.row >= self.orig.0 + self.height
    }

    pub fn newline(&mut self) -> Option<Cursor> {
        if self.is_out() {
            None
        } else {
            let prev = self.cursor;
            if let Some(bg) = self.bg {
                if !self.cause_newline(' ') {
                    self.put(' ', CharStyle::bg(bg), None);
                }
            }
            self.cursor.row += 1;
            self.cursor.col = self.orig.1;
            Some(prev)
        }
    }

    pub fn cause_newline(&self, c: char) -> bool {
        if self.is_out() {
            return true;
        }

        let w = c.width().unwrap_or(0);
        self.cursor.col + w >= self.orig.1 + self.width
    }

    pub fn put(&mut self, c: char, style: CharStyle, pos: Option<Cursor>) -> Option<Cursor> {
        if self.is_out() {
            return None;
        }

        let prev = self.cursor;
        let w = c.width().unwrap_or(0);
        if w > 0 {
            if self.cursor.col + w >= self.orig.1 + self.width {
                self.newline();
                if self.is_out() {
                    return None;
                }
            }
            self.parent.buf.borrow_mut()[self.cursor.row][self.cursor.col] =
                Tile::Char(c, style, pos);
            self.cursor.col += 1;
            for _ in 1..w {
                self.parent.buf.borrow_mut()[self.cursor.row][self.cursor.col] = Tile::Empty;
                self.cursor.col += 1;
            }
            Some(prev)
        } else {
            Some(prev)
        }
    }

    pub fn puts(&mut self, s: &str, style: CharStyle) {
        for c in s.chars() {
            self.put(c, style, None);
        }
    }

    pub fn put_inline(&mut self, c: char, style: CharStyle, pos: Option<Cursor>) -> Option<Cursor> {
        if self.cause_newline(c) {
            None
        } else {
            self.put(c, style, pos)
        }
    }
}

impl DoubleBuffer {
    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> TermView {
        self.back.view(orig, height, width)
    }

    pub fn present<T: Write>(&mut self, out: &mut T, is_ansi_color: bool) -> io::Result<()> {
        let edit = if self.front.height != self.back.height || self.front.width != self.back.width {
            write!(out, "{}", CursorState::Hide)?;
            write!(
                out,
                "{}{}{}",
                StyleWithColorType {
                    is_ansi_color,
                    style: styles::DEFAULT,
                },
                termion::clear::All,
                termion::cursor::Goto(1, 1)
            )?;

            let mut current_style = styles::DEFAULT;
            for (i, line) in self.back.render().into_iter().enumerate() {
                for &(c, s) in &line {
                    write!(
                        out,
                        "{}",
                        DiffStyle {
                            is_ansi_color,
                            from: current_style,
                            to: s,
                        }
                    )?;
                    current_style = s;
                    write!(out, "{}", c)?;
                }

                if !line.is_empty() {
                    write!(out, "{}", termion::clear::UntilNewline)?;
                }
                if i < self.back.height - 1 {
                    writeln!(out, "\r")?;
                }
            }
            true
        } else {
            let mut edit = false;
            let mut cursor_hided = false;

            for (i, (f, b)) in self
                .front
                .render()
                .into_iter()
                .zip(self.back.render().into_iter())
                .enumerate()
            {
                if f != b {
                    edit = true;
                    if !cursor_hided {
                        cursor_hided = true;
                        write!(out, "{}", CursorState::Hide)?;
                    }
                    write!(out, "{}", termion::cursor::Goto(1, i as u16 + 1))?;
                    let mut current_style = styles::DEFAULT;
                    write!(
                        out,
                        "{}",
                        StyleWithColorType {
                            is_ansi_color,
                            style: current_style,
                        }
                    )?;

                    for (c, s) in b {
                        write!(
                            out,
                            "{}",
                            DiffStyle {
                                is_ansi_color,
                                from: current_style,
                                to: s,
                            }
                        )?;
                        current_style = s;
                        write!(out, "{}", c)?;
                    }
                    write!(out, "{}", termion::clear::UntilNewline)?;
                }
            }
            edit
        };

        if edit || self.front.cursor != self.back.cursor {
            write!(out, "{}", self.back.cursor)?;
        }
        std::mem::swap(&mut self.front, &mut self.back);
        self.back = Term::default();
        Ok(())
    }

    pub fn redraw(&mut self) {
        self.front.height = 0;
        self.front.width = 0;
    }
}
