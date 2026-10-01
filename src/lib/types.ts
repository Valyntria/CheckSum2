export const ALGORITHMS = ['md5', 'sha1', 'sha256', 'sha512', 'blake3', 'xxhash3'] as const;
export type Algorithm = (typeof ALGORITHMS)[number];
export const LABELS: Record<Algorithm, string> = {
  md5: 'MD5',
  sha1: 'SHA-1',
  sha256: 'SHA-256',
  sha512: 'SHA-512',
  blake3: 'BLAKE3',
  xxhash3: 'XXH3-64',
};
export const HASH_LENGTHS: Record<Algorithm, number> = {
  md5: 32,
  sha1: 40,
  sha256: 64,
  sha512: 128,
  blake3: 64,
  xxhash3: 16,
};
export type Hashes = Partial<Record<Algorithm, string>>;
export interface HashResult {
  path: string;
  hashes: Hashes;
  bytes: number;
  elapsed_ms: number;
}
export interface HashProgress {
  bytes_read: number;
  total_bytes: number;
  elapsed_ms: number;
}
export type EntryStatus = 'hashed' | 'matched' | 'changed' | 'missing' | 'new' | 'error';
export interface FileEntry {
  path: string;
  relative_path: string;
  status: EntryStatus;
  hashes: Hashes;
  expected: string | null;
  error: string | null;
  bytes: number;
}
export interface ScanSummary {
  discovered: number;
  processed: number;
  matched: number;
  changed: number;
  missing: number;
  new: number;
  failed: number;
  skipped: number;
  bytes_read: number;
  elapsed_ms: number;
}
export interface ScanReport {
  schema_version: number;
  root: string;
  algorithms: Algorithm[];
  status: 'completed' | 'partial' | 'cancelled';
  summary: ScanSummary;
  entries: FileEntry[];
  issues: { path: string; message: string }[];
}
export interface ScanOptions {
  recursive: boolean;
  include_hidden: boolean;
  concurrency: number;
}
export interface ScanProgress extends HashProgress {
  phase: 'discovering' | 'hashing';
  discovered: number;
  completed: number;
  total: number;
  failed: number;
}
export type JobEvent = { job_id: string } & (
  | { kind: 'hash_progress'; payload: HashProgress }
  | { kind: 'scan_progress'; payload: ScanProgress }
  | { kind: 'entry'; payload: FileEntry }
);
export interface Signature {
  valid: boolean;
  signer: string | null;
  signing_fingerprint: string | null;
  primary_fingerprint: string | null;
  trust: string | null;
  problems: string[];
}
export interface GpgResult {
  overall_success: boolean;
  exit_code: number | null;
  fingerprint_match: boolean | null;
  signatures: Signature[];
  message: string;
  diagnostics: string;
}
export interface Selection {
  path: string;
  revision: number;
}
export type Notify = (message: string) => void;
