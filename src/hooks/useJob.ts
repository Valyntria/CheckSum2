import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { JobEvent } from '../lib/types';
import { errorMessage, isCancelled } from '../lib/utils';

type Command = 'calculate_checksums' | 'scan_folder' | 'verify_gpg_signature';
interface Active {
  id?: string;
  unlisten?: UnlistenFn;
  invalid: boolean;
  cancelled: boolean;
}
const stopListening = (active: Active) => {
  active.unlisten?.();
  active.unlisten = undefined;
};
const cancelNative = (id: string) => invoke('cancel_job', { jobId: id }).catch(() => undefined);

export function useJob<T>(onEvent?: (event: JobEvent) => void) {
  const activeRef = useRef<Active | null>(null);
  const eventRef = useRef(onEvent);
  eventRef.current = onEvent;
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<
    'idle' | 'running' | 'cancelling' | 'completed' | 'cancelled' | 'error'
  >('idle');
  const [progress, setProgress] = useState<Exclude<JobEvent, { kind: 'entry' }> | null>(null);

  const dispose = useCallback(() => {
    const active = activeRef.current;
    activeRef.current = null;
    if (active) {
      active.invalid = true;
      stopListening(active);
      if (active.id) void cancelNative(active.id);
    }
  }, []);
  const invalidate = useCallback(() => {
    dispose();
    setData(null);
    setError(null);
    setProgress(null);
    setStatus('idle');
  }, [dispose]);
  useEffect(() => dispose, [dispose]);

  const cancel = useCallback(() => {
    const active = activeRef.current;
    if (active) {
      active.cancelled = true;
      setStatus('cancelling');
      if (active.id) void cancelNative(active.id);
    }
  }, []);

  const run = useCallback(
    async (command: Command, request: object): Promise<T | undefined> => {
      dispose();
      const active: Active = { invalid: false, cancelled: false };
      activeRef.current = active;
      setData(null);
      setError(null);
      setProgress(null);
      setStatus('running');
      try {
        // Register before starting native work; fast files must not outrun the listener.
        active.unlisten = await listen<JobEvent>('job-event', ({ payload }) => {
          if (activeRef.current !== active || active.invalid || payload.job_id !== active.id)
            return;
          if (payload.kind !== 'entry') setProgress(payload);
          eventRef.current?.(payload);
        });
        if (active.invalid) return;
        if (active.cancelled) {
          setStatus('cancelled');
          return;
        }
        active.id = await invoke<string>('create_job');
        if (active.invalid || active.cancelled) {
          await cancelNative(active.id);
          if (!active.invalid) setStatus('cancelled');
          return;
        }
        const result = await invoke<T>(command, { jobId: active.id, request });
        if (activeRef.current !== active || active.invalid) return;
        if (active.cancelled && command !== 'scan_folder') {
          setStatus('cancelled');
          return;
        }
        setData(result);
        setStatus(active.cancelled ? 'cancelled' : 'completed');
        return result;
      } catch (cause) {
        if (activeRef.current !== active || active.invalid) return;
        if (isCancelled(cause)) setStatus('cancelled');
        else {
          setError(errorMessage(cause));
          setStatus('error');
        }
      } finally {
        stopListening(active);
        // Idempotent cleanup also handles errors before the worker took ownership.
        if (active.id) void cancelNative(active.id);
        if (activeRef.current === active) {
          activeRef.current = null;
          setProgress(null);
        }
      }
    },
    [dispose],
  );

  return {
    data,
    error,
    status,
    progress,
    run,
    cancel,
    invalidate,
    busy: status === 'running' || status === 'cancelling',
  };
}
