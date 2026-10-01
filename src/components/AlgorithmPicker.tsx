import { Checkbox, FormControlLabel, Stack, Typography } from '@mui/material';
import { ALGORITHMS, Algorithm, LABELS } from '../lib/types';

export function AlgorithmPicker({
  value,
  onChange,
}: {
  value: Algorithm[];
  onChange: (value: Algorithm[]) => void;
}) {
  return (
    <Stack spacing={0.5}>
      <Typography variant="subtitle2">Algorithms</Typography>
      <Stack direction="row" flexWrap="wrap">
        {ALGORITHMS.map((a) => (
          <FormControlLabel
            key={a}
            label={LABELS[a]}
            control={
              <Checkbox
                size="small"
                checked={value.includes(a)}
                onChange={(_, checked) =>
                  onChange(
                    ALGORITHMS.filter((item) => (item === a ? checked : value.includes(item))),
                  )
                }
              />
            }
          />
        ))}
      </Stack>
      <Typography variant="caption" color="text.secondary">
        SHA-256 is the default. MD5/SHA-1 are for compatibility; XXH3-64 is noncryptographic.
      </Typography>
    </Stack>
  );
}
