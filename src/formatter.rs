use std::io::Write;
use std::process;

pub fn system_format(mut command: process::Command, src: &str) -> Option<String> {
    let mut command = command
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
        .ok()?;
    {
        let mut stdin = command.stdin.take()?;
        write!(stdin, "{}", src).unwrap();
    }
    let out = command.wait_with_output().ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = out.stdout;
    let out = String::from_utf8(stdout).ok()?;
    Some(out.replace("\r\n", "\n"))
}
