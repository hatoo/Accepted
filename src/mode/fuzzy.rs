use super::Mode;
use super::Transition;
use crate::buffer::Buffer;
use crate::draw;
use fuzzy_matcher::clangd::fuzzy_indices;
use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path;
use std::process;
use std::sync::mpsc;
use std::thread;
use termion::event::{Event, Key};

pub struct FuzzyOpen {
    receiver: mpsc::Receiver<String>,
    finds: Vec<String>,
    line_buf: Vec<char>,

    index: usize,
    result: Vec<(String, HashSet<usize>)>,
}

fn fuzzy_match(line: &str, query: &str) -> Option<(i64, HashSet<usize>)> {
    let mut maxi = std::i64::MIN;
    let mut set = HashSet::new();

    for q in query.split_whitespace() {
        if let Some((score, idxs)) = fuzzy_indices(line, q) {
            maxi = std::cmp::max(maxi, score);
            for i in idxs {
                set.insert(i);
            }
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
            let _ = || -> Result<(), failure::Error> {
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
            result: Vec::new(),
        }
    }
}

impl FuzzyOpen {
    fn update(&mut self) {
        if self.line_buf.is_empty() {
            self.result = self
                .finds
                .iter()
                .map(|s| (s.clone(), Default::default()))
                .collect();
        } else {
            let query: String = self.line_buf.iter().collect();

            let mut res = self
                .finds
                .par_iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    fuzzy_match(s.as_str(), query.as_str())
                        .map(|(score, indices)| (score, i, s.clone(), indices))
                })
                .collect::<Vec<_>>();

            res.sort_by_key(|e| (Reverse(e.0), e.1));
            self.result = res.into_iter().map(|e| (e.2, e.3)).collect();
        }
    }
}

impl Mode for FuzzyOpen {
    fn event(&mut self, buf: &mut Buffer, event: termion::event::Event) -> Transition {
        match event {
            Event::Key(Key::Char('\n')) => {
                if let Some((path, _)) = self.result.get(self.index) {
                    buf.open(path::PathBuf::from(path));
                }
                return super::Normal::default().into();
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
                return super::Normal::default().into();
            }
            Event::Key(Key::Up) => {
                if self.result.len() > 0 {
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
    fn draw(&mut self, buf: &mut Buffer, mut view: draw::TermView) -> draw::CursorState {
        let mut pushed = false;
        while let Ok(line) = self.receiver.try_recv() {
            self.finds.push(line);
            pushed = true;
        }
        if pushed {
            self.update();
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
            for (i, (line, matches)) in self.result[..result_view.height()].iter().enumerate().rev()
            {
                for (j, c) in line.chars().enumerate() {
                    let mut style = if matches.contains(&j) {
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
