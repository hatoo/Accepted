use syntect;

pub struct Syntax<'a> {
    pub syntax: &'a syntect::parsing::SyntaxDefinition,
    pub theme: &'a syntect::highlighting::Theme,
}
