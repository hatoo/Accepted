extern crate acc;
extern crate termion;

use std::io::{stdin, stdout, Write};
use termion::clear::All;
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use acc::Window;

fn main() {
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

    let mut state = Window::new();

    loop {
        while let Ok(evt) = rx.recv_timeout(Duration::from_millis(16)) {
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
        }

        state.draw(&mut stdout);
        stdout.flush().unwrap();
    }
}
