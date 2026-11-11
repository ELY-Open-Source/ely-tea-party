use std::{
    fs,
    io::ErrorKind,
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn remove(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(error) if error.kind() == ErrorKind::NotFound => return,
            Err(error)
                if error.kind() == ErrorKind::DirectoryNotEmpty && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("cleanup {}: {error}", path.display()),
        }
    }
}
