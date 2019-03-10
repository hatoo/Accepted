use crate::core::Core;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::path::PathBuf;

pub trait Storage {
    fn load(&mut self) -> Core;
    fn save(&mut self, core: &Core);
    fn path(&self) -> &Path;
}

impl Storage for PathBuf {
    fn load(&mut self) -> Core {
        fs::File::open(self.path())
            .and_then(|f| Core::from_reader(BufReader::new(f)))
            .unwrap_or_default()
    }

    fn save(&mut self, core: &Core) {
        if let Ok(f) = fs::File::create(self.path()) {
            let _ = core.buffer().write_to(BufWriter::new(f));
        }
    }

    fn path(&self) -> &Path {
        self.as_ref()
    }
}
