extern crate accepted;
#[macro_use]
extern crate clap;
extern crate config;
extern crate dirs;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate racer;
extern crate shellexpand;
extern crate syntect;
extern crate termion;

use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use accepted::draw::DoubleBuffer;
use accepted::{Buffer, BufferMode};

use clap::{App, Arg};

#[derive(Serialize, Deserialize, Debug)]
struct SnippetSet(HashMap<String, Snippet>);
#[derive(Serialize, Deserialize, Debug)]
struct Snippet {
    prefix: String,
    body: Vec<String>,
}

fn main() {
    let matches = App::new("Accepted")
        .author(crate_authors!())
        .version(crate_version!())
        .about("A text editor to be ACCEPTED")
        .bin_name("acc")
        .arg(Arg::with_name("file"))
        .get_matches();

    let file = matches.value_of_os("file");
    let config = dirs::config_dir()
        .map(|mut p| {
            p.push("acc");
            p.push("init.toml");
            p
        }).map(|config_path| {
            let mut settings = config::Config::default();
            // Just ignore error.
            let _ = settings.merge(config::File::from(config_path));
            settings
        }).unwrap_or(config::Config::default());

    let mut snippet = BTreeMap::new();
    if let Ok(arr) = config.get_array("snippet") {
        for fname in arr {
            if let Ok(s) = fname.into_str() {
                if let Ok(snippet_json) =
                    fs::read_to_string(PathBuf::from(shellexpand::tilde(&s).as_ref()))
                {
                    if let Ok(snippet_set) = serde_json::from_str::<SnippetSet>(&snippet_json) {
                        for (_, s) in snippet_set.0 {
                            let mut body = String::new();
                            for line in &s.body {
                                for c in line.chars() {
                                    body.push(c);
                                }
                                body.push('\n');
                            }
                            snippet.insert(s.prefix, body);
                        }
                    }
                }
            }
        }
    }

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

    // TODO Make configurable
    let syntax = accepted::syntax::Syntax {
        syntax: ps.find_syntax_by_extension("rs").unwrap(),
        theme: &ts.themes["Solarized (dark)"],
    };

    let cache = racer::FileCache::default();
    let mut buf = Buffer::new(syntax, &cache);
    if let Some(path) = file {
        buf.open(path);
    }

    buf.snippet = snippet;

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
