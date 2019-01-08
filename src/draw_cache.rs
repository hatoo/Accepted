use draw;
use draw::CharStyle;
use std::cmp::min;
use std::collections::HashMap;
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

    fn highlight(
        &mut self,
        line: &str,
        syntax_set: &SyntaxSet,
        highlighter: &Highlighter,
        bg: Color,
    ) -> Vec<(char, CharStyle)> {
        self.ops = self.parse_state.parse_line(line, syntax_set);

        let highlight_state = &mut self.highlight_state;
        let parens_level = &mut self.parens_level;
        let ops = &self.ops[..];

        let iter: HighlightIterator =
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
        .collect()
    }

    fn next(&mut self, line: &str, syntax_set: &SyntaxSet, highlighter: &Highlighter) {
        self.ops = self.parse_state.parse_line(line, syntax_set);

        let highlight_state = &mut self.highlight_state;
        let parens_level = &mut self.parens_level;
        let ops = &self.ops[..];

        let iter: HighlightIterator =
            HighlightIterator::new(highlight_state, ops, line, highlighter);

        let parens = [('{', '}'), ('(', ')'), ('[', ']')];
        for (_style, s) in iter {
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
    syntax: &'a syntect::parsing::SyntaxReference,
    syntax_set: &'a syntect::parsing::SyntaxSet,
    highlighter: Highlighter<'a>,
    bg: Color,
    state_cache: Vec<DrawState>,
    draw_cache: HashMap<usize, Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    const CACHE_WIDTH: usize = 100;

    pub fn new(syntax: &syntax::Syntax<'a>) -> Self {
        let highlighter = Highlighter::new(syntax.theme);
        let bg = syntax.theme.settings.background.unwrap();
        Self {
            syntax: syntax.syntax,
            syntax_set: syntax.syntax_set,
            highlighter,
            state_cache: Vec::new(),
            draw_cache: HashMap::new(),
            bg,
        }
    }

    fn near_state(&mut self, buffer: &[Vec<char>], i: usize) -> DrawState {
        if i / Self::CACHE_WIDTH == 0 {
            return DrawState::new(self.syntax, &self.highlighter);
        }

        while self.state_cache.len() < i / Self::CACHE_WIDTH {
            let mut state = self
                .state_cache
                .last()
                .cloned()
                .unwrap_or_else(|| DrawState::new(self.syntax, &self.highlighter));

            for i in self.state_cache.len() * Self::CACHE_WIDTH
                ..self.state_cache.len() * Self::CACHE_WIDTH + Self::CACHE_WIDTH
            {
                state.next(
                    &buffer[i].iter().collect::<String>(),
                    self.syntax_set,
                    &self.highlighter,
                );
            }
            self.state_cache.push(state);
        }

        self.state_cache[i / Self::CACHE_WIDTH - 1].clone()
    }

    pub fn cache_line(&mut self, buffer: &[Vec<char>], i: usize) {
        if !self.draw_cache.contains_key(&i) {
            let mut state = self.near_state(buffer, i);
            for i in i - (i % Self::CACHE_WIDTH)
                ..min(
                    buffer.len(),
                    i - (i % Self::CACHE_WIDTH) + Self::CACHE_WIDTH,
                )
            {
                let draw = state.highlight(
                    &buffer[i].iter().collect::<String>(),
                    self.syntax_set,
                    &self.highlighter,
                    self.bg,
                );

                self.draw_cache.insert(i, draw);
            }
        }
    }

    pub fn get_line(&self, i: usize) -> Option<&[(char, CharStyle)]> {
        self.draw_cache.get(&i).map(|v| v.as_slice())
    }

    pub fn set_dirty_from(&mut self, dirty_from: usize) {
        self.draw_cache.retain(|&k, _| k >= dirty_from);
        self.state_cache.drain(dirty_from / Self::CACHE_WIDTH..);
    }
}
