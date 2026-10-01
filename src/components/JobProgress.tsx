import { Box, Button, LinearProgress, Stack, Typography } from '@mui/material';
import { HashProgress, ScanProgress } from '../lib/types';
import { bytesLabel } from '../lib/utils';

export function JobProgress({
  progress,
  cancelling,
  cancel,
}: {
  progress: HashProgress | ScanProgress | null;
  cancelling: boolean;
  cancel: () => void;
}) {
  const scanning = progress && 'phase' in progress;
  const discovering = scanning && progress.phase === 'discovering';
  const total = progress?.total_bytes ?? 0;
  const percent =
    total > 0
      ? Math.min(100, (100 * (progress?.bytes_read ?? 0)) / total)
      : scanning && progress.total > 0
        ? (100 * progress.completed) / progress.total
        : 0;
  const seconds = (progress?.elapsed_ms ?? 0) / 1000;
  const rate = seconds > 0 ? (progress?.bytes_read ?? 0) / seconds : 0;
  return (
    <Stack direction="row" spacing={2} alignItems="center" role="status" aria-live="polite">
      <Box flex={1}>
        <LinearProgress
          variant={
            progress && !discovering && (total > 0 || (scanning && progress.total > 0))
              ? 'determinate'
              : 'indeterminate'
          }
          value={percent}
        />
        <Typography variant="caption">
          {discovering
            ? `Discovering files… ${progress.discovered} found`
            : scanning
              ? `${progress.completed} / ${progress.total} files · ${progress.failed} failures`
              : `${Math.round(percent)}%`}
          {progress &&
            !discovering &&
            ` · ${bytesLabel(progress.bytes_read)} / ${bytesLabel(total)} · ${seconds.toFixed(1)} s`}
          {rate > 0 &&
            ` · ${bytesLabel(rate)}/s · ~${Math.ceil(Math.max(0, total - (progress?.bytes_read ?? 0)) / rate)} s left`}
        </Typography>
      </Box>
      <Button color="warning" variant="outlined" disabled={cancelling} onClick={cancel}>
        {cancelling ? 'Cancelling…' : 'Cancel'}
      </Button>
    </Stack>
  );
}
