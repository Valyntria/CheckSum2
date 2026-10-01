use crate::{hash::hash_file, manifest, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};

struct Task {
    path: PathBuf,
    relative: String,
    expected: Option<String>,
    size: u64,
    issue: Option<String>,
}

fn hidden(path: &Path, metadata: &fs::Metadata) -> bool {
    let dot = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'));
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        dot || metadata.file_attributes() & 2 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        dot
    }
}

fn collect(
    root: &Path,
    options: &ScanOptions,
    cancel: &AtomicBool,
    issues: &mut Vec<FileIssue>,
    skipped: &mut usize,
    mut discovered: impl FnMut(usize),
) -> Vec<(PathBuf, u64)> {
    let mut folders = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(folder) = folders.pop() {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(e) => {
                issues.push(FileIssue {
                    path: folder.display().to_string(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        for entry in entries {
            if cancel.load(Ordering::Acquire) {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    issues.push(FileIssue {
                        path: folder.display().to_string(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(md) => md,
                Err(e) => {
                    issues.push(FileIssue {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            if metadata.file_type().is_symlink()
                || (!options.include_hidden && hidden(&path, &metadata))
            {
                *skipped += 1;
                continue;
            }
            if path.to_str().is_none() {
                issues.push(FileIssue {
                    path: path.display().to_string(),
                    message: "Filename cannot be represented as Unicode; skipped".into(),
                });
                continue;
            }
            if metadata.is_file() {
                files.push((path, metadata.len()));
                discovered(files.len());
            } else if metadata.is_dir() && options.recursive {
                folders.push(path);
            } else {
                *skipped += 1;
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

pub fn scan(
    root: &Path,
    algorithms: Vec<Algorithm>,
    options: ScanOptions,
    manifest_path: Option<&Path>,
    cancel: &AtomicBool,
    emit: impl Fn(ScanEvent) + Sync,
) -> Result<ScanReport, Error> {
    if !(1..=8).contains(&options.concurrency) {
        return Err(Error::invalid("Concurrency must be between 1 and 8"));
    }
    let algorithms: Vec<_> = algorithms
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if algorithms.is_empty() {
        return Err(Error::invalid("Select at least one algorithm"));
    }
    if manifest_path.is_some() && algorithms.len() != 1 {
        return Err(Error::invalid("Choose exactly one manifest algorithm"));
    }
    let started = Instant::now();
    let root = root.canonicalize()?;
    if !root.is_dir() || root.to_str().is_none() {
        return Err(Error::invalid("Select a folder with a Unicode path"));
    }
    let expected = manifest_path
        .map(|p| manifest::read(p, algorithms[0]))
        .transpose()?;
    let manifest_absolute = manifest_path.map(Path::canonicalize).transpose()?;
    let mut issues = Vec::new();
    let mut skipped = 0;
    let mut last_discovery = Instant::now();
    emit(ScanEvent::Progress(ScanProgress {
        phase: "discovering",
        discovered: 0,
        completed: 0,
        total: 0,
        failed: 0,
        bytes_read: 0,
        total_bytes: 0,
        elapsed_ms: 0,
    }));
    let files = collect(
        &root,
        &options,
        cancel,
        &mut issues,
        &mut skipped,
        |count| {
            if last_discovery.elapsed() >= Duration::from_millis(100) {
                emit(ScanEvent::Progress(ScanProgress {
                    phase: "discovering",
                    discovered: count,
                    completed: 0,
                    total: 0,
                    failed: 0,
                    bytes_read: 0,
                    total_bytes: 0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                }));
                last_discovery = Instant::now();
            }
        },
    );
    let mut tasks = BTreeMap::new();
    for (path, size) in files {
        if manifest_absolute.as_ref() == Some(&path) {
            skipped += 1;
            continue;
        }
        let relative = manifest::relative_name(&root, &path)?;
        tasks.insert(
            relative.clone(),
            Task {
                expected: expected.as_ref().and_then(|e| e.get(&relative).cloned()),
                path,
                relative,
                size,
                issue: None,
            },
        );
    }
    // Explicitly listed files are always checked, even when hidden-file discovery is disabled.
    if let Some(expected) = &expected {
        for (relative, hash) in expected {
            if tasks.contains_key(relative) {
                continue;
            }
            let (path, issue) = match manifest::target(&root, relative) {
                Ok(p) => (p, None),
                Err(e) => (root.join(relative), Some(e.message)),
            };
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            tasks.insert(
                relative.clone(),
                Task {
                    path,
                    relative: relative.clone(),
                    expected: Some(hash.clone()),
                    size,
                    issue,
                },
            );
        }
    }
    let tasks: Vec<_> = tasks.into_values().collect();
    let total = tasks.len();
    let total_bytes: u64 = tasks.iter().map(|t| t.size).sum();
    let next = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(issues.len());
    let bytes = AtomicU64::new(0);
    let last_progress = Mutex::new(Instant::now());
    let publish = |force: bool| {
        let mut last = last_progress.lock().unwrap_or_else(|p| p.into_inner());
        if force || last.elapsed() >= Duration::from_millis(100) {
            emit(ScanEvent::Progress(ScanProgress {
                phase: "hashing",
                discovered: total,
                completed: completed.load(Ordering::Relaxed),
                total,
                failed: failed.load(Ordering::Relaxed),
                bytes_read: bytes.load(Ordering::Relaxed),
                total_bytes,
                elapsed_ms: started.elapsed().as_millis() as u64,
            }));
            *last = Instant::now();
        }
    };
    publish(true);
    let (sender, receiver) = mpsc::channel();
    let mut entries = Vec::new();
    let workers_ok = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..options.concurrency.min(total) {
            let sender = sender.clone();
            let tasks = &tasks;
            let algorithms = &algorithms;
            let next = &next;
            let completed = &completed;
            let failed = &failed;
            let bytes = &bytes;
            let publish = &publish;
            let root = &root;
            let verifying = expected.is_some();
            workers.push(scope.spawn(move || loop {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(task) = tasks.get(index) else {
                    break;
                };
                let mut entry = FileEntry {
                    path: task.path.display().to_string(),
                    relative_path: task.relative.clone(),
                    status: EntryStatus::Error,
                    hashes: Hashes::new(),
                    expected: task.expected.clone(),
                    error: None,
                    bytes: 0,
                };
                let mut previous = 0;
                let result = if let Some(issue) = &task.issue {
                    Err(Error::invalid(issue))
                } else if verifying {
                    manifest::target(root, &task.relative).and_then(|path| {
                        hash_file(&path, algorithms, cancel, |p| {
                            bytes.fetch_add(
                                p.bytes_read.saturating_sub(previous),
                                Ordering::Relaxed,
                            );
                            previous = p.bytes_read;
                            publish(false);
                        })
                    })
                } else {
                    hash_file(&task.path, algorithms, cancel, |p| {
                        bytes.fetch_add(p.bytes_read.saturating_sub(previous), Ordering::Relaxed);
                        previous = p.bytes_read;
                        publish(false);
                    })
                };
                match result {
                    Ok(result) => {
                        entry.bytes = result.bytes;
                        entry.hashes = result.hashes;
                        entry.status = if let Some(expected) = &task.expected {
                            if entry.hashes.get(&algorithms[0]) == Some(expected) {
                                EntryStatus::Matched
                            } else {
                                EntryStatus::Changed
                            }
                        } else if verifying {
                            EntryStatus::New
                        } else {
                            EntryStatus::Hashed
                        };
                    }
                    Err(e) if e.kind == ErrorKind::Cancelled => break,
                    Err(e) => {
                        entry.status = if task.issue.is_none()
                            && task.expected.is_some()
                            && fs::symlink_metadata(&task.path)
                                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                        {
                            EntryStatus::Missing
                        } else {
                            EntryStatus::Error
                        };
                        entry.error = Some(e.message);
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
                completed.fetch_add(1, Ordering::Relaxed);
                if sender.send(entry).is_err() {
                    break;
                }
                publish(false);
            }));
        }
        drop(sender);
        for entry in receiver {
            emit(ScanEvent::Entry(entry.clone()));
            entries.push(entry);
        }
        workers
            .into_iter()
            .map(|h| h.join().is_ok())
            .fold(true, |ok, next| ok & next)
    });
    if !workers_ok {
        issues.push(FileIssue {
            path: root.display().to_string(),
            message: "A scan worker failed; results are incomplete".into(),
        });
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut summary = ScanSummary {
        discovered: total,
        processed: entries.len(),
        skipped,
        bytes_read: bytes.load(Ordering::Relaxed),
        elapsed_ms: started.elapsed().as_millis() as u64,
        failed: issues.len(),
        ..Default::default()
    };
    for entry in &entries {
        match entry.status {
            EntryStatus::Matched => summary.matched += 1,
            EntryStatus::Changed => summary.changed += 1,
            EntryStatus::Missing => summary.missing += 1,
            EntryStatus::New => summary.new += 1,
            EntryStatus::Error => summary.failed += 1,
            EntryStatus::Hashed => {}
        }
    }
    let status = if cancel.load(Ordering::Acquire) {
        ScanStatus::Cancelled
    } else if summary.failed > 0 || !workers_ok {
        ScanStatus::Partial
    } else {
        ScanStatus::Completed
    };
    publish(true);
    Ok(ScanReport {
        schema_version: 1,
        root: root.display().to_string(),
        algorithms,
        status,
        summary,
        entries,
        issues,
    })
}
