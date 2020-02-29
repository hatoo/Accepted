use crate::core::Core;
use crate::core::CoreBuffer;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::path::PathBuf;

pub trait Storage<B: CoreBuffer>: Send {
    fn load(&mut self) -> Core<B>;
    fn save(&mut self, core: &Core<B>) -> bool;
    fn path(&self) -> &Path;
}

impl<B: CoreBuffer> Storage<B> for PathBuf {
    fn load(&mut self) -> Core<B> {
        fs::File::open(self)
            .and_then(|f| Core::<B>::from_reader(BufReader::new(f)))
            .unwrap_or_default()
    }

    fn save(&mut self, core: &Core<B>) -> bool {
        if let Ok(f) = fs::File::create(self) {
            core.core_buffer().write_to(&mut BufWriter::new(f)).is_ok()
        } else {
            false
        }
    }

    fn path(&self) -> &Path {
        self.as_ref()
    }
}
