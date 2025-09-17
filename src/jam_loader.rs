use std::{env, fs, io, path::PathBuf};

pub fn load_kernel_from_env() -> io::Result<Vec<u8>> {
    // First try environment variable
    if let Ok(env_path) = env::var("MINER_JAM_PATH") {
        let path = PathBuf::from(env_path);
        return fs::read(path);
    }

    // Then try binary directory
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let kernel_path = exe_dir.join("miner.jam");
            if kernel_path.exists() {
                return fs::read(kernel_path);
            }
        }
    }

    // Finally try current directory
    let current_dir_path = PathBuf::from("miner.jam");
    if current_dir_path.exists() {
        fs::read(current_dir_path)
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, "miner.jam not found"))
    }
}