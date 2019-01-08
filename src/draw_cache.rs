use draw;
use draw::CharStyle;
use syntax;
use syntect::highlighting::Color;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::SyntaxSet;
use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp};

#[derive(Clone)]
struct DrawState {
    parse_state: ParseState,
    highlight_state: HighlightState,
    ops: Vec<(usize, ScopeStackOp)>,
    // 3 of { ( [
    parens_level: [usize; 3],
}

impl DrawState {
    const RAINBOW: [Color; 10] = [
        Color {
            r: 0xE6,
            g: 0x0,
            b: 0x12,
            a: 0xff,
        },
        Color {
            r: 0xf3,
            g: 0x98,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0xff,
            g: 0xf1,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0x8f,
            g: 0xc3,
            b: 0x1f,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x99,
            b: 0x44,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0xa0,
            b: 0xe9,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x68,
            b: 0xb7,
            a: 0xff,
        },
        Color {
            r: 0x92,
            g: 0x07,
            b: 0x83,
            a: 0xff,
        },
        Color {
            r: 0xe4,
            g: 0x00,
            b: 0x7f,
            a: 0xff,
        },
        Color {
            r: 0xe5,
            g: 0x00,
            b: 0x4f,
            a: 0xff,
        },
    ];
    fn new(
        syntax: &syntect::parsing::SyntaxReference,
        highlighter: &syntect::highlighting::Highlighter,
    ) -> Self {
        Self {
            parse_state: ParseState::new(syntax),
            highlight_state: HighlightState::new(highlighter, ScopeStack::new()),
            ops: Vec::new(),
            parens_level: [0, 0, 0],
        }
    }

    fn iter<'a>(
        &'a mut self,
        line: &'a str,
        syntax_set: &'a SyntaxSet,
        highlighter: &'a Highlighter,
        bg: Color,
    ) -> impl Iterator<Item = (char, CharStyle)> + 'a {
        self.ops = self.parse_state.parse_line(line, syntax_set);

        let highlight_state = &mut self.highlight_state;
        let parens_level = &mut self.parens_level;
        let ops = &self.ops[..];

        let iter: HighlightIterator<'a, 'a> =
            HighlightIterator::new(highlight_state, ops, line, highlighter);

        iter.flat_map(move |(style, s)| {
            s.chars()
                .map(|c| {
                    let parens = [('{', '}'), ('(', ')'), ('[', ']')];
                    for (k, (l, r)) in parens.iter().enumerate() {
                        if c == *l {
                            let fg = Self::RAINBOW[parens_level[k] % Self::RAINBOW.len()];
                            parens_level[k] += 1;
                            let style = CharStyle::fg_bg(fg, bg);
                            return (c, style);
                        }
                        if c == *r && parens_level[k] > 0 {
                            parens_level[k] -= 1;
                            let fg = Self::RAINBOW[parens_level[k] % Self::RAINBOW.len()];
                            let style = CharStyle::fg_bg(fg, bg);
                            return (c, style);
                        }
                    }
                    (c, draw::CharStyle::Style(style))
                })
                .collect::<Vec<_>>()
                .into_iter()
        })
    }

    fn next(&mut self, line: &str, syntax_set: &SyntaxSet, highlighter: &Highlighter) {
        self.ops = self.parse_state.parse_line(line, syntax_set);

        let highlight_state = &mut self.highlight_state;
        let parens_level = &mut self.parens_level;
        let ops = &self.ops[..];

        let iter: HighlightIterator =
            HighlightIterator::new(highlight_state, ops, line, highlighter);

        let parens = [('{', '}'), ('(', ')'), ('[', ']')];
        for (style, s) in iter {
            for c in s.chars() {
                for (k, (l, r)) in parens.iter().enumerate() {
                    if c == *l {
                        parens_level[k] += 1;
                    }
                    if c == *r && parens_level[k] > 0 {
                        parens_level[k] -= 1;
                    }
                }
            }
        }
    }
}

pub struct DrawCache<'a> {
    syntax_set: &'a syntect::parsing::SyntaxSet,
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
    // 3 of { ( [
    parens_level: [usize; 3],
    bg: Color,
    cache: Vec<Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    const RAINBOW: [Color; 10] = [
        Color {
            r: 0xE6,
            g: 0x0,
            b: 0x12,
            a: 0xff,
        },
        Color {
            r: 0xf3,
            g: 0x98,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0xff,
            g: 0xf1,
            b: 0x00,
            a: 0xff,
        },
        Color {
            r: 0x8f,
            g: 0xc3,
            b: 0x1f,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x99,
            b: 0x44,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0xa0,
            b: 0xe9,
            a: 0xff,
        },
        Color {
            r: 0x00,
            g: 0x68,
            b: 0xb7,
            a: 0xff,
        },
        Color {
            r: 0x92,
            g: 0x07,
            b: 0x83,
            a: 0xff,
        },
        Color {
            r: 0xe4,
            g: 0x00,
            b: 0x7f,
            a: 0xff,
        },
        Color {
            r: 0xe5,
            g: 0x00,
            b: 0x4f,
            a: 0xff,
        },
    ];

    pub fn new(syntax: &syntax::Syntax<'a>) -> Self {
        let highlighter = Highlighter::new(syntax.theme);
        let hstate = HighlightState::new(&highlighter, ScopeStack::new());
        let bg = syntax.theme.settings.background.unwrap();
        Self {
            syntax_set: syntax.syntax_set,
            highlighter,
            parse_state: ParseState::new(syntax.syntax),
            highlight_state: hstate,
            cache: Vec::new(),
            bg,
            parens_level: [0, 0, 0],
        }
    }

    pub fn cache_line(&mut self, buffer: &[Vec<char>], i: usize) -> &[(char, CharStyle)] {
        if self.cache.len() <= i {
            for i in self.cache.len()..=i {
                let line = buffer[i].iter().collect::<String>();
                let ops = self.parse_state.parse_line(&line, self.syntax_set);
                let iter = HighlightIterator::new(
                    &mut self.highlight_state,
                    &ops[..],
                    &line,
                    &self.highlighter,
                );
                let mut line = Vec::with_capacity(buffer[i].len());
                for (style, s) in iter {
                    for c in s.chars() {
                        line.push((c, draw::CharStyle::Style(style)));
                    }
                }
                let parens = [('{', '}'), ('(', ')'), ('[', ']')];
                for c in &mut line {
                    for (k, (l, r)) in parens.iter().enumerate() {
                        if c.0 == *l {
                            let fg = Self::RAINBOW[self.parens_level[k] % Self::RAINBOW.len()];
                            c.1 = CharStyle::fg_bg(fg, self.bg);
                            self.parens_level[k] += 1;
                        }
                        if c.0 == *r && self.parens_level[k] > 0 {
                            self.parens_level[k] -= 1;
                            let fg = Self::RAINBOW[self.parens_level[k] % Self::RAINBOW.len()];
                            c.1 = CharStyle::fg_bg(fg, self.bg);
                        }
                    }
                }
                self.cache.push(line);
            }
        }
        &self.cache[i]
    }

    pub fn get_line(&self, i: usize) -> Option<&[(char, CharStyle)]> {
        self.cache.get(i).map(|v| v.as_slice())
    }
}
