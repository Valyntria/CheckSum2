use crate::{manifest, Algorithm, Error, ScanReport, ScanStatus};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Csv,
    Manifest,
}

fn spreadsheet_cell(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r', '\n']) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

pub fn serialize(
    report: &ScanReport,
    format: Format,
    algorithm: Option<Algorithm>,
    exclude_empty: bool,
) -> Result<Vec<u8>, Error> {
    match format {
        Format::Json => {
            serde_json::to_vec_pretty(report).map_err(|e| Error::invalid(e.to_string()))
        }
        Format::Manifest => {
            if report.status != ScanStatus::Completed
                || report.summary.failed > 0
                || report.summary.missing > 0
                || report.summary.changed > 0
                || report.entries.is_empty()
            {
                return Err(Error::invalid("Checksum manifests require complete results with no errors, missing files, or mismatches. CSV/JSON can preserve partial results."));
            }
            Ok(manifest::encode(
                &report.entries,
                algorithm.ok_or_else(|| Error::invalid("Choose a manifest algorithm"))?,
            )?
            .into_bytes())
        }
        Format::Csv => {
            let all = [
                Algorithm::Md5,
                Algorithm::Sha1,
                Algorithm::Sha256,
                Algorithm::Sha512,
                Algorithm::Blake3,
                Algorithm::Xxhash3,
            ];
            let algorithms: Vec<_> = all
                .into_iter()
                .filter(|a| {
                    !exclude_empty || report.entries.iter().any(|e| e.hashes.contains_key(a))
                })
                .collect();
            let mut writer = csv::Writer::from_writer(Vec::new());
            let mut header = vec![
                "scan_status",
                "root",
                "discovered",
                "processed",
                "failed",
                "skipped",
                "relative_path",
                "path",
                "file_status",
                "error",
                "expected",
            ];
            header.extend(algorithms.iter().map(|a| a.label()));
            writer
                .write_record(&header)
                .map_err(|e| Error::invalid(e.to_string()))?;
            let scan_status = match report.status {
                ScanStatus::Completed => "completed",
                ScanStatus::Partial => "partial",
                ScanStatus::Cancelled => "cancelled",
            };
            let prefix = || {
                vec![
                    scan_status.to_owned(),
                    spreadsheet_cell(&report.root),
                    report.summary.discovered.to_string(),
                    report.summary.processed.to_string(),
                    report.summary.failed.to_string(),
                    report.summary.skipped.to_string(),
                ]
            };
            for entry in &report.entries {
                let mut row = prefix();
                let status = serde_json::to_value(&entry.status)
                    .map_err(|e| Error::invalid(e.to_string()))?;
                row.extend([
                    spreadsheet_cell(&entry.relative_path),
                    spreadsheet_cell(&entry.path),
                    status.as_str().unwrap_or("error").to_owned(),
                    spreadsheet_cell(entry.error.as_deref().unwrap_or("")),
                    entry.expected.clone().unwrap_or_default(),
                ]);
                row.extend(
                    algorithms
                        .iter()
                        .map(|a| entry.hashes.get(a).cloned().unwrap_or_default()),
                );
                writer
                    .write_record(row)
                    .map_err(|e| Error::invalid(e.to_string()))?;
            }
            // Preserve discovery failures and even empty/cancelled scan summaries in CSV.
            for issue in &report.issues {
                let mut row = prefix();
                row.extend([
                    String::new(),
                    spreadsheet_cell(&issue.path),
                    "discovery_error".into(),
                    spreadsheet_cell(&issue.message),
                    String::new(),
                ]);
                row.resize(header.len(), String::new());
                writer
                    .write_record(row)
                    .map_err(|e| Error::invalid(e.to_string()))?;
            }
            if report.entries.is_empty() && report.issues.is_empty() {
                let mut row = prefix();
                row.resize(header.len(), String::new());
                writer
                    .write_record(row)
                    .map_err(|e| Error::invalid(e.to_string()))?;
            }
            writer
                .into_inner()
                .map_err(|e| Error::invalid(e.to_string()))
        }
    }
}

pub fn save(
    path: &Path,
    report: &ScanReport,
    format: Format,
    algorithm: Option<Algorithm>,
    exclude_empty: bool,
) -> Result<(), Error> {
    let bytes = serialize(report, format, algorithm, exclude_empty)?;
    if let Ok(target) = path.canonicalize() {
        if report
            .entries
            .iter()
            .any(|e| Path::new(&e.path).canonicalize().is_ok_and(|p| p == target))
        {
            return Err(Error::invalid(
                "A report cannot overwrite one of its input files",
            ));
        }
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut output = tempfile::NamedTempFile::new_in(parent)?;
    output.write_all(&bytes)?;
    output.as_file().sync_all()?;
    output.persist(path).map_err(|e| Error::from(e.error))?;
    Ok(())
}
