pub fn next_indent_level(line: &[char], indent_width: usize) -> usize {
    let base = line.iter().take_while(|&&c| c == ' ').count() / indent_width;
    if ['{', '[', '('].iter().any(|c| line.last() == Some(c)) {
        base + 1
    } else {
        base
    }
}
