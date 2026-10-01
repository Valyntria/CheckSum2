import { useCallback, useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/api/dialog';
import {
  Alert,
  Button,
  Checkbox,
  FormControlLabel,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import {
  ALGORITHMS,
  Algorithm,
  FileEntry,
  JobEvent,
  LABELS,
  Notify,
  ScanOptions,
  ScanReport,
  Selection,
} from '../lib/types';
import { detectAlgorithm, errorMessage } from '../lib/utils';
import { useJob } from '../hooks/useJob';
import { useAlgorithms } from '../hooks/useAlgorithms';
import { AlgorithmPicker } from './AlgorithmPicker';
import { ExportReport } from './ExportReport';
import { JobProgress } from './JobProgress';
import { ResultList } from './ResultList';

export function FolderScanTab({
  selection,
  notify,
}: {
  selection: Selection | null;
  notify: Notify;
}) {
  const [root, setRoot] = useState('');
  const [manifestPath, setManifestPath] = useState('');
  const [mode, setMode] = useState<'scan' | 'verify'>('scan');
  const [manifestAlgorithm, setManifestAlgorithm] = useState<Algorithm>('sha256');
  const [algorithms, setAlgorithms] = useAlgorithms('folder-algorithms');
  const [options, setOptions] = useState<ScanOptions>({
    recursive: true,
    include_hidden: false,
    concurrency: 2,
  });
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const queue = useRef<FileEntry[]>([]);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const stopQueue = useCallback(() => {
    clearTimeout(timer.current);
    timer.current = undefined;
    queue.current = [];
  }, []);
  const onEvent = useCallback((event: JobEvent) => {
    if (event.kind !== 'entry') return;
    queue.current.push(event.payload);
    if (timer.current === undefined)
      timer.current = setTimeout(() => {
        const batch = queue.current;
        queue.current = [];
        timer.current = undefined;
        setEntries((current) => current.concat(batch));
      }, 100);
  }, []);
  const job = useJob<ScanReport>(onEvent);
  const { invalidate } = job;
  const clear = useCallback(() => {
    invalidate();
    stopQueue();
    setEntries([]);
  }, [invalidate, stopQueue]);
  useEffect(() => stopQueue, [stopQueue]);
  useEffect(() => {
    if (selection) {
      clear();
      setRoot(selection.path);
    }
  }, [selection, clear]);
  useEffect(() => {
    if (job.data) {
      stopQueue();
      setEntries(job.data.entries);
    }
  }, [job.data, stopQueue]);

  const browse = async (kind: 'folder' | 'manifest') => {
    try {
      const selected = await open({
        directory: kind === 'folder',
        multiple: false,
        title:
          kind === 'folder' ? 'Choose the folder to scan or verify' : 'Choose a checksum manifest',
      });
      if (typeof selected !== 'string') return;
      clear();
      if (kind === 'folder') setRoot(selected);
      else {
        setManifestPath(selected);
        const detected = detectAlgorithm(selected);
        if (detected) setManifestAlgorithm(detected);
        if (!root)
          setRoot(
            selected.slice(0, Math.max(selected.lastIndexOf('/'), selected.lastIndexOf('\\')) + 1),
          );
      }
    } catch (e) {
      notify(errorMessage(e));
    }
  };
  const start = () => {
    clear();
    void job.run('scan_folder', {
      root,
      algorithms: mode === 'verify' ? [manifestAlgorithm] : algorithms,
      options,
      manifest_path: mode === 'verify' ? manifestPath : null,
    });
  };
  const report = job.data;
  const summary = report?.summary;
  const differences = summary
    ? summary.changed + summary.missing + summary.failed + summary.new
    : 0;
  return (
    <Stack spacing={2}>
      <TextField
        select
        size="small"
        label="Operation"
        value={mode}
        onChange={(e) => {
          clear();
          setMode(e.target.value as typeof mode);
        }}
      >
        <MenuItem value="scan">Calculate folder checksums</MenuItem>
        <MenuItem value="verify">Verify a checksum manifest</MenuItem>
      </TextField>
      <Stack direction="row" spacing={1}>
        <TextField
          label="Folder"
          size="small"
          value={root}
          fullWidth
          InputProps={{ readOnly: true }}
        />
        <Button variant="contained" onClick={() => void browse('folder')}>
          Browse
        </Button>
      </Stack>
      {mode === 'scan' ? (
        <AlgorithmPicker
          value={algorithms}
          onChange={(value) => {
            clear();
            setAlgorithms(value);
          }}
        />
      ) : (
        <>
          <Stack direction="row" spacing={1}>
            <TextField
              label="Checksum manifest"
              size="small"
              value={manifestPath}
              fullWidth
              InputProps={{ readOnly: true }}
            />
            <Button variant="outlined" onClick={() => void browse('manifest')}>
              Choose
            </Button>
          </Stack>
          <TextField
            select
            label="Manifest algorithm"
            size="small"
            value={manifestAlgorithm}
            onChange={(e) => {
              clear();
              setManifestAlgorithm(e.target.value as Algorithm);
            }}
          >
            {ALGORITHMS.map((a) => (
              <MenuItem value={a} key={a}>
                {LABELS[a]}
              </MenuItem>
            ))}
          </TextField>
          <Typography variant="caption">
            Paths in the manifest are relative to the selected folder. SHA-256 and BLAKE3 have the
            same hash length; confirm the algorithm.
          </Typography>
        </>
      )}
      <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap alignItems="center">
        <FormControlLabel
          label="Include subfolders"
          control={
            <Checkbox
              checked={options.recursive}
              onChange={(_, checked) => {
                clear();
                setOptions({ ...options, recursive: checked });
              }}
            />
          }
        />
        <FormControlLabel
          label="Include hidden files and folders"
          control={
            <Checkbox
              checked={options.include_hidden}
              onChange={(_, checked) => {
                clear();
                setOptions({ ...options, include_hidden: checked });
              }}
            />
          }
        />
        <TextField
          select
          label="Parallel files"
          size="small"
          sx={{ minWidth: 130 }}
          value={options.concurrency}
          onChange={(e) => {
            clear();
            setOptions({ ...options, concurrency: Number(e.target.value) });
          }}
        >
          {[1, 2, 4, 8].map((n) => (
            <MenuItem value={n} key={n}>
              {n}
            </MenuItem>
          ))}
        </TextField>
      </Stack>
      <Typography variant="caption" color="text.secondary">
        Symbolic links are skipped. Manifest-listed files are checked even when excluded from
        discovery. Use fewer parallel files for slower drives.
      </Typography>
      <Button
        variant="contained"
        disabled={job.busy || !root || (mode === 'scan' ? !algorithms.length : !manifestPath)}
        onClick={start}
      >
        {mode === 'scan' ? 'Scan folder' : 'Verify manifest'}
      </Button>
      {job.busy && (
        <JobProgress
          progress={job.progress?.kind === 'scan_progress' ? job.progress.payload : null}
          cancel={job.cancel}
          cancelling={job.status === 'cancelling'}
        />
      )}
      {job.error && <Alert severity="error">{job.error}</Alert>}
      {job.status === 'cancelled' && !report && (
        <Alert severity="warning">Cancelled before results were available.</Alert>
      )}
      {report && summary && (
        <Alert
          severity={report.status === 'completed' && differences === 0 ? 'success' : 'warning'}
        >
          Scan {report.status}. {summary.processed} / {summary.discovered} files processed;{' '}
          {summary.failed} errors; {summary.skipped} excluded entries.
          {mode === 'verify' &&
            ` ${summary.matched} matched, ${summary.changed} changed, ${summary.missing} missing, ${summary.new} additional files.`}
        </Alert>
      )}
      {report && report.issues.length > 0 && (
        <details>
          <summary>{report.issues.length} discovery or worker errors</summary>
          <ul>
            {report.issues.map((issue, i) => (
              <li key={i}>
                {issue.path}: {issue.message}
              </li>
            ))}
          </ul>
        </details>
      )}
      {entries.length > 0 && (
        <>
          <Typography variant="subtitle2">Results ({entries.length})</Typography>
          <ResultList entries={entries} notify={notify} />
        </>
      )}
      <ExportReport report={report} notify={notify} disabled={job.busy} />
    </Stack>
  );
}
