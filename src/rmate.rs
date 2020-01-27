use crate::core::Core;
use crate::storage::Storage;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

struct Rmate {
    display_name: String,
    real_path: String,
    token: String,
    data: String,
}

pub struct RmateSave {
    rmate: Rmate,
    sender: mpsc::Sender<(String, String)>,
}

pub struct RmateStorage {
    path: PathBuf,
    rmate_save: RmateSave,
}

impl From<RmateSave> for RmateStorage {
    fn from(rmate_save: RmateSave) -> Self {
        let path = PathBuf::from(rmate_save.rmate.display_name.clone());
        Self { path, rmate_save }
    }
}

impl Storage for RmateStorage {
    fn load(&mut self) -> Core {
        let mut core = Core::default();
        core.set_string(self.rmate_save.rmate.data.clone(), true);
        core
    }
    fn save(&mut self, core: &Core) -> bool {
        let data = core.get_string();
        self.rmate_save
            .sender
            .send((self.rmate_save.rmate.token.clone(), data))
            .is_ok()
    }
    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

pub fn start_server(sender: mpsc::Sender<RmateSave>) -> Result<(), failure::Error> {
    let listener = TcpListener::bind("127.0.0.1:52698")?;

    for stream in listener.incoming() {
        let _ = || -> Result<(), failure::Error> {
            let stream_reader = stream?;
            let mut stream = stream_reader.try_clone()?;
            writeln!(stream, "Accepted")?;
            let (save_tx, save_rx) = mpsc::channel();

            let sender_clone = sender.clone();
            thread::spawn(move || {
                let _ = reader_thread(stream_reader, save_tx, sender_clone);
            });

            thread::spawn(move || {
                let _ = write_thread(stream, save_rx);
            });
            Ok(())
        }();
    }

    Ok(())
}

fn write_thread(
    mut stream: TcpStream,
    save_rx: mpsc::Receiver<(String, String)>,
) -> Result<(), failure::Error> {
    for (token, data) in save_rx {
        writeln!(stream, "save")?;
        writeln!(stream, "token: {}", token)?;
        writeln!(stream, "data: {}", data.len())?;
        writeln!(stream, "{}", data)?;
    }

    Ok(())
}

fn reader_thread(
    stream: TcpStream,
    save_tx: mpsc::Sender<(String, String)>,
    sender: mpsc::Sender<RmateSave>,
) -> Option<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;

        assert!(line.trim_end() == "open");

        let mut hash = HashMap::new();

        while {
            line.clear();
            reader.read_line(&mut line).ok()?;
            line != ".\n"
        } {
            // dbg!(&line);
            let mut iter = line.split(": ");
            let header: &str = iter.next()?;
            let content: &str = iter.next()?.trim_end();

            if header == "data" {
                let length: usize = content.parse().ok()?;
                let mut buf: Vec<u8> = vec![0; length];

                reader.read_exact(&mut buf).ok()?;

                let data = String::from_utf8(buf).ok()?;
                hash.insert(header.to_string(), data);
                line.clear();

                let mut tail = vec![0; 1];
                reader.read_exact(&mut tail).ok()?;
            } else {
                hash.insert(header.to_string(), content.to_string());
            }
        }

        let rmate = Rmate {
            display_name: hash.get("display-name").cloned().unwrap_or_default(),
            real_path: hash.get("real-path").cloned().unwrap_or_default(),
            token: hash.get("token")?.to_string(),
            data: hash.get("data").cloned().unwrap_or_default(),
        };

        sender
            .send(RmateSave {
                rmate,
                sender: save_tx.clone(),
            })
            .ok()?;
    }
}
