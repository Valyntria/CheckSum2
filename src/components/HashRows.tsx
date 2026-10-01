import { IconButton, Stack, TextField } from '@mui/material';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { ALGORITHMS, Hashes, LABELS, Notify } from '../lib/types';
import { errorMessage } from '../lib/utils';

export function HashRows({ hashes, notify }: { hashes: Hashes; notify: Notify }) {
  return (
    <Stack spacing={1}>
      {ALGORITHMS.filter((a) => hashes[a]).map((a) => (
        <Stack direction="row" spacing={1} key={a} alignItems="center">
          <TextField
            fullWidth
            size="small"
            label={LABELS[a]}
            value={hashes[a]}
            InputProps={{ readOnly: true, sx: { fontFamily: 'monospace' } }}
          />
          <IconButton
            aria-label={`Copy ${LABELS[a]}`}
            onClick={() => {
              void navigator.clipboard.writeText(hashes[a] ?? '').then(
                () => notify(`${LABELS[a]} copied`),
                (e) => notify(`Copy failed: ${errorMessage(e)}`),
              );
            }}
          >
            <ContentCopyIcon />
          </IconButton>
        </Stack>
      ))}
    </Stack>
  );
}
