use std::env;
use std::path::Path;

pub fn set_env<P: AsRef<Path>>(path: P) {
    let file_path = path.as_ref().as_os_str();
    let file_stem = path.as_ref().file_stem().unwrap_or_default();

    env::set_var("FILE_PATH", file_path);
    env::set_var("FILE_STEM", file_stem);
}
