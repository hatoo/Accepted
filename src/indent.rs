use crate::parenthesis;

pub fn next_indent_level(line: &str, indent_width: usize) -> usize {
    let base = line
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .map(|c| if c == ' ' { 1 } else { indent_width })
        .sum::<usize>()
        / indent_width;
    if parenthesis::PARENTHESIS_LEFTS
        .iter()
        .any(|&c| line.ends_with(c))
    {
        base + 1
    } else {
        base
    }
}
