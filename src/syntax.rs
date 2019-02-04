use syntect;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Syntax<'a> {
    pub syntax_set: &'a syntect::parsing::SyntaxSet,
    pub syntax: &'a syntect::parsing::SyntaxReference,
    pub theme: &'a syntect::highlighting::Theme,
}

pub struct SyntaxParent {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl Default for SyntaxParent {
    fn default() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_nonewlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

impl SyntaxParent {
    pub fn load_syntax(&self, extension: &str) -> Option<Syntax> {
        let syntax = self.syntax_set.find_syntax_by_extension(extension)?;
        // let theme = ThemeSet::load_from_reader(&mut Cursor::new(theme::ONE_DARK.as_bytes())).unwrap();
        Some(Syntax {
            syntax_set: &self.syntax_set,
            syntax,
            theme: &self.theme_set.themes["Solarized (dark)"],
        })
    }

    pub fn load_syntax_or_txt(&self, extension: &str) -> Syntax {
        self.load_syntax(extension)
            .unwrap_or_else(|| self.load_syntax("txt").unwrap())
    }
}

impl<'a> Syntax<'a> {
    pub fn highlight_lines(&self) -> HighlightLines<'a> {
        HighlightLines::new(self.syntax, self.theme)
    }
}
