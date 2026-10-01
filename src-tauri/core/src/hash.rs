use crate::{Algorithm, Error, ErrorKind, HashProgress, HashResult, Hashes};
use md5::{Digest, Md5};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::{
    fs::Metadata,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub fn normalize_hash(value: &str, algorithm: Algorithm) -> Result<String, Error> {
    let value = value.trim();
    if value.len() != algorithm.hex_len() || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::invalid(format!(
            "{} requires {} hexadecimal characters",
            algorithm.label(),
            algorithm.hex_len()
        )));
    }
    Ok(value.to_ascii_lowercase())
}

pub(crate) fn same_metadata(a: &Metadata, b: &Metadata) -> bool {
    let same = a.len() == b.len()
        && a.modified().ok() == b.modified().ok()
        && a.created().ok() == b.created().ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        same && a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        same
    }
}

pub fn hash_file(
    path: &Path,
    algorithms: &[Algorithm],
    cancel: &AtomicBool,
    mut progress: impl FnMut(HashProgress),
) -> Result<HashResult, Error> {
    if cancel.load(Ordering::Acquire) {
        return Err(Error::cancelled());
    }
    if algorithms.is_empty() {
        return Err(Error::invalid("Select at least one algorithm"));
    }
    let display_path = path
        .to_str()
        .ok_or_else(|| Error::invalid("This filename cannot be represented as Unicode"))?
        .to_owned();
    let started = Instant::now();
    #[cfg(windows)]
    let mut file = {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_READ};
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
            .open(path)?
    };
    #[cfg(not(windows))]
    let mut file = std::fs::File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(Error::invalid("Select a regular file"));
    }
    let mut md5 = algorithms.contains(&Algorithm::Md5).then(Md5::new);
    let mut sha1 = algorithms.contains(&Algorithm::Sha1).then(Sha1::new);
    let mut sha256 = algorithms.contains(&Algorithm::Sha256).then(Sha256::new);
    let mut sha512 = algorithms.contains(&Algorithm::Sha512).then(Sha512::new);
    let mut blake3 = algorithms
        .contains(&Algorithm::Blake3)
        .then(blake3::Hasher::new);
    let mut xxh3 = algorithms
        .contains(&Algorithm::Xxhash3)
        .then(xxhash_rust::xxh3::Xxh3::new);
    let mut buffer = vec![0; 1024 * 1024];
    let mut read = 0;
    let mut last = Instant::now();
    progress(HashProgress {
        bytes_read: 0,
        total_bytes: before.len(),
        elapsed_ms: 0,
    });
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(Error::cancelled());
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        let chunk = &buffer[..n];
        if let Some(h) = &mut md5 {
            h.update(chunk);
        }
        if let Some(h) = &mut sha1 {
            h.update(chunk);
        }
        if let Some(h) = &mut sha256 {
            h.update(chunk);
        }
        if let Some(h) = &mut sha512 {
            h.update(chunk);
        }
        if let Some(h) = &mut blake3 {
            h.update(chunk);
        }
        if let Some(h) = &mut xxh3 {
            h.update(chunk);
        }
        read += n as u64;
        if last.elapsed() >= Duration::from_millis(100) {
            progress(HashProgress {
                bytes_read: read,
                total_bytes: before.len(),
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
            last = Instant::now();
        }
    }
    if cancel.load(Ordering::Acquire) {
        return Err(Error::cancelled());
    }
    let after = file.metadata()?;
    let current = std::fs::metadata(path).map_err(|_| {
        Error::new(
            ErrorKind::Changed,
            "The file was removed or replaced while hashing",
        )
    })?;
    if read != before.len() || !same_metadata(&before, &after) || !same_metadata(&after, &current) {
        return Err(Error::new(
            ErrorKind::Changed,
            "The file changed while hashing; retry when it is no longer being written",
        ));
    }
    let mut hashes = Hashes::new();
    if let Some(h) = md5 {
        hashes.insert(Algorithm::Md5, format!("{:x}", h.finalize()));
    }
    if let Some(h) = sha1 {
        hashes.insert(Algorithm::Sha1, format!("{:x}", h.finalize()));
    }
    if let Some(h) = sha256 {
        hashes.insert(Algorithm::Sha256, format!("{:x}", h.finalize()));
    }
    if let Some(h) = sha512 {
        hashes.insert(Algorithm::Sha512, format!("{:x}", h.finalize()));
    }
    if let Some(h) = blake3 {
        hashes.insert(Algorithm::Blake3, h.finalize().to_hex().to_string());
    }
    if let Some(h) = xxh3 {
        hashes.insert(Algorithm::Xxhash3, format!("{:016x}", h.digest()));
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    progress(HashProgress {
        bytes_read: read,
        total_bytes: before.len(),
        elapsed_ms,
    });
    Ok(HashResult {
        path: display_path,
        hashes,
        bytes: read,
        elapsed_ms,
    })
}
