use checksum_core::{
    gpg,
    hash::{hash_file, normalize_hash},
    manifest,
    report::{self, Format},
    scan, *,
};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const ALL: [Algorithm; 6] = [
    Algorithm::Md5,
    Algorithm::Sha1,
    Algorithm::Sha256,
    Algorithm::Sha512,
    Algorithm::Blake3,
    Algorithm::Xxhash3,
];
fn hash(path: &Path) -> HashResult {
    hash_file(path, &ALL, &AtomicBool::new(false), |_| {}).unwrap()
}
fn folder(path: &Path, manifest: Option<&Path>) -> ScanReport {
    scan::scan(
        path,
        vec![Algorithm::Sha256],
        ScanOptions::default(),
        manifest,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
}

#[test]
fn known_answer_vectors_for_all_algorithms() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("abc");
    fs::write(&path, b"abc").unwrap();
    let result = hash(&path);
    let expected = [
        "900150983cd24fb0d6963f7d28e17f72",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        "78af5f94892f3950",
    ];
    for (algorithm, expected) in ALL.iter().zip(expected) {
        assert_eq!(result.hashes[algorithm], expected);
    }
}

#[test]
fn empty_files_and_xxh3_leading_zeroes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty");
    fs::write(&path, []).unwrap();
    let empty = hash(&path);
    assert_eq!(empty.bytes, 0);
    assert_eq!(
        empty.hashes[&Algorithm::Sha256],
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        empty.hashes[&Algorithm::Blake3],
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    fs::write(&path, [31]).unwrap();
    assert_eq!(hash(&path).hashes[&Algorithm::Xxhash3], "05087bbed866d0de");
}

#[test]
fn chunk_boundaries_match_whole_buffer_and_rereads_do_not_cache() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large");
    for size in [
        1024 * 1024 - 1,
        1024 * 1024,
        1024 * 1024 + 1,
        2 * 1024 * 1024 + 7,
    ] {
        let data = vec![size as u8; size];
        fs::write(&path, &data).unwrap();
        let result = hash(&path);
        assert_eq!(
            result.hashes[&Algorithm::Sha256],
            format!("{:x}", Sha256::digest(&data))
        );
        assert_eq!(
            result.hashes[&Algorithm::Blake3],
            blake3::hash(&data).to_hex().to_string()
        );
    }
}

#[test]
fn cancellation_applies_to_empty_files_and_blake3() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(&path, []).unwrap();
    let cancel = AtomicBool::new(true);
    assert_eq!(
        hash_file(&path, &[Algorithm::Blake3], &cancel, |_| {})
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    fs::write(&path, vec![0; 2 * 1024 * 1024]).unwrap();
    cancel.store(false, Ordering::Release);
    let error = hash_file(&path, &[Algorithm::Blake3], &cancel, |_| {
        cancel.store(true, Ordering::Release);
    })
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
}

#[cfg(unix)]
#[test]
fn mutation_during_hashing_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(&path, b"original").unwrap();
    let mut changed = false;
    let result = hash_file(&path, &ALL, &AtomicBool::new(false), |_| {
        if !changed {
            fs::write(&path, b"changed!").unwrap();
            changed = true;
        }
    });
    assert_eq!(result.unwrap_err().kind, ErrorKind::Changed);
}

#[test]
fn pasted_hashes_accept_case_and_outer_whitespace() {
    assert_eq!(
        normalize_hash(&format!(" \n{}\t", "AB".repeat(32)), Algorithm::Sha256).unwrap(),
        "ab".repeat(32)
    );
    assert!(normalize_hash("abcd", Algorithm::Sha256).is_err());
}

#[test]
fn manifest_parser_rejects_bad_paths_duplicates_and_unicode_delimiters() {
    let hash = "a".repeat(64);
    for path in [
        "../secret",
        "/secret",
        "C:\\secret",
        "a/../../secret",
        "a\\..\\secret",
        "\\server\\share",
    ] {
        assert!(
            manifest::parse(&format!("{hash} *{path}\n"), Algorithm::Sha256).is_err(),
            "{path}"
        );
    }
    assert!(manifest::parse(&format!("{hash} *a\n{hash} *./a\n"), Algorithm::Sha256).is_err());
    assert!(manifest::parse(&format!("{hash}é filename"), Algorithm::Sha256).is_err());
    assert!(manifest::parse("", Algorithm::Sha256).is_err());
}

