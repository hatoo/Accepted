use std;
use std::fmt;

use syntect;
use termion;
use termion::color::{Bg, Fg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb { r: u8, g: u8, b: u8 },
    Reset,
}

impl Default for Color {
    fn default() -> Self {
        Self::Reset
    }
}

impl Into<Box<dyn termion::color::Color>> for Color {
    fn into(self) -> Box<dyn termion::color::Color> {
        match self {
            Color::Reset => Box::new(termion::color::Reset),
            Color::Rgb { r, g, b } => Box::new(termion::color::Rgb(r, g, b)),
        }
    }
}

impl Color {
    fn to_ansi(self) -> Box<dyn termion::color::Color> {
        match self {
            Color::Reset => Box::new(termion::color::Reset),
            Color::Rgb { r, g, b } => Box::new(termion::color::AnsiValue(
                ansi_colours::ansi256_from_rgb((r, g, b)),
            )),
        }
    }
}

impl From<syntect::highlighting::Color> for Color {
    fn from(scolor: syntect::highlighting::Color) -> Self {
        Self::Rgb {
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
        fg: Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        bg: Color::Reset,
        modification: CharModification::Default,
    };
    pub const HIGHLIGHT: CharStyle = CharStyle {
        fg: Color::Rgb { r: 255, g: 0, b: 0 },
        bg: Color::Rgb { r: 0, g: 0, b: 0 },
        modification: CharModification::Default,
    };
    pub const UI: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        },
        bg: Color::Reset,
        modification: CharModification::Default,
    };
    pub const FOOTER: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 64,
            g: 64,
            b: 64,
        },
        bg: Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
        modification: CharModification::Default,
    };
    pub const FOOTER_HIGHLIGHT: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 215,
            g: 32,
            b: 32,
        },
        bg: Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
        modification: CharModification::Default,
    };
    pub const FOOTER_BLUE: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 0x41,
            g: 0x69,
            b: 0xe1,
        },
        bg: Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
        modification: CharModification::Default,
    };
    pub const SELECTED: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
        bg: Color::Rgb { r: 0, g: 0, b: 0 },
        modification: CharModification::Default,
    };
    pub const TAB_BAR: CharStyle = CharStyle {
        fg: Color::Rgb {
            r: 0xee,
            g: 0xee,
            b: 0xee,
        },
        bg: Color::Rgb {
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
                Fg(self.style.fg.to_ansi().as_ref()),
                Bg(self.style.bg.to_ansi().as_ref()),
                self.style.modification
            )
        } else {
            write!(
                f,
                "{}{}{}",
                Fg(Into::<Box<dyn termion::color::Color>>::into(self.style.fg).as_ref()),
                Bg(Into::<Box<dyn termion::color::Color>>::into(self.style.bg).as_ref()),
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
                write!(f, "{}", Fg(self.to.fg.to_ansi().as_ref()))?
            } else {
                write!(
                    f,
                    "{}",
                    Fg(Into::<Box<dyn termion::color::Color>>::into(self.to.fg).as_ref())
                )?
            }
        }
        if self.from.bg != self.to.bg {
            if self.is_ansi_color {
                write!(f, "{}", Bg(self.to.bg.to_ansi().as_ref()))?
            } else {
                write!(
                    f,
                    "{}",
                    Bg(Into::<Box<dyn termion::color::Color>>::into(self.to.bg).as_ref())
                )?
            }
        }
        if self.from.modification != self.to.modification {
            write!(f, "{}", self.to.modification)?
        }
        Ok(())
    }
}
