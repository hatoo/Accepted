use std::borrow::Cow;
use std::cmp::min;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use ropey::Rope;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::SyntaxSet;
use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp};

use crate::core::{CoreBuffer, Cursor};
use crate::draw::CharStyle;
use crate::draw::Color;
use crate::parenthesis;
use crate::ropey_util::RopeExt;
use crate::syntax;

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
        },
        Color {
            r: 0xf3,
            g: 0x98,
            b: 0x00,
        },
        Color {
            r: 0xff,
            g: 0xf1,
            b: 0x00,
        },
        Color {
            r: 0x8f,
            g: 0xc3,
            b: 0x1f,
        },
        Color {
            r: 0x00,
            g: 0x99,
            b: 0x44,
        },
        Color {
            r: 0x00,
            g: 0xa0,
            b: 0xe9,
        },
        Color {
            r: 0x00,
            g: 0x68,
            b: 0xb7,
        },
        Color {
            r: 0x92,
            g: 0x07,
            b: 0x83,
        },
        Color {
            r: 0xe4,
            g: 0x00,
            b: 0x7f,
        },
        Color {
            r: 0xe5,
            g: 0x00,
            b: 0x4f,
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
                    for (k, (l, r)) in parenthesis::PARENTHESIS_PAIRS.iter().enumerate() {
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
                    (c, style.into())
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

        for (_style, s) in iter {
            for c in s.chars() {
                for (k, (l, r)) in parenthesis::PARENTHESIS_PAIRS.iter().enumerate() {
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
    draw_cache_pseudo: HashMap<usize, Vec<(char, CharStyle)>>,
}

impl<'a> DrawCache<'a> {
    const CACHE_WIDTH: usize = 100;

    pub fn new(syntax: &syntax::Syntax<'a>) -> Self {
        let highlighter = Highlighter::new(syntax.theme);
        let bg = syntax.theme.settings.background.unwrap().into();
        Self {
            syntax: syntax.syntax,
            syntax_set: syntax.syntax_set,
            highlighter,
            state_cache: Vec::new(),
            draw_cache: HashMap::new(),
            draw_cache_pseudo: HashMap::new(),
            bg,
        }
    }

    fn start_state(&self) -> DrawState {
        DrawState::new(self.syntax, &self.highlighter)
    }

    pub fn extend_cache_duration<B: CoreBuffer>(&mut self, buffer: &B, duration: Duration) {
        let start = Instant::now();
        while self.state_cache.len() < buffer.len_lines() / Self::CACHE_WIDTH {
            let mut state = self
                .state_cache
                .last()
                .cloned()
                .unwrap_or_else(|| self.start_state());

            for line in self.state_cache.len() * Self::CACHE_WIDTH
                ..(self.state_cache.len() + 1) * Self::CACHE_WIDTH
            {
                // TODO use COW
                state.next(
                    buffer
                        .get_range(
                            Cursor { row: line, col: 0 }..Cursor {
                                row: line,
                                col: buffer.len_line(line),
                            },
                        )
                        .as_str(),
                    self.syntax_set,
                    &self.highlighter,
                );
            }

            self.state_cache.push(state);

            if Instant::now() - start >= duration {
                return;
            }
        }
    }

    fn near_state(&mut self, i: usize) -> Option<DrawState> {
        if i / Self::CACHE_WIDTH == 0 {
            return Some(self.start_state());
        }

        self.state_cache.get(i / Self::CACHE_WIDTH - 1).cloned()
    }

    pub fn cache_line(&mut self, buffer: &Rope, i: usize) {
        if !self.draw_cache.contains_key(&i) {
            if let Some(mut state) = self.near_state(i) {
                for i in i - (i % Self::CACHE_WIDTH)
                    ..min(
                        buffer.len_lines(),
                        i - (i % Self::CACHE_WIDTH) + Self::CACHE_WIDTH,
                    )
                {
                    let draw = state.highlight(
                        &Cow::from(buffer.l(i)),
                        self.syntax_set,
                        &self.highlighter,
                        self.bg,
                    );

                    self.draw_cache.insert(i, draw);
                }
            }
        }

        if !self.draw_cache.contains_key(&i) && !self.draw_cache_pseudo.contains_key(&i) {
            let mut state = self.start_state();
            for i in i - (i % Self::CACHE_WIDTH)
                ..min(
                    buffer.len_lines(),
                    i - (i % Self::CACHE_WIDTH) + Self::CACHE_WIDTH,
                )
            {
                let draw = state.highlight(
                    &Cow::from(buffer.l(i)),
                    self.syntax_set,
                    &self.highlighter,
                    self.bg,
                );

                self.draw_cache_pseudo.insert(i, draw);
            }
        }
    }

    pub fn get_line(&self, i: usize) -> Option<&[(char, CharStyle)]> {
        self.draw_cache
            .get(&i)
            .map(Vec::as_slice)
            .or_else(|| self.draw_cache_pseudo.get(&i).map(Vec::as_slice))
    }

    pub fn dirty_from(&mut self, dirty_from: usize) {
        self.draw_cache.clear();
        self.draw_cache_pseudo.clear();
        if dirty_from / Self::CACHE_WIDTH < self.state_cache.len() {
            self.state_cache.drain(dirty_from / Self::CACHE_WIDTH..);
        }
    }
}