#[test]
fn hidden_directories_are_excluded_and_explicit_hidden_manifest_entries_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".hidden")).unwrap();
    fs::write(dir.path().join(".hidden/file"), b"abc").unwrap();
    fs::write(dir.path().join("visible"), b"abc").unwrap();
    let report = folder(dir.path(), None);
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.summary.skipped, 1);
    let sums = dir.path().join("SHA256SUMS");
    fs::write(
        &sums,
        format!(
            "{} *.hidden/file\n",
            report.entries[0].hashes[&Algorithm::Sha256]
        ),
    )
    .unwrap();
    let verified = folder(dir.path(), Some(&sums));
    assert_eq!(verified.summary.matched, 1);
    assert_eq!(verified.summary.new, 1);
}

#[cfg(unix)]
#[test]
fn symlink_cycles_are_skipped_and_manifest_symlinks_are_errors() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file"), b"abc").unwrap();
    symlink(dir.path(), dir.path().join("loop")).unwrap();
    let report = folder(dir.path(), None);
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.summary.skipped, 1);
    let sums = dir.path().join("SHA256SUMS");
    fs::write(
        &sums,
        format!(
            "{} *loop/file\n",
            report.entries[0].hashes[&Algorithm::Sha256]
        ),
    )
    .unwrap();
    let checked = folder(dir.path(), Some(&sums));
    assert_eq!(checked.status, ScanStatus::Partial);
    assert_eq!(checked.summary.failed, 1);
}

#[cfg(unix)]
#[test]
fn non_unicode_names_are_reported_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(OsString::from_vec(vec![0xff])), b"abc").unwrap();
    let report = folder(dir.path(), None);
    assert_eq!(report.status, ScanStatus::Partial);
    assert_eq!(report.summary.failed, 1);
}

#[test]
fn manifest_verification_distinguishes_all_file_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["same", "changed", "missing"] {
        fs::write(dir.path().join(name), b"abc").unwrap();
    }
    let baseline = folder(dir.path(), None);
    let sums = dir.path().join("SHA256SUMS");
    fs::write(
        &sums,
        manifest::encode(&baseline.entries, Algorithm::Sha256).unwrap(),
    )
    .unwrap();
    fs::write(dir.path().join("changed"), b"xyz").unwrap();
    fs::remove_file(dir.path().join("missing")).unwrap();
    fs::write(dir.path().join("new"), b"new").unwrap();
    let report = folder(dir.path(), Some(&sums));
    assert_eq!(
        (
            report.summary.matched,
            report.summary.changed,
            report.summary.missing,
            report.summary.new
        ),
        (1, 1, 1, 1)
    );
    assert!(report::serialize(&report, Format::Manifest, Some(Algorithm::Sha256), true).is_err());
}

