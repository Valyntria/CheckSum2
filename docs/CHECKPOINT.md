# CheckSum current-work checkpoint

Prepared 1 October 2026 from the local working tree, based on upstream commit `7b4618ff829b57d5266073c16f2161c03fca74d9`.

This is a source checkpoint. The work has not been pushed to GitHub and no installer is included.

## Implemented

- Rewritten React frontend with typed components, independent jobs, stale-result protection, reliable listener cleanup, fresh-file verification, and SHA-256 defaults.
- Rust core separated from the Tauri adapter; all six hashes use a single cancellable streaming read, with file-change checks and correctly padded XXH3-64 output.
- GPG verification evaluates exit status, every signature, primary/subkey fingerprints, and expected identity. Input changes invalidate old results; automatic key retrieval is disabled.
- Folder scans retain partial results and explicit errors, skip symlinks, handle hidden folders and non-Unicode paths, and provide progress, concurrency controls, and virtualized results.
- GNU checksum manifest import/export and folder comparison, plus typed CSV/JSON reports and atomic exports.
- Release asset staging and checksum generation fixes, expanded CI, dependency updates, regression tests, README, and a review-to-fix mapping.

See `docs/REVIEW-FIXES.md` for the detailed mapping and limitations. The archive contains every rewritten and new source file, including lockfiles.

## Checks completed so far

| Check | Result |
| --- | --- |
| Frontend production build | Passed |
| Frontend regression tests | 10 passed |
| Rust core regression tests | 18 passed |
| Release asset tests | 2 passed |
| Linux Rust core Clippy, warnings denied | Passed |
| Windows Rust core Clippy, warnings denied | Passed |
| Windows desktop cross-compilation check (`cargo check`) | Passed |
| npm dependency audit after dependency refresh | 0 vulnerabilities reported at that check |
| Source formatting and Git whitespace check | Completed / passed |

These are checks completed during implementation. The complete suite has not yet been rerun after the last small formatting, GPG output-limit, and release-tag validation edits. The dependency audit preceded the final formatter dependency addition. This checkpoint should not be treated as a release certification.

## Remaining verification

- Rerun the final aggregate checks against this exact source snapshot.
- Exercise the new release-tag validation script and release workflow.
- Build and launch native installers and smoke-test the actual Tauri UI on supported platforms.
- Validate real GPG integration, file dialogs, drag-and-drop, cancellation, and platform-specific filesystem behavior in the running desktop app.
- Run the updated GitHub Actions jobs after the changes are submitted.

## Use the checkpoint

`CheckSum-current-source.zip` is the full source tree. Extract it, then follow the setup and development instructions in `README.md`. Generated dependencies, build output, and Git metadata are excluded.

`CheckSum-current-work.patch` contains all changes, including new files, relative to the base commit above. In a clean checkout at that commit, run:

```sh
git apply --check /path/to/CheckSum-current-work.patch
git apply /path/to/CheckSum-current-work.patch
```

Use either the source archive or the patch. Preserve any separate local work before applying the patch.
