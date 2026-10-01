import { useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/api/dialog';
import { Alert, Button, MenuItem, Stack, TextField, Typography } from '@mui/material';
import { ALGORITHMS, Algorithm, HashResult, LABELS, Notify, Selection } from '../lib/types';
import { errorMessage, normalizeHash, singleReport } from '../lib/utils';
import { useJob } from '../hooks/useJob';
import { useAlgorithms } from '../hooks/useAlgorithms';
import { AlgorithmPicker } from './AlgorithmPicker';
import { JobProgress } from './JobProgress';
import { HashRows } from './HashRows';
import { ExportReport } from './ExportReport';

export function SingleFileTab({
  selection,
  notify,
}: {
  selection: Selection | null;
  notify: Notify;
}) {
  const [path, setPath] = useState('');
  const [revision, setRevision] = useState(0);
  const [algorithms, setAlgorithms] = useAlgorithms('single-algorithms');
  const [expected, setExpected] = useState('');
  const [verifyAlgorithm, setVerifyAlgorithm] = useState<Algorithm>('sha256');
  const [verification, setVerification] = useState<{
    matches: boolean;
    algorithm: Algorithm;
  } | null>(null);
  const verificationRevision = useRef(0);
  const job = useJob<HashResult>();
  const { run, invalidate } = job;
  const algorithmKey = algorithms.join(',');

  useEffect(() => {
    if (selection) {
      invalidate();
      setVerification(null);
      verificationRevision.current++;
      setPath(selection.path);
      setRevision((r) => r + 1);
    }
  }, [selection, invalidate]);
  useEffect(() => {
    invalidate();
    setVerification(null);
    verificationRevision.current++;
    if (path && algorithmKey)
      void run('calculate_checksums', { path, algorithms: algorithmKey.split(',') });
  }, [path, revision, algorithmKey, run, invalidate]);

  const browse = async () => {
    try {
      const selected = await open({ multiple: false, title: 'Select a file to hash' });
      if (typeof selected === 'string') {
        invalidate();
        setVerification(null);
        setPath(selected);
        setRevision((r) => r + 1);
      }
    } catch (e) {
      notify(errorMessage(e));
    }
  };
  const verify = async () => {
    if (!path) return;
    let normalized: string;
    try {
      normalized = normalizeHash(expected, verifyAlgorithm);
    } catch (e) {
      notify(errorMessage(e));
      return;
    }
    const version = ++verificationRevision.current;
    setVerification(null);
    // Verification always reads the file again, including algorithms not displayed before.
    const result = await run('calculate_checksums', {
      path,
      algorithms: [...new Set([...algorithms, verifyAlgorithm])],
    });
    if (result && verificationRevision.current === version)
      setVerification({
        matches: result.hashes[verifyAlgorithm] === normalized,
        algorithm: verifyAlgorithm,
      });
  };
  const changeExpected = (value: string) => {
    verificationRevision.current++;
    setVerification(null);
    setExpected(value);
  };

  return (
    <Stack spacing={2}>
      <Typography variant="body2" color="text.secondary">
        Choose or drop a file. Reselecting it or pressing Recalculate always reads it again.
      </Typography>
      <Stack direction="row" spacing={1}>
        <TextField
          label="File"
          size="small"
          value={path}
          fullWidth
          InputProps={{ readOnly: true }}
        />
        <Button variant="contained" onClick={() => void browse()}>
          Browse
        </Button>
      </Stack>
      <AlgorithmPicker
        value={algorithms}
        onChange={(value) => {
          invalidate();
          setVerification(null);
          setAlgorithms(value);
        }}
      />
      <Button
        variant="outlined"
        disabled={!path || algorithms.length === 0}
        onClick={() => {
          invalidate();
          setVerification(null);
          setRevision((r) => r + 1);
        }}
      >
        Recalculate
      </Button>
      {job.busy && (
        <JobProgress
          progress={job.progress?.kind === 'hash_progress' ? job.progress.payload : null}
          cancel={job.cancel}
          cancelling={job.status === 'cancelling'}
        />
      )}
      {job.error && <Alert severity="error">{job.error}</Alert>}
      {job.status === 'cancelled' && <Alert severity="info">Hashing cancelled.</Alert>}
      {job.data?.path === path && <HashRows hashes={job.data.hashes} notify={notify} />}
      <Typography variant="subtitle2">Verify against a trusted checksum</Typography>
      <TextField
        select
        size="small"
        label="Verification algorithm"
        value={verifyAlgorithm}
        onChange={(e) => {
          verificationRevision.current++;
          setVerification(null);
          setVerifyAlgorithm(e.target.value as Algorithm);
        }}
      >
        {ALGORITHMS.map((a) => (
          <MenuItem key={a} value={a}>
            {LABELS[a]}
          </MenuItem>
        ))}
      </TextField>
      <TextField
        label="Expected checksum"
        size="small"
        value={expected}
        onChange={(e) => changeExpected(e.target.value)}
        fullWidth
        placeholder="Paste the hexadecimal checksum"
      />
      <Button
        variant="contained"
        disabled={!path || !expected.trim() || job.busy}
        onClick={() => void verify()}
      >
        Verify with a fresh read
      </Button>
      {verification && (
        <Alert severity={verification.matches ? 'success' : 'error'}>
          {LABELS[verification.algorithm]}{' '}
          {verification.matches
            ? 'matches the expected checksum.'
            : 'does not match the expected checksum.'}
        </Alert>
      )}
      <Typography variant="caption" color="text.secondary">
        A matching checksum confirms integrity only relative to the checksum you supplied. Obtain it
        from a trusted source.
      </Typography>
      <ExportReport
        report={job.data?.path === path ? singleReport(job.data) : null}
        notify={notify}
        disabled={job.busy}
      />
    </Stack>
  );
}
