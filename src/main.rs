extern crate acc;
#[macro_use]
extern crate clap;
extern crate syntect;
extern crate termion;

use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use std::io::{stdin, stdout, Cursor, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use acc::draw::DoubleBuffer;
use acc::theme;
use acc::{Buffer, BufferMode};

use clap::{App, Arg};

fn main() {
    let matches = App::new("Accepted")
        .version(crate_version!())
        .bin_name("acc")
        .arg(Arg::with_name("file"))
        .get_matches();

    let file = matches.value_of_os("file");

    let stdin = stdin();
    let mut stdout = MouseTerminal::from(AlternateScreen::from(stdout()).into_raw_mode().unwrap());
    // let mut stdout = MouseTerminal::from(stdout().into_raw_mode().unwrap());

    let (tx, rx) = channel();

    thread::spawn(move || {
        for c in stdin.events() {
            if let Ok(evt) = c {
                tx.send(evt).unwrap();
            }
        }
    });

    let ps = SyntaxSet::load_defaults_nonewlines();
    // let theme = ThemeSet::load_from_reader(&mut Cursor::new(theme::ONE_DARK.as_bytes())).unwrap();
    let ts = ThemeSet::load_defaults();

    let syntax = acc::syntax::Syntax {
        syntax: ps.find_syntax_by_extension("rs").unwrap(),
        theme: &ts.themes["Solarized (dark)"],
    };

    let mut buf = Buffer::new(syntax);
    if let Some(path) = file {
        buf.open(path);
    }

    let mut state = BufferMode::new(buf);

    let mut draw = DoubleBuffer::new();

    loop {
        if let Ok(evt) = rx.recv_timeout(Duration::from_millis(16)) {
            if state.event(evt) {
                return;
            }
        }

        state.draw(&mut draw.back);
        draw.present(&mut stdout);
        stdout.flush().unwrap();
    }
}
