use syntect;
use syntect::easy::HighlightLines;

pub struct Syntax<'a> {
    pub syntax_set: &'a syntect::parsing::SyntaxSet,
    pub syntax: &'a syntect::parsing::SyntaxReference,
    pub theme: &'a syntect::highlighting::Theme,
}

impl<'a> Syntax<'a> {
    pub fn highlight_lines(&self) -> HighlightLines<'a> {
        HighlightLines::new(self.syntax, self.theme)
    }
}
