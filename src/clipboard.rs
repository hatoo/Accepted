use anyhow::Context;
use std::io::{Read, Write};
use std::process;
use std::process::Command;

pub fn clipboard_copy(s: &str) -> anyhow::Result<()> {
    let mut p = Command::new("pbcopy")
        .stdin(process::Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("win32yank")
                .arg("-i")
                .stdin(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("win32yank.exe")
                .arg("-i")
                .stdin(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("xsel")
                .arg("-bi")
                .stdin(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("xclip")
                .arg("-i")
                .stdin(process::Stdio::piped())
                .spawn()
        })?;
    {
        let mut stdin = p.stdin.take().context("take stdin")?;

        write!(stdin, "{}", s)?;
    }
    p.wait()?;
    Ok(())
}

pub fn clipboard_paste() -> anyhow::Result<String> {
    let p = Command::new("pbpaste")
        .stdout(process::Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("win32yank")
                .arg("-o")
                .stdout(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("win32yank.exe")
                .arg("-o")
                .stdout(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("xsel")
                .arg("-bo")
                .stdout(process::Stdio::piped())
                .spawn()
        })
        .or_else(|_| {
            Command::new("xclip")
                .arg("-o")
                .stdout(process::Stdio::piped())
                .spawn()
        })?;
    let mut stdout = p.stdout.context("take stdout")?;
    let mut buf = String::new();
    stdout.read_to_string(&mut buf)?;
    // win32yank.exe emits CRLF but I don't want to.
    buf = buf.replace('\r', "");
    Ok(buf)
}
