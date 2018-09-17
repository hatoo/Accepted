use std::io::{Read, Write};
use std::process;
use std::process::Command;

pub fn clipboard_copy(s: &str) -> bool {
    if let Ok(mut p) = Command::new("pbcopy")
        .stdin(process::Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("win32yank")
                .args(["-i"].iter())
                .stdin(process::Stdio::piped())
                .spawn()
        }).or_else(|_| {
            Command::new("xsel")
                .args(["-bi"].iter())
                .stdin(process::Stdio::piped())
                .spawn()
        }) {
        if let Some(mut stdin) = p.stdin.take() {
            write!(stdin, "{}", s).unwrap();
            return true;
        }
    }
    false
}

pub fn clipboard_paste() -> Option<String> {
    if let Ok(mut p) = Command::new("pbpaste")
        .stdout(process::Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("win32yank")
                .args(["-o"].iter())
                .stdout(process::Stdio::piped())
                .spawn()
        }).or_else(|_| {
            Command::new("xsel")
                .args(["-bo"].iter())
                .stdout(process::Stdio::piped())
                .spawn()
        }) {
        if let Some(mut stdout) = p.stdout.take() {
            let mut buf = String::new();
            stdout.read_to_string(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}
