use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, io};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
    Blake3,
    Xxhash3,
}

impl Algorithm {
    pub fn hex_len(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 | Self::Blake3 => 64,
            Self::Sha512 => 128,
            Self::Xxhash3 => 16,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Blake3 => "blake3",
            Self::Xxhash3 => "xxhash3",
        }
    }
}

pub type Hashes = BTreeMap<Algorithm, String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Cancelled,
    Changed,
    InvalidInput,
    Io,
    Timeout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }
    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "Operation cancelled")
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::new(ErrorKind::Io, e.to_string())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HashResult {
    pub path: String,
    pub hashes: Hashes,
    pub bytes: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HashProgress {
    pub bytes_read: u64,
    pub total_bytes: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Hashed,
    Matched,
    Changed,
    Missing,
    New,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub relative_path: String,
    pub status: EntryStatus,
    pub hashes: Hashes,
    pub expected: Option<String>,
    pub error: Option<String>,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Completed,
    Partial,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanSummary {
    pub discovered: usize,
    pub processed: usize,
    pub matched: usize,
    pub changed: usize,
    pub missing: usize,
    pub new: usize,
    pub failed: usize,
    pub skipped: usize,
    pub bytes_read: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileIssue {
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: u32,
    pub root: String,
    pub algorithms: Vec<Algorithm>,
    pub status: ScanStatus,
    pub summary: ScanSummary,
    pub entries: Vec<FileEntry>,
    pub issues: Vec<FileIssue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanProgress {
    pub phase: &'static str,
    pub discovered: usize,
    pub completed: usize,
    pub total: usize,
    pub failed: usize,
    pub bytes_read: u64,
    pub total_bytes: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ScanEvent {
    Progress(ScanProgress),
    Entry(FileEntry),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanOptions {
    pub recursive: bool,
    pub include_hidden: bool,
    pub concurrency: usize,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            include_hidden: false,
            concurrency: 2,
        }
    }
}
