use std;
use std::fmt;
use std::io::{self, Write};

use syntect;
use syntect::highlighting::FontStyle;
use termion;
use termion::color::{Bg, Fg, Rgb};
use unicode_width::UnicodeWidthChar;

use crate::compiler::CompilerOutput;
use crate::core::Cursor;
use crate::cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Into<Rgb> for Color {
    fn into(self) -> Rgb {
        Rgb(self.r, self.g, self.b)
    }
}

impl From<syntect::highlighting::Color> for Color {
    fn from(scolor: syntect::highlighting::Color) -> Self {
        Self {
            r: scolor.r,
            g: scolor.g,
            b: scolor.b,
        }
    }
}

impl From<syntect::highlighting::Style> for CharStyle {
    fn from(s: syntect::highlighting::Style) -> Self {
        Self {
            bg: s.background.into(),
            fg: s.foreground.into(),
            modification: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharModification {
    Default,
    UnderLine,
}

impl Default for CharModification {
    fn default() -> Self {
        CharModification::Default
    }
}

impl fmt::Display for CharModification {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CharModification::Default => {
                write!(f, "{}", termion::style::NoUnderline)
            }
            CharModification::UnderLine => {
                write!(f, "{}", termion::style::Underline)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharStyle {
    pub fg: Color,
    pub bg: Color,
    pub modification: CharModification,
}

impl CharStyle {
    pub fn fg(fg: Color) -> Self {
        Self {
            fg,
            bg: Default::default(),
            modification: Default::default(),
        }
    }
    pub fn bg(bg: Color) -> Self {
        Self {
            fg: Default::default(),
            bg,
            modification: Default::default(),
        }
    }
    pub fn fg_bg(fg: Color, bg: Color) -> Self {
        Self {
            fg,
            bg,
            modification: Default::default(),
        }
    }
}

pub mod styles {
    use super::{CharStyle, Color, CharModification};

    pub const DEFAULT: CharStyle = CharStyle {
        fg: Color { r: 255, g: 255, b: 255 },
        bg: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        modification: CharModification::Default,
    };
    pub const HIGHLIGHT: CharStyle = CharStyle {
        fg: Color { r: 255, g: 0, b: 0 },
        bg: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        modification: CharModification::Default,
    };
    pub const UI: CharStyle = CharStyle {
        fg: Color {
            r: 128,
            g: 128,
            b: 128,
        },
        bg: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        modification: CharModification::Default,
    };
    pub const FOOTER: CharStyle = CharStyle {
        fg: Color {
            r: 64,
            g: 64,
            b: 64,
        },
        bg: Color {
            r: 200,
            g: 200,
            b: 200,
        },
        modification: CharModification::Default,
    };
    pub const SELECTED: CharStyle = CharStyle {
        fg: Color {
            r: 200,
            g: 200,
            b: 200,
        },
        bg: Color { r: 0, g: 0, b: 0 },
        modification: CharModification::Default,
    };

}

impl fmt::Display for CharStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", Fg(Into::<Rgb>::into(self.fg)), Bg(Into::<Rgb>::into(self.bg)), self.modification)
    }
}

pub struct DiffStyle {
    from: CharStyle,
    to: CharStyle,
}

impl fmt::Display for DiffStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.from.fg != self.to.fg {
            write!(f, "{}", Fg(Into::<Rgb>::into(self.to.fg)))?
        }
        if self.from.bg != self.to.bg {
            write!(f, "{}", Bg(Into::<Rgb>::into(self.to.bg)))?
        }
        if self.from.modification != self.to.modification {
            write!(f, "{}", self.to.modification)?
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tile {
    Empty,
    Char(char, CharStyle, Option<Cursor>),
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

#[derive(Debug, Default)]
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
    pub bg: Option<Color>,
    pub cursor: Cursor,
}

pub struct LinenumView<'a> {
    view: View<'a>,
    current_linenum: usize,
    width: usize,
    rustc_outputs: &'a [CompilerOutput],
}

impl<'a> LinenumView<'a> {
    pub fn new(
        current_linenum: usize,
        max_linenum: usize,
        rustc_outputs: &'a [CompilerOutput],
        view: View<'a>,
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
            buf: vec![vec![Tile::Char(' ', styles::DEFAULT, None); width]; height],
        }
    }
}

impl Term {
    pub fn pos(&self, cursor: Cursor) -> Option<Cursor> {
        for x in (0..=cursor.col).rev() {
            if let Tile::Char(_, _, Some(c)) = self.buf[cursor.row][x] {
                return Some(c);
            }
        }
        None
    }

    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> View {
        assert!(orig.0 + height <= self.height);
        assert!(orig.1 + width <= self.width);

        View {
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

impl DoubleBuffer {
    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> View {
        self.back.view(orig, height, width)
    }

    pub fn present<T: Write>(&mut self, out: &mut T) -> io::Result<()> {
        let edit = if self.front.height != self.back.height || self.front.width != self.back.width {
            write!(
                out,
                "{}{}{}",
                styles::DEFAULT,
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
                            from: current_style,
                            to: s
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

            for (i, (f, b)) in self
                .front
                .render()
                .into_iter()
                .zip(self.back.render().into_iter())
                .enumerate()
            {
                if f != b {
                    edit = true;
                    write!(out, "{}", termion::cursor::Goto(1, i as u16 + 1))?;
                    let mut current_style = styles::DEFAULT;
                    write!(out, "{}", current_style)?;

                    for (c, s) in b {
                        write!(
                            out,
                            "{}",
                            DiffStyle {
                                from: current_style,
                                to: s
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

impl<'a> View<'a> {
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
            self.parent.buf[self.cursor.row][self.cursor.col] = Tile::Char(c, style, pos);
            self.cursor.col += 1;
            for _ in 1..w {
                self.parent.buf[self.cursor.row][self.cursor.col] = Tile::Empty;
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
