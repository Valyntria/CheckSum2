# CheckSum

A local desktop application for file checksums, folder integrity reports, standard checksum manifests, and detached GPG signatures. Built with React, Tauri 1, and a Rust core.

## Features

- MD5, SHA-1, SHA-256, SHA-512, BLAKE3, and XXH3-64.
- Fresh reads on file selection, recalculation, and verification; no path-only checksum cache.
- Independent cancellable jobs with progress, elapsed time, throughput, and estimated time remaining.
- Folder scans with bounded concurrency and incremental, virtualized results.
- CSV and JSON reports that retain errors and partial/cancelled status.
- Import and export GNU-style checksum manifests, including `SHA256SUMS` and `.sha256` files.
- Manifest comparisons show matched, changed, missing, additional, and failed files.
- Detached GPG verification using an installed GnuPG and its local public keyring. Automatic key retrieval is disabled.
- Dark/light themes and persisted algorithm preferences.

## Installation

Download an installer or portable archive from [Releases](https://github.com/Valyntria/CheckSum/releases), when available. Assets built from earlier commits do not include these changes until a new release is published.

- **Windows:** the app uses WebView2. The installer includes its bootstrapper; first-time runtime installation may need internet access. Portable builds require WebView2 already installed.
- **macOS:** use the provided app/DMG for your CPU architecture. A build produced without signing/notarization may require the standard macOS approval flow.
- **Linux:** use a compatible `.deb`, `.rpm`, or AppImage. This project still targets the Tauri 1 / WebKitGTK 4.0 stack.
- **GPG:** install GnuPG separately and ensure `gpg --version` works from the environment launching the app. The app does not bundle GnuPG or download/import public keys.

## Using the app

### Single file

Choose or drop a file and select algorithms. SHA-256 is the initial default; previously saved preferences are retained. Drop the same file again or select **Recalculate** to read it again after editing it.

For verification, choose the expected algorithm, paste its hexadecimal checksum, and select **Verify with a fresh read**. Upper/lowercase hex and surrounding whitespace are accepted. SHA-256 and BLAKE3 are both 64 hex characters, so length alone cannot identify the algorithm.

MD5 and SHA-1 are available for compatibility with existing checksum lists. XXH3-64 is noncryptographic. A checksum match does not identify the publisher: obtain the expected value from a trusted source.

### Folder scans and manifests

Choose **Folder / Manifest**, select a folder, and start a scan. Hidden files and directories are excluded by default. Windows hidden attributes are respected. Directory and file symlinks are skipped; non-Unicode paths are reported as errors. Concurrency can be set to 1, 2, 4, or 8 files; start with 1 or 2 on rotating disks or network shares.

Export a complete scan as a checksum manifest to create a baseline. Choose **Verify a checksum manifest**, select the manifest and the folder that its paths are relative to, then confirm the algorithm. The manifest itself is excluded from discovery. Files explicitly named by the manifest are always checked, even if hidden or outside the selected discovery depth; unsafe paths and symlinks are rejected.

Supported lines follow GNU checksum syntax:

```text
<hexadecimal-checksum> *relative/path/to/file
<hexadecimal-checksum>  filename with spaces
```

GNU escaped filenames (backslash, newline, carriage return), UTF-8 names, blank lines, and `#` comment lines are supported. Duplicate paths, absolute paths, parent traversal, malformed hashes, and manifests larger than 16 MiB are rejected. This version does not import BSD-tagged formats such as `SHA256 (file) = hash`.

Reports distinguish **completed**, **partial**, and **cancelled** scans. A completed comparison can still contain changed or missing files: inspect the outcome counts. Only complete results without errors, missing files, or mismatches can be exported as a new checksum manifest. CSV/JSON preserve partial results. CSV includes scan metadata and escapes formula-like filenames for spreadsheet safety; JSON retains exact filenames and the report schema.

### GPG verification

Select the data file and detached `.sig` or `.asc` signature. Supply a full primary-key or signing-subkey fingerprint from an independently trusted source when checking publisher identity. Changing any input clears the previous result.

Success requires GPG exit code zero, at least one complete valid signature, no invalid/expired/revoked signatures, and a matching valid signer when an expected fingerprint was supplied. Multiple signatures are assessed individually. Without an expected fingerprint, the app reports cryptographic validity with **identity not checked**; a key's displayed user ID is not independently authenticated.

Verification invokes GPG with `--no-options`, `--no-auto-key-retrieve`, `--no-auto-key-locate`, and `--no-auto-key-import`, while using the normal local keyring. Key-management and trust decisions happen outside CheckSum. Verification has a five-minute timeout and bounded output capture.

## Development

Use Node 24.15 or later (Node 22.22.2 is also supported), npm, and a current stable Rust toolchain with `rustfmt` and `clippy`.

```sh
npm ci
npm run tauri dev
```

Install [Tauri 1 platform prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites/) first. Windows development requires the MSVC build tools and WebView2. On Ubuntu 22.04, install the GTK3/WebKitGTK 4.0 development packages listed in `.github/workflows/ci.yml`. Ubuntu 24.04's default WebKitGTK 4.1 packages are not a drop-in replacement for this Tauri 1 stack.

`npm run dev` starts the frontend server only. File access and GPG commands require the Tauri desktop runtime.

### Checks

```sh
npm test
npm run test:release
npm run build
npm audit
cargo test --manifest-path src-tauri/Cargo.toml -p checksum-core --locked
cargo clippy --manifest-path src-tauri/Cargo.toml -p checksum-core --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
```

The standalone core tests require no desktop/WebView libraries. With platform prerequisites installed, test the complete workspace and build native bundles:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --workspace --features custom-protocol --locked
npm run tauri build
```

### Source layout

| Path | Responsibility |
| --- | --- |
| `src/components/` | Tabs, algorithm selection, exports, progress, and virtualized file results |
| `src/hooks/useJob.ts` | Native job lifecycle, stale-result protection, cancellation, and listener cleanup |
| `src/lib/` | Shared frontend types and pure utilities |
| `src-tauri/src/main.rs` | Tauri command and event adapters |
| `src-tauri/core/src/` | Hashing, traversal, manifests, reports, GPG, and job registry |
| `src-tauri/core/tests/` | Native correctness regression tests |
| `tests/` | Frontend job and component regression tests |
| `scripts/release-assets.mjs` | Flat release assets and portable SHA256 checksums |

The vendored Wry 0.24.11 differs from upstream by an import of `webkit2gtk::SettingsExt` in `src/webview/webkitgtk/mod.rs`. This compatibility patch should be reassessed during a coordinated Tauri upgrade. Keep the API, CLI, Rust dependencies, configuration, and platform tests aligned when upgrading.

## Releases

CI checks frontend behavior, the core, and native compilation on Windows, macOS, and Linux. Update the version consistently in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` before tagging a release. A `v*` tag triggers the draft release workflow; manual dispatch requires an existing tag.

Installers are staged under unique filenames. `SHA256SUMS.txt` refers to those exact downloaded filenames, with no build-directory prefixes or duplicate `.app.zip` entries. From a folder containing the downloaded assets and manifest, compatible GNU tools can run:

```sh
sha256sum --check SHA256SUMS.txt
```

Code-signing and notarization require the maintainer's platform certificates. The existing Tauri updater-key environment variables alone do not sign Windows or macOS executables. Confirm installation and native dialogs on each target OS before publishing the draft.

## Consistency and privacy

Hashing uses one buffered pass for all selected algorithms. File metadata is checked before and after reading, including Unix file identity/change time; Windows hashing opens files without sharing write/delete access. These measures detect ordinary changes and replacements but do not create an atomic snapshot of a live filesystem. For consistent backup baselines, stop writers or use a filesystem snapshot. Cancellation may wait for an outstanding operating-system read on an unresponsive device.

The app has no application telemetry or file-upload feature. Hashing is local, and GPG key retrieval is explicitly disabled. Runtime/OS updates, installer bootstrap behavior, and access to user-selected network filesystems are separate from application verification.

See [the review-fix mapping](docs/REVIEW-FIXES.md) for the changes and their limitations.

## License

MIT. See [LICENSE](LICENSE).
