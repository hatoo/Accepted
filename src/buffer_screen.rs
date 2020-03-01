use std::borrow::Cow;
use crate::buffer::{Buffer, ShowCursor};
use crate::draw_cache::DrawCache;
use crate::draw;
use crate::draw::{styles, CharStyle, LinenumView, TermView};
use crate::core::Cursor;
use std::ops::RangeInclusive;
use std::ops::RangeBounds;
use crate::core::CoreBuffer;

pub struct BufferScreen<'a, B: CoreBuffer> {
    buffer: Buffer<'a, B>,
    cache: DrawCache<'a>,
}

impl<'a, B: CoreBuffer> BufferScreen<'a, B> {
    pub fn extend_cache_duration(&mut self, duration: std::time::Duration) {
        let highlighter = syntect::highlighting::Highlighter::new(&self.syntax.theme);
        self.cache
            .extend_cache_duration(self.buffer.core.core_buffer(), duration, &highlighter);
    }

    pub fn draw(&mut self, view: TermView) -> Option<Cursor> {
        self.buffer.poll_compile_message();
        self.draw_with_selected::<RangeInclusive<Cursor>>(view, None)
    }

    /*
    pub fn draw_with_selected<R: RangeBounds<Cursor>>(
        &mut self,
        mut view: TermView,
        selected: Option<R>,
    ) -> Option<Cursor> {
        match self.show_cursor_on_draw {
            ShowCursor::ShowMiddle => {
                self.show_cursor_middle_(view.height());
            }
            ShowCursor::Show => {
                self.show_cursor_(view.height(), view.width());
            }
            ShowCursor::None => {}
        }
        let highlighter = syntect::highlighting::Highlighter::new(&self.syntax.theme);
        self.show_cursor_on_draw = ShowCursor::None;
        view.bg = self.syntax.theme.settings.background.map(Into::into);
        let v = Vec::new();
        let compiler_outputs = self
            .last_compiler_result
            .as_ref()
            .map(|res| &res.messages)
            .unwrap_or_else(|| &v);
        let mut view = LinenumView::new(
            self.row_offset,
            self.core.core_buffer().len_lines(),
            &compiler_outputs,
            view,
        );
        let mut cursor = None;
        let tab_size = self.indent_width();

        if self.buffer_update != self.core.buffer_changed() {
            self.buffer_update = self.core.buffer_changed();
            self.cache.dirty_from(self.core.dirty_from);
        }

        'outer: for i in self.row_offset..self.core.core_buffer().len_lines() {
            self.cache
                .cache_line(self.core.core_buffer(), i, &highlighter);
            let line_ref = self.cache.get_line(i).unwrap();
            let mut line = Cow::Borrowed(line_ref);

            self.core.dirty_from = i;

            if !self.search.is_empty() && line.len() >= self.search.len() {
                for j in 0..=line.len() - self.search.len() {
                    let m = self
                        .search
                        .iter()
                        .zip(line[j..j + self.search.len()].iter())
                        .all(|(c1, (c2, _))| c1 == c2);
                    if m {
                        for k in j..j + self.search.len() {
                            line.to_mut()[k].1 = draw::styles::HIGHLIGHT;
                        }
                    }
                }
            }

            for (j, &c) in line.iter().enumerate() {
                let (c, mut style) = c;
                let t = Cursor { row: i, col: j };

                if self.is_annotate(t) {
                    style.modification = draw::CharModification::UnderLine;
                }

                let style = if selected.as_ref().map(|r| r.contains(&t)) == Some(true) {
                    styles::SELECTED
                } else {
                    style
                };

                if c == '\t' {
                    if self.core.cursor() == t {
                        cursor = view.put(' ', style, Some(t));
                    } else if view.put(' ', style, Some(t)).is_none() {
                        break 'outer;
                    }
                    for _ in 1..tab_size {
                        if view.cause_newline(' ') {
                            break;
                        } else {
                            if view.put(' ', style, Some(t)).is_none() {
                                break 'outer;
                            }
                        }
                    }
                } else {
                    if self.core.cursor() == t {
                        cursor = view.put(c, style, Some(t));
                    } else if view.put(c, style, Some(t)).is_none() {
                        break 'outer;
                    }
                }
            }
            let t = Cursor {
                row: i,
                col: self.core.core_buffer().len_line(i),
            };

            if self.core.cursor() == t {
                cursor = view.cursor();
            }

            if self.core.core_buffer().len_line(i) == 0 {
                if let Some(col) = self.syntax.theme.settings.background {
                    view.put(' ', CharStyle::bg(col.into()), Some(t));
                } else {
                    view.put(' ', styles::DEFAULT, Some(t));
                }
            }

            if i != self.core.core_buffer().len_lines() - 1 {
                if let Some(col) = self.syntax.theme.settings.background {
                    while !view.cause_newline(' ') {
                        view.put(' ', CharStyle::bg(col.into()), Some(t));
                    }
                } else {
                    while !view.cause_newline(' ') {
                        view.put(' ', styles::DEFAULT, Some(t));
                    }
                }
                view.newline();
            }
        }

        cursor
    }
    */
}