# Verification and manifest improvements

This change addresses the review of commit `7b4618f`.

| Review item | Implementation |
| --- | --- |
| Stale file cache | Removed the path-only cache. Reselect, Recalculate, and Verify all read file contents again. |
| Late results overwrite a newer selection | `useJob` ignores obsolete results and events; every native job has a separate identity. |
| Stale GPG verification | Changing any verification input invalidates the result and cancels obsolete work. |
| GPG exit status ignored | The backend returns one overall outcome derived from process exit, every signature, and any expected fingerprint. |
| Partial scans presented as complete | Reports carry completion state, counts, per-file outcomes, and discovery errors. Cancellation preserves partial results. |
| Unsafe mmap and separate BLAKE3 pass | All algorithms share one buffered streaming pass. Metadata changes are detected; no file-backed memory maps remain in application hashing. |
| Incomplete hash verification | All six algorithms can be verified, with explicit algorithm selection and normalized hex input. |
| XXH3 padding | XXH3-64 is always 16 hexadecimal characters. |
| Global cancellation | Independent tokens, cancellable BLAKE3 chunks, GPG process termination, and race-safe job registration. |
| Event listener cleanup | Listener registration and cleanup belong to each job; late registration after unmount is cleaned up. |
| Folder traversal | Hidden folders are excluded along with files, Windows hidden attributes are recognized, symlinks are skipped, and non-Unicode names are reported. |
| Release checksum paths and duplicates | Assets are staged under unique final filenames before one sorted SHA256 manifest is generated. |
| Primary/subkey fingerprints | Both full fingerprints are parsed and accepted; an omitted expected fingerprint is explicitly marked unchecked. |
| GPG network retrieval | Invocation disables config-file options and automatic retrieval, key location, and embedded-key import. |
| Manifest support | GNU-style checksum import/export, path validation, escaped filenames, and folder comparison outcomes. |
| Defaults and descriptions | SHA-256 for new preferences; legacy and noncryptographic algorithms are labeled. |
| Regression coverage | Core, frontend, and release-manifest tests run in CI; native CI covers Linux, macOS, and Windows. |
| Maintainability | Typed components, job hooks, and a standalone Rust core crate. |
| Large-folder behavior | Throttled streaming events, virtualized results, configurable concurrency, throughput, elapsed time, and ETA. |
| Documentation and updates | README, weekly dependency checks, refreshed lockfiles, and documented Wry patch. |

Additional protections include formula-safe CSV cells, atomic report writes, and rejecting an export that would overwrite a hashed input file.

## Verification limits

- Metadata comparisons detect ordinary modification or replacement, but do not create an atomic filesystem snapshot. Stop writers or use a filesystem snapshot for a consistent backup baseline.
- The scanner deliberately excludes symbolic links. Non-Unicode paths are visible errors instead of crashes.
- Checksums authenticate content only relative to a trusted baseline. GPG identities require an independently trusted full fingerprint or a separately established trust policy.
- A completed manifest comparison can contain mismatches or missing files; the outcome counts remain explicit. Manifest export refuses cancelled, partial, mismatched, missing, or empty results.
- GPG must be installed separately. Key import is managed outside the app; no key download is initiated by verification.
- File reads on an unresponsive device may delay cooperative cancellation until the operating system returns from the current read.
