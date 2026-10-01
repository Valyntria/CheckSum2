import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useJob } from '../src/hooks/useJob';
import { JobEvent } from '../src/lib/types';

const api = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/tauri', () => ({ invoke: api.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: api.listen }));
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
let active: Set<(event: { payload: JobEvent }) => void>;
let jobs: Map<string, ReturnType<typeof deferred<string>>>;
let next: number;
beforeEach(() => {
  vi.resetAllMocks();
  active = new Set();
  jobs = new Map();
  next = 0;
  api.listen.mockImplementation(
    async (_: string, listener: (event: { payload: JobEvent }) => void) => {
      active.add(listener);
      return () => active.delete(listener);
    },
  );
  api.invoke.mockImplementation(async (command: string, args?: { jobId: string }) => {
    if (command === 'create_job') return `job-${++next}`;
    if (command === 'cancel_job') return;
    const d = deferred<string>();
    jobs.set(args!.jobId, d);
    return d.promise;
  });
});

describe('independent native jobs', () => {
  it('ignores late results and progress from a replaced job', async () => {
    const { result } = renderHook(() => useJob<string>());
    let first!: Promise<string | undefined>;
    let second!: Promise<string | undefined>;
    act(() => {
      first = result.current.run('calculate_checksums', { path: 'A' });
    });
    await waitFor(() => expect(jobs.has('job-1')).toBe(true));
    act(() => {
      second = result.current.run('calculate_checksums', { path: 'B' });
    });
    await waitFor(() => expect(jobs.has('job-2')).toBe(true));
    act(() => {
      for (const listener of active)
        listener({
          payload: {
            job_id: 'job-1',
            kind: 'hash_progress',
            payload: { bytes_read: 9, total_bytes: 10, elapsed_ms: 1 },
          },
        });
    });
    expect(result.current.progress).toBeNull();
    await act(async () => {
      jobs.get('job-2')!.resolve('B');
      await second;
    });
    await act(async () => {
      jobs.get('job-1')!.resolve('A');
      expect(await first).toBeUndefined();
    });
    expect(result.current.data).toBe('B');
    expect(active.size).toBe(0);
  });

  it('cleans up a listener whose registration completes after unmount', async () => {
    const registration = deferred<() => void>();
    const stop = vi.fn();
    api.listen.mockReturnValue(registration.promise);
    const { result, unmount } = renderHook(() => useJob());
    let running!: Promise<unknown>;
    act(() => {
      running = result.current.run('calculate_checksums', {});
    });
    unmount();
    await act(async () => {
      registration.resolve(stop);
      await running;
    });
    expect(stop).toHaveBeenCalledOnce();
    expect(api.invoke).not.toHaveBeenCalledWith('create_job');
  });

  it('cancels a job created after cancellation was requested', async () => {
    const creation = deferred<string>();
    api.invoke.mockImplementation(async (command) =>
      command === 'create_job' ? creation.promise : undefined,
    );
    const { result } = renderHook(() => useJob());
    let running!: Promise<unknown>;
    act(() => {
      running = result.current.run('calculate_checksums', {});
    });
    await waitFor(() => expect(api.invoke).toHaveBeenCalledWith('create_job'));
    act(() => result.current.cancel());
    await act(async () => {
      creation.resolve('late');
      await running;
    });
    expect(api.invoke).toHaveBeenCalledWith('cancel_job', { jobId: 'late' });
    expect(api.invoke.mock.calls.some(([command]) => command === 'calculate_checksums')).toBe(
      false,
    );
    expect(result.current.status).toBe('cancelled');
    expect(active.size).toBe(0);
  });

  it('cancels only the selected hook instance', async () => {
    const a = renderHook(() => useJob<string>());
    const b = renderHook(() => useJob<string>());
    let pa!: Promise<unknown>;
    let pb!: Promise<unknown>;
    act(() => {
      pa = a.result.current.run('calculate_checksums', {});
      pb = b.result.current.run('calculate_checksums', {});
    });
    await waitFor(() => expect(jobs.size).toBe(2));
    api.invoke.mockClear();
    act(() => a.result.current.cancel());
    expect(api.invoke).toHaveBeenCalledWith('cancel_job', { jobId: 'job-1' });
    expect(api.invoke).not.toHaveBeenCalledWith('cancel_job', { jobId: 'job-2' });
    await act(async () => {
      jobs.get('job-1')!.resolve('A');
      jobs.get('job-2')!.resolve('B');
      await Promise.all([pa, pb]);
    });
    expect(a.result.current.data).toBeNull();
    expect(b.result.current.data).toBe('B');
  });

  it('keeps partial scan results after cooperative cancellation', async () => {
    const { result } = renderHook(() => useJob<string>());
    let pending!: Promise<unknown>;
    act(() => {
      pending = result.current.run('scan_folder', {});
    });
    await waitFor(() => expect(jobs.size).toBe(1));
    act(() => result.current.cancel());
    await act(async () => {
      jobs.get('job-1')!.resolve('partial report');
      await pending;
    });
    expect(result.current.data).toBe('partial report');
    expect(result.current.status).toBe('cancelled');
  });
});
