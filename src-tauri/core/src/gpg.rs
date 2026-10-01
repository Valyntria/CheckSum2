use crate::{hash::same_metadata, Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Signature {
    pub valid: bool,
    pub signer: Option<String>,
    pub signing_fingerprint: Option<String>,
    pub primary_fingerprint: Option<String>,
    pub trust: Option<String>,
    pub problems: Vec<String>,
    #[serde(skip)]
    good: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpgResult {
    pub overall_success: bool,
    pub exit_code: Option<i32>,
    pub fingerprint_match: Option<bool>,
    pub signatures: Vec<Signature>,
    pub message: String,
    pub diagnostics: String,
}

pub fn normalize_fingerprint(input: &str) -> Result<String, Error> {
    let value: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if !matches!(value.len(), 40 | 64) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::invalid(
            "Enter a full 40- or 64-character hexadecimal key fingerprint",
        ));
    }
    Ok(value.to_ascii_uppercase())
}

pub fn parse_status(
    status: &str,
    exit_code: Option<i32>,
    expected: Option<&str>,
    diagnostics: String,
) -> Result<GpgResult, Error> {
    let expected = expected
        .filter(|s| !s.trim().is_empty())
        .map(normalize_fingerprint)
        .transpose()?;
    let mut signatures = Vec::new();
    let mut current: Option<Signature> = None;
    let mut process_failure = false;
    for line in status.lines() {
        let Some(line) = line.strip_prefix("[GNUPG:] ") else {
            continue;
        };
        let (token, rest) = line.split_once(' ').unwrap_or((line, ""));
        if token == "NEWSIG" {
            if let Some(sig) = current.take() {
                signatures.push(sig);
            }
            current = Some(Signature::default());
            continue;
        }
        if matches!(token, "FAILURE" | "ERROR" | "NODATA" | "BADARMOR") {
            process_failure = true;
        }
        if !matches!(
            token,
            "GOODSIG"
                | "VALIDSIG"
                | "BADSIG"
                | "ERRSIG"
                | "EXPSIG"
                | "EXPKEYSIG"
                | "REVKEYSIG"
                | "NO_PUBKEY"
        ) && !token.starts_with("TRUST_")
        {
            continue;
        }
        let sig = current.get_or_insert_with(Signature::default);
        match token {
            "GOODSIG" => {
                sig.good = true;
                sig.signer = rest.split_once(' ').map(|(_, name)| name.to_owned());
            }
            "VALIDSIG" => {
                let parts: Vec<_> = rest.split_whitespace().collect();
                sig.signing_fingerprint = parts.first().and_then(|s| normalize_fingerprint(s).ok());
                sig.primary_fingerprint = parts
                    .get(9)
                    .and_then(|s| normalize_fingerprint(s).ok())
                    .or_else(|| sig.signing_fingerprint.clone());
            }
            "BADSIG" | "ERRSIG" | "EXPSIG" | "EXPKEYSIG" | "REVKEYSIG" | "NO_PUBKEY" => {
                sig.problems.push(token.into())
            }
            _ => sig.trust = Some(token.trim_start_matches("TRUST_").to_lowercase()),
        }
    }
    if let Some(sig) = current {
        signatures.push(sig);
    }
    for sig in &mut signatures {
        sig.valid = sig.good && sig.signing_fingerprint.is_some() && sig.problems.is_empty();
    }
    let fingerprint_match = expected.as_ref().map(|fp| {
        signatures.iter().any(|s| {
            s.valid
                && (s.signing_fingerprint.as_ref() == Some(fp)
                    || s.primary_fingerprint.as_ref() == Some(fp))
        })
    });
    let overall_success = exit_code == Some(0)
        && !process_failure
        && !signatures.is_empty()
        && signatures.iter().all(|s| s.valid)
        && fingerprint_match != Some(false);
    let message = if overall_success {
        if expected.is_some() { "All signatures are valid; the expected key fingerprint matches." } else { "All signatures are valid. Signer identity was not checked against an expected fingerprint." }
    } else if fingerprint_match == Some(false) && exit_code == Some(0) { "The expected key fingerprint does not match a valid signer." }
    else { "Verification failed or is incomplete. Review the signature details and GPG diagnostics." }.to_owned();
    Ok(GpgResult {
        overall_success,
        exit_code,
        fingerprint_match,
        signatures,
        message,
        diagnostics,
    })
}

fn capture(mut reader: impl Read) -> std::io::Result<String> {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        let remaining = (2 * 1024 * 1024_usize).saturating_sub(captured.len());
        truncated |= n > remaining;
        captured.extend_from_slice(&buffer[..n.min(remaining)]);
    }
    if truncated {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "GPG output exceeded the 2 MiB limit; verification is incomplete",
        ));
    }
    Ok(String::from_utf8_lossy(&captured).into_owned())
}

pub fn verify(
    file: &Path,
    signature: &Path,
    expected: Option<&str>,
    cancel: &AtomicBool,
) -> Result<GpgResult, Error> {
    if cancel.load(Ordering::Acquire) {
        return Err(Error::cancelled());
    }
    if let Some(value) = expected.filter(|v| !v.trim().is_empty()) {
        normalize_fingerprint(value)?;
    }
    let before_file = std::fs::metadata(file)?;
    let before_sig = std::fs::metadata(signature)?;
    if !before_file.is_file() || !before_sig.is_file() {
        return Err(Error::invalid(
            "Select a regular data file and detached signature",
        ));
    }
    let mut command = Command::new("gpg");
    command
        .args([
            "--no-options",
            "--batch",
            "--no-tty",
            "--no-auto-key-retrieve",
            "--no-auto-key-locate",
            "--no-auto-key-import",
            "--status-fd=1",
            "--verify",
            "--",
        ])
        .arg(signature)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::invalid("GPG was not found. Install GnuPG and ensure gpg is on PATH.")
        } else {
            e.into()
        }
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::invalid("Could not capture GPG status"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::invalid("Could not capture GPG diagnostics"))?;
    let out = std::thread::spawn(move || capture(stdout));
    let err = std::thread::spawn(move || capture(stderr));
    let started = Instant::now();
    let mut interrupted = None;
    let status = loop {
        if cancel.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(300) {
            interrupted = Some(if cancel.load(Ordering::Acquire) {
                Error::cancelled()
            } else {
                Error::new(ErrorKind::Timeout, "GPG exceeded the five-minute limit")
            });
            let _ = child.kill();
            break child.wait();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e);
            }
        }
    };
    let stdout = out
        .join()
        .map_err(|_| Error::invalid("GPG status reader failed"))??;
    let stderr = err
        .join()
        .map_err(|_| Error::invalid("GPG diagnostic reader failed"))??;
    if let Some(error) = interrupted {
        return Err(error);
    }
    if cancel.load(Ordering::Acquire) {
        return Err(Error::cancelled());
    }
    if !same_metadata(&before_file, &std::fs::metadata(file)?)
        || !same_metadata(&before_sig, &std::fs::metadata(signature)?)
    {
        return Err(Error::new(
            ErrorKind::Changed,
            "A verification input changed while GPG was running",
        ));
    }
    parse_status(&stdout, status?.code(), expected, stderr)
}
