use std::io;
use std::io::Error;

use ropey::Rope;

pub trait CoreBuffer: Default {
    fn from_reader<T: io::Read>(reader: T) -> io::Result<Self>;
}

#[derive(Default)]
pub struct RopeyCoreBuffer(Rope);

impl CoreBuffer for RopeyCoreBuffer {
    fn from_reader<T: io::Read>(reader: T) -> Result<Self, Error> {
        Ok(RopeyCoreBuffer(Rope::from_reader(reader)?))
    }
}
