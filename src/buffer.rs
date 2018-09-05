use core::Cursor;
use draw::{CharStyle, View};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use Core;

pub struct Buffer {
    pub path: Option<PathBuf>,
    pub core: Core,
    pub search: Vec<char>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            core: Core::new(),
            search: Vec::new(),
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        fs::read_to_string(path.as_ref()).map(|s| {
            let mut core = Core::new();
            core.buffer = s
                .lines()
                .map(|l| l.trim_right().chars().collect())
                .collect();

            if core.buffer.is_empty() {
                core.buffer = vec![Vec::new()];
            }

            Self {
                path: Some(path.as_ref().to_path_buf()),
                core,
                search: Vec::new(),
            }
        })
    }

    pub fn draw(&self, mut view: View) -> Option<Cursor> {
        let mut cursor = None;

        'outer: for i in self.core.row_offset..self.core.buffer.len() {
            let line: Vec<(char, CharStyle)> = self.core.buffer[i]
                .iter()
                .cloned()
                .zip(
                    search(self.search.as_slice(), &self.core.buffer[i])
                        .into_iter()
                        .map(|b| {
                            if b {
                                CharStyle::Highlight
                            } else {
                                CharStyle::Default
                            }
                        }),
                ).collect();

            for j in 0..self.core.buffer[i].len() {
                let c = line[j];
                let t = Cursor { row: i, col: j };

                if self.core.cursor == t {
                    cursor = view.put(c.0, c.1);
                } else {
                    if view.put(c.0, c.1).is_none() {
                        break 'outer;
                    }
                }

                let t = Cursor {
                    row: i,
                    col: self.core.buffer[i].len(),
                };

                if self.core.cursor == t {
                    cursor = Some(view.cursor);
                }
            }
            let t = Cursor {
                row: i,
                col: self.core.buffer[i].len(),
            };

            if self.core.cursor == t {
                cursor = Some(view.cursor);
            }
            view.newline();
        }

        cursor
    }
}

fn search(seq: &[char], line: &[char]) -> Vec<bool> {
    let mut res = vec![false; line.len()];
    if seq.is_empty() || line.is_empty() {
        return res;
    }
    let mut i = 0;
    while i + seq.len() <= line.len() {
        if &line[i..i + seq.len()] == seq {
            for _ in 0..seq.len() {
                res[i] = true;
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    res
}
