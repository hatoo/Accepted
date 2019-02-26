use std::collections::HashMap;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use clap::{crate_authors, crate_version, App, Arg};
use rbtag::BuildInfo;
use serde_derive::{Deserialize, Serialize};
use termion::event::{Event, Key};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;

use accepted::config;
use accepted::draw::DoubleBuffer;
use accepted::{Buffer, BufferMode};

#[derive(BuildInfo)]
struct BuildTag;

#[derive(Serialize, Deserialize, Debug)]
struct SnippetSet(HashMap<String, Snippet>);

#[derive(Serialize, Deserialize, Debug)]
struct Snippet {
    prefix: String,
    body: Vec<String>,
}

fn main() {
    let config_path = dirs::config_dir().map(|mut p| {
        p.push("acc");
        p.push("config.toml");
        p
    });

    let after_help = if let Some(config_path) = config_path.as_ref() {
        format!("Config file will be loaded from {}", config_path.display())
    } else {
        "No config path detected in this system".to_string()
    };

    let matches = App::new("Accepted")
        .author(crate_authors!())
        .version(crate_version!())
        .long_version(format!("v{} {}", crate_version!(), BuildTag.get_build_commit(),).as_str())
        .about("A text editor to be ACCEPTED")
        .after_help(after_help.as_str())
        .bin_name("acc")
        .arg(Arg::with_name("file"))
        .get_matches();

    let file = matches.value_of_os("file");
    let config = config_path
        .and_then(|config_path| fs::read_to_string(&config_path).ok())
        .and_then(|s| {
            let result = config::parse_config_with_default(s.as_str());
            match result {
                Err(err) => {
                    let mut buf = String::new();
                    println!("Failed to load config.toml");
                    println!("Reason: {}", err);
                    println!();
                    println!("Press Enter to continue");
                    std::io::stdin().read_line(&mut buf).unwrap();
                    None
                }
                Ok(config) => Some(config),
            }
        })
        .unwrap_or_default();

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

    let syntax_parent = accepted::syntax::SyntaxParent::default();

    let mut buf = Buffer::new(&syntax_parent, &config);
    if let Some(path) = file {
        buf.open(path);
    }

    let mut state = BufferMode::new(buf);

    let mut draw = DoubleBuffer::default();

    let frame = Duration::from_secs(1) / 60;

    loop {
        let start_frame = Instant::now();
        state.buf.extend_cache_duration(frame);
        let now = Instant::now();

        let evt = if (now - start_frame) > frame {
            rx.try_recv().ok()
        } else {
            rx.recv_timeout(frame - (now - start_frame)).ok()
        };

        if let Some(evt) = evt {
            if evt == Event::Key(Key::Ctrl('l')) {
                draw.redraw();
            }
            if state.event(evt) {
                return;
            }
        }

        state.draw(&mut draw.back);
        draw.present(
            &mut stdout,
            config
                .get::<config::types::keys::ANSIColor>(None)
                .cloned()
                .unwrap_or(false),
        )
        .unwrap();
        stdout.flush().unwrap();
    }
}
