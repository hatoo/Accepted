use syntect;
use syntect::easy::HighlightLines;

pub struct Syntax<'a> {
    pub syntax: &'a syntect::parsing::SyntaxDefinition,
    pub theme: &'a syntect::highlighting::Theme,
}

impl<'a> Syntax<'a> {
    pub fn highlight_lines(&self) -> HighlightLines<'a> {
        HighlightLines::new(self.syntax, self.theme)
    }
}
