use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

struct Rmate {
    display_name: String,
    token: String,
    data: String,
}

struct RmateSave {
    rmate: Rmate,
    sender: mpsc::Sender<(String, String)>,
}

fn start_server(sender: mpsc::Sender<RmateSave>) -> Result<(), failure::Error> {
    let listener = TcpListener::bind("127.0.0.1:52698")?;

    for stream in listener.incoming() {
        let stream = stream?;
        let stream_reader = stream.try_clone()?;
        let (rmate_tx, rmate_rx) = mpsc::channel();
        let (save_tx, save_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = reader_thread(stream_reader, save_tx, rmate_tx);
        });

        thread::spawn(move || {
            let _ = write_thread(stream, save_rx);
        });
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

        assert!(line == "open");

        let mut hash = HashMap::new();

        while {
            line.clear();
            reader.read_line(&mut line).ok()?;
            line.trim_end() != "."
        } {
            let mut iter = line.split(": ");
            let header: &str = iter.next()?;
            let content: &str = iter.next()?;

            if header == "data" {
                let length: usize = content.parse().ok()?;
                let mut buf: Vec<u8> = vec![0; length];

                reader.read_exact(&mut buf).ok()?;

                let data = String::from_utf8(buf).ok()?;
                hash.insert(header.to_string(), data);
            } else {
                hash.insert(header.to_string(), content.to_string());
            }
        }

        let rmate = Rmate {
            display_name: hash.get("display-name").cloned().unwrap_or_default(),
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