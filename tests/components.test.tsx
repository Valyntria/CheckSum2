import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { SingleFileTab } from '../src/components/SingleFileTab';
import { GpgVerifyTab } from '../src/components/GpgVerifyTab';
import { readAlgorithms } from '../src/hooks/useAlgorithms';
import { ALGORITHMS, HASH_LENGTHS, Algorithm } from '../src/lib/types';
import { normalizeHash } from '../src/lib/utils';

const api = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/tauri', () => ({ invoke: api.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: api.listen }));
vi.mock('@tauri-apps/api/dialog', () => ({ open: api.open, save: vi.fn() }));
let hashesRun: number;
beforeEach(() => {
  vi.resetAllMocks();
  hashesRun = 0;
  let jobs = 0;
  api.listen.mockResolvedValue(() => {});
  api.invoke.mockImplementation(async (command, args) => {
    if (command === 'create_job') return `job-${++jobs}`;
    if (command === 'calculate_checksums') {
      hashesRun++;
      return {
        path: args.request.path,
        hashes: Object.fromEntries(
          args.request.algorithms.map((a: Algorithm) => [
            a,
            (hashesRun === 1 ? 'a' : 'b').repeat(HASH_LENGTHS[a]),
          ]),
        ),
        bytes: 3,
        elapsed_ms: 1,
      };
    }
  });
});

it('rereads the same path on drop and on verification instead of reusing a cached checksum', async () => {
  const props = { notify: vi.fn(), selection: { path: '/example', revision: 1 } };
  const { rerender } = render(<SingleFileTab {...props} />);
  await waitFor(() =>
    expect(screen.getByLabelText('SHA-256', { selector: 'input[type="text"]' })).toHaveValue(
      'a'.repeat(64),
    ),
  );
  rerender(<SingleFileTab {...props} selection={{ path: '/example', revision: 2 }} />);
  await waitFor(() => expect(hashesRun).toBe(2));
  fireEvent.change(screen.getByLabelText('Expected checksum'), {
    target: { value: ' B'.padEnd(66, 'B') + ' ' },
  });
  fireEvent.change(screen.getByLabelText('Expected checksum'), {
    target: { value: ` ${'B'.repeat(64)}\n` },
  });
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Verify with a fresh read' })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole('button', { name: 'Verify with a fresh read' }));
  await screen.findByText('SHA-256 matches the expected checksum.');
  expect(hashesRun).toBe(3);
});

it('verification includes BLAKE3 and XXH3-64', async () => {
  localStorage.setItem('single-algorithms', JSON.stringify(['blake3', 'xxhash3']));
  render(<SingleFileTab selection={{ path: '/example', revision: 1 }} notify={vi.fn()} />);
  await waitFor(() => expect(hashesRun).toBe(1));
  for (const algorithm of ['blake3', 'xxhash3'] as const) {
    fireEvent.mouseDown(screen.getByLabelText('Verification algorithm'));
    fireEvent.click(
      screen.getByRole('option', { name: algorithm === 'blake3' ? 'BLAKE3' : 'XXH3-64' }),
    );
    fireEvent.change(screen.getByLabelText('Expected checksum'), {
      target: { value: 'b'.repeat(HASH_LENGTHS[algorithm]) },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Verify with a fresh read' }));
    await screen.findByText(
      `${algorithm === 'blake3' ? 'BLAKE3' : 'XXH3-64'} matches the expected checksum.`,
    );
  }
});

it('GPG results are invalidated by selecting another file or editing the fingerprint', async () => {
  const original = api.invoke.getMockImplementation()!;
  api.invoke.mockImplementation(async (command, args) =>
    command === 'verify_gpg_signature'
      ? {
          overall_success: true,
          fingerprint_match: true,
          exit_code: 0,
          signatures: [],
          diagnostics: '',
          message: 'Verified fixture',
        }
      : original(command, args),
  );
  api.open
    .mockResolvedValueOnce('/signed')
    .mockResolvedValueOnce('/signature')
    .mockResolvedValueOnce('/different');
  render(<GpgVerifyTab notify={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: 'Select file' }));
  await waitFor(() => expect(screen.getByLabelText('Signed file')).toHaveValue('/signed'));
  fireEvent.click(screen.getByRole('button', { name: 'Select signature' }));
  await waitFor(() =>
    expect(screen.getByLabelText('Detached signature')).toHaveValue('/signature'),
  );
  fireEvent.click(screen.getByRole('button', { name: 'Verify GPG signature' }));
  await screen.findByText('Verified fixture');
  fireEvent.click(screen.getByRole('button', { name: 'Select file' }));
  await waitFor(() => expect(screen.queryByText('Verified fixture')).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole('button', { name: 'Verify GPG signature' }));
  await screen.findByText('Verified fixture');
  fireEvent.change(
    screen.getByLabelText('Expected primary or signing key fingerprint (optional)'),
    { target: { value: 'A'.repeat(40) } },
  );
  expect(screen.queryByText('Verified fixture')).not.toBeInTheDocument();
});

it('uses the overall GPG outcome and distinguishes unchecked identity', async () => {
  const original = api.invoke.getMockImplementation()!;
  api.invoke.mockImplementation(async (command, args) =>
    command === 'verify_gpg_signature'
      ? {
          overall_success: false,
          fingerprint_match: null,
          exit_code: 1,
          signatures: [],
          diagnostics: '',
          message: 'Verification failed',
        }
      : original(command, args),
  );
  api.open.mockResolvedValue('/fixture');
  render(<GpgVerifyTab notify={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: 'Select file' }));
  fireEvent.click(screen.getByRole('button', { name: 'Select signature' }));
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Verify GPG signature' })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole('button', { name: 'Verify GPG signature' }));
  const alert = await screen.findByRole('alert');
  expect(alert).toHaveClass('MuiAlert-standardError');
  expect(screen.getByText('Fingerprint: Not checked')).toBeInTheDocument();
});

it('recovers from malformed preferences and validates every supported hash format', () => {
  localStorage.setItem('single-algorithms', '{broken');
  expect(readAlgorithms('single-algorithms')).toEqual(['sha256']);
  for (const a of ALGORITHMS)
    expect(normalizeHash(` ${'A'.repeat(HASH_LENGTHS[a])}\n`, a)).toBe('a'.repeat(HASH_LENGTHS[a]));
  expect(() => normalizeHash('xyz', 'sha256')).toThrow();
});
