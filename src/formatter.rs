use std::io::Write;
use std::process;

pub fn system_rustfmt(src: &str) -> Option<String> {
    let mut rustfmt = process::Command::new("rustfmt")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
        .ok()?;
    {
        let mut stdin = rustfmt.stdin.take()?;
        write!(stdin, "{}", src).unwrap();
    }
    let out = rustfmt.wait_with_output().ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = out.stdout;
    let out = String::from_utf8(stdout).ok()?;
    Some(out.replace("\r\n", "\n"))
}

pub fn system_clang_format(src: &str) -> Option<String> {
    let mut clang_format = process::Command::new("clang-format")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
        .ok()?;
    {
        let mut stdin = clang_format.stdin.take()?;
        write!(stdin, "{}", src).unwrap();
    }
    let out = clang_format.wait_with_output().ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = out.stdout;
    let out = String::from_utf8(stdout).ok()?;
    Some(out.replace("\r\n", "\n"))
}
