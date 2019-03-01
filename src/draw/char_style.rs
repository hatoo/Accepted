use std;
use std::fmt;

use syntect;
use termion;
use termion::color::{AnsiValue, Bg, Fg, Rgb};

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

impl Into<AnsiValue> for Color {
    fn into(self) -> AnsiValue {
        AnsiValue(ansi_colours::ansi256_from_rgb((self.r, self.g, self.b)))
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
            CharModification::Default => write!(f, "{}", termion::style::NoUnderline),
            CharModification::UnderLine => write!(f, "{}", termion::style::Underline),
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
    use super::{CharModification, CharStyle, Color};

    pub const DEFAULT: CharStyle = CharStyle {
        fg: Color {
            r: 255,
            g: 255,
            b: 255,
        },
        bg: Color { r: 0, g: 0, b: 0 },
        modification: CharModification::Default,
    };
    pub const HIGHLIGHT: CharStyle = CharStyle {
        fg: Color { r: 255, g: 0, b: 0 },
        bg: Color { r: 0, g: 0, b: 0 },
        modification: CharModification::Default,
    };
    pub const UI: CharStyle = CharStyle {
        fg: Color {
            r: 128,
            g: 128,
            b: 128,
        },
        bg: Color { r: 0, g: 0, b: 0 },
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
    pub const TAB_BAR: CharStyle = CharStyle {
        fg: Color {
            r: 0xee,
            g: 0xee,
            b: 0xee,
        },
        bg: Color {
            r: 0x41,
            g: 0x69,
            b: 0xe1,
        },
        modification: CharModification::Default,
    };
}

pub struct StyleWithColorType {
    pub is_ansi_color: bool,
    pub style: CharStyle,
}

impl fmt::Display for StyleWithColorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_ansi_color {
            write!(
                f,
                "{}{}{}",
                Fg(Into::<AnsiValue>::into(self.style.fg)),
                Bg(Into::<AnsiValue>::into(self.style.bg)),
                self.style.modification
            )
        } else {
            write!(
                f,
                "{}{}{}",
                Fg(Into::<Rgb>::into(self.style.fg)),
                Bg(Into::<Rgb>::into(self.style.bg)),
                self.style.modification
            )
        }
    }
}

pub struct DiffStyle {
    pub is_ansi_color: bool,
    pub from: CharStyle,
    pub to: CharStyle,
}

impl fmt::Display for DiffStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.from.fg != self.to.fg {
            if self.is_ansi_color {
                write!(f, "{}", Fg(Into::<AnsiValue>::into(self.to.fg)))?
            } else {
                write!(f, "{}", Fg(Into::<Rgb>::into(self.to.fg)))?
            }
        }
        if self.from.bg != self.to.bg {
            if self.is_ansi_color {
                write!(f, "{}", Bg(Into::<AnsiValue>::into(self.to.bg)))?
            } else {
                write!(f, "{}", Bg(Into::<Rgb>::into(self.to.bg)))?
            }
        }
        if self.from.modification != self.to.modification {
            write!(f, "{}", self.to.modification)?
        }
        Ok(())
    }
}
