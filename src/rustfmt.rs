use std::io::Write;
use std::process;

pub fn system_rustfmt(src: &str) -> Option<String> {
    let mut rustfmt = process::Command::new("rustfmt")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdin = rustfmt.stdin.take()?;
    write!(stdin, "{}", src).unwrap();
    let out = rustfmt.wait_with_output().ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = out.stdout;
    let out = String::from_utf8(stdout).ok()?;
    Some(out.replace("\r\n", "\n"))
}
