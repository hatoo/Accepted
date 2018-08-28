extern crate acc;
#[macro_use]
extern crate clap;
extern crate termion;

use termion::clear::All;
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;

use std::io::{stdin, stdout, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use acc::BufferMode;

use clap::{App, Arg};

fn main() {
    let matches = App::new("Accepted")
        .version(crate_version!())
        .bin_name("acc")
        .arg(Arg::with_name("file"))
        .get_matches();

    let file = matches.value_of_os("file");

    let stdin = stdin();
    let mut stdout = MouseTerminal::from(stdout().into_raw_mode().unwrap());

    let (tx, rx) = channel();

    thread::spawn(move || {
        for c in stdin.events() {
            if let Ok(evt) = c {
                tx.send(evt).unwrap();
            }
        }
    });

    let mut state = if let Some(path) = file {
        BufferMode::open(path).unwrap()
    } else {
        BufferMode::new()
    };

    loop {
        while let Ok(evt) = rx.recv_timeout(Duration::from_millis(16)) {
            /*
            match evt {
                Event::Key(Key::Ctrl('q')) => {
                    return;
                }
                Event::Key(Key::Backspace) => {
                    state.backspase();
                }
                Event::Key(Key::Char(c)) => {
                    state.insert(c);
                }
                Event::Key(Key::Left) => {
                    state.cursor_left();
                }
                Event::Key(Key::Right) => {
                    state.cursor_right();
                }
                Event::Key(Key::Up) => {
                    state.cursor_up();
                }
                Event::Key(Key::Down) => {
                    state.cursor_down();
                }
                _ => {}
            }
            */
            if state.event(evt) {
                return;
            }
        }

        let buf = state.draw();
        stdout.write(&buf).unwrap();
        stdout.flush().unwrap();
    }
}
