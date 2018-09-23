pub fn next_indent_level(line: &[char]) -> usize {
    let base = line.iter().take_while(|&&c| c == ' ').count() / 4;
    if ['{', '[', '('].into_iter().any(|c| line.last() == Some(c)) {
        base + 1
    } else {
        base
    }
}
