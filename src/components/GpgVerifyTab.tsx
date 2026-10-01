import { useState } from 'react';
import { open } from '@tauri-apps/api/dialog';
import { Alert, Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { GpgResult, Notify } from '../lib/types';
import { errorMessage } from '../lib/utils';
import { useJob } from '../hooks/useJob';

export function GpgVerifyTab({ notify }: { notify: Notify }) {
  const [file, setFile] = useState('');
  const [signature, setSignature] = useState('');
  const [fingerprint, setFingerprint] = useState('');
  const job = useJob<GpgResult>();
  const choose = async (which: 'file' | 'signature') => {
    try {
      const selected = await open({
        multiple: false,
        title: which === 'file' ? 'Select the signed file' : 'Select its detached signature',
      });
      if (typeof selected === 'string') {
        job.invalidate();
        if (which === 'file') setFile(selected);
        else setSignature(selected);
      }
    } catch (e) {
      notify(errorMessage(e));
    }
  };
  return (
    <Stack spacing={2}>
      <Typography variant="body2" color="text.secondary">
        Uses your installed GnuPG and local public keyring. Automatic key retrieval is disabled.
      </Typography>
      <Stack direction="row" spacing={1}>
        <TextField
          size="small"
          label="Signed file"
          value={file}
          fullWidth
          InputProps={{ readOnly: true }}
        />
        <Button onClick={() => void choose('file')} variant="contained">
          Select file
        </Button>
      </Stack>
      <Stack direction="row" spacing={1}>
        <TextField
          size="small"
          label="Detached signature"
          value={signature}
          fullWidth
          InputProps={{ readOnly: true }}
        />
        <Button onClick={() => void choose('signature')} variant="contained">
          Select signature
        </Button>
      </Stack>
      <TextField
        size="small"
        label="Expected primary or signing key fingerprint (optional)"
        value={fingerprint}
        onChange={(e) => {
          job.invalidate();
          setFingerprint(e.target.value);
        }}
        helperText="Use the complete fingerprint from an independently trusted source."
      />
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained"
          disabled={!file || !signature || job.busy}
          onClick={() =>
            void job.run('verify_gpg_signature', {
              file_path: file,
              signature_path: signature,
              expected_fingerprint: fingerprint.trim() || null,
            })
          }
        >
          {job.busy ? 'Verifying…' : 'Verify GPG signature'}
        </Button>
        {job.busy && (
          <Button color="warning" disabled={job.status === 'cancelling'} onClick={job.cancel}>
            {job.status === 'cancelling' ? 'Cancelling…' : 'Cancel'}
          </Button>
        )}
      </Stack>
      {job.error && <Alert severity="error">{job.error}</Alert>}
      {job.status === 'cancelled' && <Alert severity="info">GPG verification cancelled.</Alert>}
      {job.data && (
        <Stack spacing={1}>
          <Alert
            severity={
              !job.data.overall_success
                ? 'error'
                : job.data.fingerprint_match === true
                  ? 'success'
                  : 'warning'
            }
          >
            {job.data.message}
          </Alert>
          <Typography variant="body2">
            Fingerprint:{' '}
            {job.data.fingerprint_match === null
              ? 'Not checked'
              : job.data.fingerprint_match
                ? 'Matches a valid signer'
                : 'Does not match'}
          </Typography>
          {job.data.signatures.map((sig, i) => (
            <Paper key={i} variant="outlined" sx={{ p: 2, overflowWrap: 'anywhere' }}>
              <Typography variant="subtitle2">
                Signature {i + 1}: {sig.valid ? 'Cryptographically valid' : 'Invalid or incomplete'}
              </Typography>
              <Typography variant="body2">
                Identity claimed by key: {sig.signer ?? 'Unavailable'}
              </Typography>
              <Typography variant="body2">
                Signing key: {sig.signing_fingerprint ?? 'Unavailable'}
              </Typography>
              <Typography variant="body2">
                Primary key: {sig.primary_fingerprint ?? 'Unavailable'}
              </Typography>
              <Typography variant="body2">
                Local GPG trust: {sig.trust ?? 'Not reported'}
              </Typography>
              {sig.problems.length > 0 && (
                <Typography color="error">{sig.problems.join(', ')}</Typography>
              )}
            </Paper>
          ))}
          <details>
            <summary>GPG diagnostics (exit code {job.data.exit_code ?? 'unavailable'})</summary>
            <pre className="diagnostics">{job.data.diagnostics}</pre>
          </details>
        </Stack>
      )}
    </Stack>
  );
}
