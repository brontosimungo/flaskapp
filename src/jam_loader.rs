use std::{env, fs, io, path::PathBuf};

pub fn load_kernel_from_env() -> io::Result<Vec<u8>> {
    let path: PathBuf = env::var("MINER_JAM_PATH")
        .map(Into::into)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "MINER_JAM_PATH non défini"))?;
    fs::read(path)
}
