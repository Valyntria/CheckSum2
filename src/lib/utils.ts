import { Algorithm, HASH_LENGTHS, HashResult, ScanReport } from './types';

export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) return String(error.message);
  return String(error);
}
export function isCancelled(error: unknown): boolean {
  return (
    typeof error === 'object' && error !== null && 'kind' in error && error.kind === 'cancelled'
  );
}
export function normalizeHash(value: string, algorithm: Algorithm): string {
  const normalized = value.trim().toLowerCase();
  if (normalized.length !== HASH_LENGTHS[algorithm] || !/^[0-9a-f]+$/.test(normalized))
    throw new Error(`Expected ${HASH_LENGTHS[algorithm]} hexadecimal characters.`);
  return normalized;
}
export function bytesLabel(value: number): string {
  if (value < 1024) return `${value} B`;
  const unit = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** unit).toFixed(1)} ${['B', 'KiB', 'MiB', 'GiB', 'TiB'][unit]}`;
}
export function detectAlgorithm(path: string): Algorithm | null {
  const name = path.split(/[/\\]/).pop()?.toLowerCase() ?? '';
  if (name.includes('sha256')) return 'sha256';
  if (name.includes('sha512')) return 'sha512';
  if (name.includes('sha1')) return 'sha1';
  if (name.includes('md5')) return 'md5';
  if (name.includes('blake3') || name.endsWith('.b3')) return 'blake3';
  if (name.includes('xxh3') || name.includes('xxhash3')) return 'xxhash3';
  return null;
}
export function singleReport(result: HashResult): ScanReport {
  const slash = Math.max(result.path.lastIndexOf('/'), result.path.lastIndexOf('\\'));
  return {
    schema_version: 1,
    root: result.path.slice(0, slash + 1),
    algorithms: Object.keys(result.hashes) as Algorithm[],
    status: 'completed',
    summary: {
      discovered: 1,
      processed: 1,
      matched: 0,
      changed: 0,
      missing: 0,
      new: 0,
      failed: 0,
      skipped: 0,
      bytes_read: result.bytes,
      elapsed_ms: result.elapsed_ms,
    },
    entries: [
      {
        path: result.path,
        relative_path: result.path.slice(slash + 1),
        status: 'hashed',
        hashes: result.hashes,
        expected: null,
        error: null,
        bytes: result.bytes,
      },
    ],
    issues: [],
  };
}
