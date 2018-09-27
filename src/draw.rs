use core::Cursor;
use cursor;
use rustc::RustcOutput;
use std;
use std::fmt;
use std::io::Write;
use syntect;
use syntect::highlighting::Color;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Style;
use termion;
use termion::color::{Bg, Fg, Rgb};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharStyle {
    Default,
    Highlight,
    UI,
    Footer,
    Selected,
    Style(Style),
}

impl CharStyle {
    pub fn fg_bg(fg: syntect::highlighting::Color, bg: syntect::highlighting::Color) -> Self {
        CharStyle::Style(Style {
            foreground: fg,
            background: bg,
            font_style: FontStyle::default(),
        })
    }

    pub fn bg(color: syntect::highlighting::Color) -> Self {
        CharStyle::Style(Style {
            foreground: Color::WHITE,
            background: color,
            font_style: FontStyle::default(),
        })
    }
}

impl fmt::Display for CharStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CharStyle::Default => write!(
                f,
                "{}{}",
                Bg(termion::color::Reset),
                termion::color::Fg(termion::color::Reset)
            ),
            CharStyle::Highlight => write!(
                f,
                "{}{}{}",
                Bg(termion::color::Reset),
                termion::color::Fg(termion::color::Rgb(255, 0, 0)),
                termion::style::NoUnderline
            ),
            CharStyle::UI => write!(
                f,
                "{}{}{}",
                Bg(termion::color::Reset),
                termion::color::Fg(termion::color::Rgb(128, 128, 128)),
                termion::style::NoUnderline
            ),
            CharStyle::Footer => write!(
                f,
                "{}{}{}",
                Bg(termion::color::Rgb(200, 200, 200)),
                termion::color::Fg(termion::color::Rgb(64, 64, 64)),
                termion::style::NoUnderline
            ),
            CharStyle::Selected => write!(
                f,
                "{}{}{}",
                Bg(termion::color::Rgb(0, 0, 0)),
                termion::color::Fg(termion::color::Rgb(200, 200, 200)),
                termion::style::NoUnderline
            ),
            CharStyle::Style(style) => {
                let fg = style.foreground;
                let bg = style.background;
                let font_style = style.font_style;

                write!(
                    f,
                    "{}{}",
                    Fg(Rgb(fg.r, fg.g, fg.b)),
                    Bg(Rgb(bg.r, bg.g, bg.b)),
                );
                if font_style.contains(FontStyle::UNDERLINE) {
                    write!(f, "{}", termion::style::Underline)
                } else {
                    write!(f, "{}", termion::style::NoUnderline)
                }
            }
        }
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
    pub bg: Option<Color>,
    pub cursor: Cursor,
}

pub struct LinenumView<'a> {
    view: View<'a>,
    current_linenum: usize,
    width: usize,
    rustc_outputs: &'a [RustcOutput],
}

impl<'a> LinenumView<'a> {
    pub fn new(
        current_linenum: usize,
        max_linenum: usize,
        rustc_outputs: &'a [RustcOutput],
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
            self.view.put(c, CharStyle::UI, None);
        }

        if let Some(o) = self
            .rustc_outputs
            .iter()
            .find(|o| o.line == self.current_linenum)
        {
            for _ in 0..self.width - w - 1 {
                self.view.put(' ', CharStyle::UI, None);
            }
            self.view
                .put(o.level.chars().next().unwrap(), CharStyle::Highlight, None);
        } else {
            for _ in 0..self.width - w {
                self.view.put(' ', CharStyle::UI, None);
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

    fn put_space(&mut self) {
        for _ in 0..self.width {
            self.view.put(' ', CharStyle::UI, None);
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
            buf: vec![vec![Tile::Char(' ', CharStyle::Default, None); width]; height],
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

                while res.last() == Some(&(' ', CharStyle::Default)) {
                    res.pop();
                }

                res
            }).collect()
    }
}

impl Default for DoubleBuffer {
    fn default() -> Self {
        Self {
            front: Term::default(),
            back: Term::default(),
        }
    }
}

impl DoubleBuffer {
    pub fn view(&mut self, orig: (usize, usize), height: usize, width: usize) -> View {
        self.back.view(orig, height, width)
    }

    pub fn present<T: Write>(&mut self, out: &mut T) {
        let edit = if self.front.height != self.back.height || self.front.width != self.back.width {
            write!(
                out,
                "{}{}{}",
                CharStyle::Default,
                termion::clear::All,
                termion::cursor::Goto(1, 1)
            );

            let mut current_style = CharStyle::Default;
            for (i, line) in self.back.render().into_iter().enumerate() {
                for &(c, s) in &line {
                    if current_style != s {
                        current_style = s;
                        write!(out, "{}", current_style);
                    }
                    write!(out, "{}", c);
                }

                if !line.is_empty() {
                    write!(out, "{}", termion::clear::UntilNewline);
                }
                if i < self.back.height - 1 {
                    writeln!(out, "\r");
                }
            }
            true
        } else {
            let mut edit = false;
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
                    current_style = CharStyle::Default;
                    write!(out, "{}", termion::cursor::Goto(1, i as u16 + 1));
                    write!(out, "{}", current_style);

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
            edit
        };

        if edit || self.front.cursor != self.back.cursor {
            write!(out, "{}", self.back.cursor);
        }
        std::mem::swap(&mut self.front, &mut self.back);
        self.back = Term::default();
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
            return false;
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
