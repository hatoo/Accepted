use super::Mode;
use super::Transition;
use crate::buffer::Buffer;
use crate::core::CoreBuffer;
use crate::draw;
use fuzzy_matcher::clangd::fuzzy_indices;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashSet};
use std::io::{BufRead, BufReader};
use std::path;
use std::process;
use std::sync::mpsc;
use std::thread;
use termion::event::{Event, Key};

#[derive(Eq)]
struct MatchedItem {
    score: i64,
    index: usize,
    line: String,
    match_indices: HashSet<usize>,
}

impl MatchedItem {
    fn cmp_key(&self) -> (Reverse<i64>, usize) {
        (Reverse(self.score), self.index)
    }
}

impl PartialEq for MatchedItem {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_key().eq(&other.cmp_key())
    }
}

impl PartialOrd for MatchedItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cmp_key().partial_cmp(&other.cmp_key())
    }
}

impl Ord for MatchedItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_key().cmp(&other.cmp_key())
    }
}

pub struct FuzzyOpen {
    receiver: mpsc::Receiver<String>,
    finds: Vec<String>,
    line_buf: Vec<char>,

    index: usize,
    result: BTreeSet<MatchedItem>,
}

fn fuzzy_match(line: &str, query: &str) -> Option<(i64, HashSet<usize>)> {
    let mut maxi = std::i64::MIN;
    let mut set = HashSet::new();

    for q in query.split_whitespace() {
        if let Some((score, idxs)) = fuzzy_indices(line, q) {
            maxi = std::cmp::max(maxi, score);
            set.extend(idxs.into_iter());
        } else {
            return None;
        }
    }

    Some((maxi, set))
}

impl Default for FuzzyOpen {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = || -> anyhow::Result<()> {
                let mut child = process::Command::new("find")
                    .arg(".")
                    .stdout(process::Stdio::piped())
                    .stderr(process::Stdio::piped())
                    .spawn()?;
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            if tx.send(line.trim_end().to_string()).is_err() {
                                child.kill()?;
                                break;
                            }
                        }
                    }
                }
                Ok(())
            }();
        });

        Self {
            receiver: rx,
            finds: Vec::new(),
            line_buf: Vec::new(),

            index: 0,
            result: Default::default(),
        }
    }
}

impl FuzzyOpen {
    fn update(&mut self) {
        if self.line_buf.is_empty() {
            self.result = self
                .finds
                .iter()
                .enumerate()
                .map(|(i, s)| MatchedItem {
                    score: 0,
                    index: i,
                    line: s.clone(),
                    match_indices: Default::default(),
                })
                .collect();
        } else {
            let query: String = self.line_buf.iter().collect();

            self.result = self
                .finds
                .par_iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    fuzzy_match(s.as_str(), query.as_str()).map(|(score, indices)| MatchedItem {
                        score,
                        index: i,
                        line: s.clone(),
                        match_indices: indices,
                    })
                })
                .collect();
        }
    }

    fn push_line(&mut self, line: String) {
        let query: String = self.line_buf.iter().collect();
        let index = self.finds.len();
        if let Some((score, matches)) = fuzzy_match(&line, query.as_str()) {
            self.result.insert(MatchedItem {
                score,
                index,
                match_indices: matches,
                line: line.clone(),
            });
        }
        self.finds.push(line);
    }
}

impl<B: CoreBuffer> Mode<B> for FuzzyOpen {
    fn event(&mut self, buf: &mut Buffer<B>, event: termion::event::Event) -> Transition<B> {
        match event {
            Event::Key(Key::Char('\n')) => {
                if let Some(item) = self.result.iter().nth(self.index) {
                    buf.open(path::PathBuf::from(&item.line));
                }
                return super::Normal::default().into_transition();
            }
            Event::Key(Key::Char(c)) if !c.is_control() => {
                self.line_buf.push(c);
                self.update();
            }
            Event::Key(Key::Backspace) => {
                if self.line_buf.pop().is_some() {
                    self.update();
                }
            }
            Event::Key(Key::Esc) => {
                return super::Normal::default().into_transition();
            }
            Event::Key(Key::Up) => {
                if !self.result.is_empty() {
                    self.index = std::cmp::min(self.index + 1, self.result.len() - 1);
                }
            }
            Event::Key(Key::Down) => {
                if self.index > 0 {
                    self.index -= 1;
                }
            }
            _ => {}
        }
        Transition::Nothing
    }
    fn draw(&mut self, buf: &mut Buffer<B>, mut view: draw::TermView) -> draw::CursorState {
        while let Ok(line) = self.receiver.try_recv() {
            self.push_line(line);
        }

        let height = view.height();
        {
            let mut sub = view.view((0, 0), height - 1, view.width());
            let buf_view_len = if sub.height() > self.result.len() {
                sub.height() - self.result.len()
            } else {
                0
            };

            if buf_view_len > 0 {
                let view_buf = sub.view((0, 0), buf_view_len, sub.width());
                buf.draw(view_buf);
            }

            let mut result_view =
                sub.view((buf_view_len, 0), sub.height() - buf_view_len, sub.width());
            for (i, item) in self
                .result
                .iter()
                .take(result_view.height())
                .enumerate()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                for (j, c) in item.line.chars().enumerate() {
                    let mut style = if item.match_indices.contains(&j) {
                        draw::styles::HIGHLIGHT
                    } else {
                        draw::styles::DEFAULT
                    };

                    if i == self.index {
                        style.bg = draw::Color {
                            r: 0x44,
                            g: 0x44,
                            b: 0x44,
                        };
                    }

                    result_view.put_inline(c, style, None);
                }
                result_view.newline();
            }
        }
        let mut query_view = view.view((view.height() - 1, 0), 1, view.width());
        query_view.puts(
            &format!("Fuzzy> {}", self.line_buf.iter().collect::<String>()),
            draw::styles::DEFAULT,
        );

        if query_view.is_out() {
            draw::CursorState::Hide
        } else {
            draw::CursorState::Show(query_view.cursor, draw::CursorShape::Bar)
        }
    }
}