#[test]
fn cancelled_scans_retain_partial_results_and_status() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        fs::write(dir.path().join(format!("file{i}")), b"abc").unwrap();
    }
    let cancel = AtomicBool::new(false);
    let report = scan::scan(
        dir.path(),
        vec![Algorithm::Sha256],
        ScanOptions {
            concurrency: 1,
            ..Default::default()
        },
        None,
        &cancel,
        |event| {
            if matches!(event, ScanEvent::Entry(_)) {
                cancel.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(report.status, ScanStatus::Cancelled);
    assert!(!report.entries.is_empty());
    assert!(report::serialize(&report, Format::Manifest, Some(Algorithm::Sha256), true).is_err());
    let json = report::serialize(&report, Format::Json, None, true).unwrap();
    let loaded: ScanReport = serde_json::from_slice(&json).unwrap();
    assert_eq!(loaded.status, ScanStatus::Cancelled);
}

#[test]
fn manifest_roundtrip_handles_spaces_unicode_and_gnu_escaping() {
    let mut hashes = Hashes::new();
    hashes.insert(Algorithm::Sha256, "a".repeat(64));
    let entries: Vec<_> = ["space name", "العربية.txt", "line\nbreak", "back\\slash"]
        .into_iter()
        .map(|name| FileEntry {
            path: name.into(),
            relative_path: name.into(),
            status: EntryStatus::Hashed,
            hashes: hashes.clone(),
            expected: None,
            error: None,
            bytes: 0,
        })
        .collect();
    let parsed = manifest::parse(
        &manifest::encode(&entries, Algorithm::Sha256).unwrap(),
        Algorithm::Sha256,
    )
    .unwrap();
    assert_eq!(parsed.len(), 4);
    assert!(parsed.contains_key("line\nbreak"));
    assert!(parsed.contains_key("العربية.txt"));
}

#[test]
fn csv_is_formula_safe_and_exports_cannot_overwrite_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("=1+1");
    fs::write(&input, b"abc").unwrap();
    let report = folder(dir.path(), None);
    let csv =
        String::from_utf8(report::serialize(&report, Format::Csv, None, true).unwrap()).unwrap();
    assert!(csv.contains("'=1+1"));
    assert!(report::save(&input, &report, Format::Json, None, true).is_err());
    assert_eq!(fs::read(&input).unwrap(), b"abc");
    let output = dir.path().join("report.json");
    report::save(&output, &report, Format::Json, None, true).unwrap();
    assert_eq!(
        serde_json::from_slice::<ScanReport>(&fs::read(output).unwrap())
            .unwrap()
            .entries
            .len(),
        1
    );
}

fn good_signature(signing: &str, primary: &str) -> String {
    format!("[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG 1234 Test Signer\n[GNUPG:] VALIDSIG {signing} 2026-01-01 1 0 4 0 1 8 00 {primary}\n[GNUPG:] TRUST_UNDEFINED 0 pgp\n")
}

#[test]
fn gpg_accepts_primary_or_subkey_fingerprints_and_marks_unchecked_identity() {
    let sub = "A".repeat(40);
    let primary = "B".repeat(40);
    let status = good_signature(&sub, &primary);
    for expected in [&sub, &primary] {
        assert!(
            gpg::parse_status(&status, Some(0), Some(expected), String::new())
                .unwrap()
                .overall_success
        );
    }
    let unchecked = gpg::parse_status(&status, Some(0), None, String::new()).unwrap();
    assert!(unchecked.overall_success);
    assert_eq!(unchecked.fingerprint_match, None);
    assert!(
        !gpg::parse_status(&status, Some(0), Some(&"C".repeat(40)), String::new())
            .unwrap()
            .overall_success
    );
}

#[test]
fn gpg_rejects_mixed_signatures_failed_exit_and_incomplete_status() {
    let good = good_signature(&"A".repeat(40), &"B".repeat(40));
    let mixed = format!("{good}[GNUPG:] NEWSIG\n[GNUPG:] BADSIG 5678 Bad Signer\n");
    for exit in [Some(0), Some(1), None] {
        assert!(
            !gpg::parse_status(&mixed, exit, None, String::new())
                .unwrap()
                .overall_success
        );
    }
    assert!(
        !gpg::parse_status(&good, Some(1), None, String::new())
            .unwrap()
            .overall_success
    );
    assert!(
        !gpg::parse_status("[GNUPG:] GOODSIG 1234 Test", Some(0), None, String::new())
            .unwrap()
            .overall_success
    );
    assert!(
        !gpg::parse_status("", Some(0), None, String::new())
            .unwrap()
            .overall_success
    );
}

#[test]
fn revoked_expired_and_error_signatures_never_pass() {
    for token in ["REVKEYSIG", "EXPSIG", "EXPKEYSIG", "ERRSIG", "NO_PUBKEY"] {
        let status = format!(
            "{}[GNUPG:] {token} 1234 Test\n",
            good_signature(&"A".repeat(40), &"B".repeat(40))
        );
        assert!(
            !gpg::parse_status(&status, Some(0), None, String::new())
                .unwrap()
                .overall_success
        );
    }
}
