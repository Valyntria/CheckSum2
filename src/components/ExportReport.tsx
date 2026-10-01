import { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { save } from '@tauri-apps/api/dialog';
import { Button, Checkbox, FormControlLabel, MenuItem, Stack, TextField } from '@mui/material';
import { Algorithm, LABELS, Notify, ScanReport } from '../lib/types';
import { errorMessage } from '../lib/utils';

export function ExportReport({
  report,
  notify,
  disabled = false,
}: {
  report: ScanReport | null;
  notify: Notify;
  disabled?: boolean;
}) {
  const [format, setFormat] = useState<'csv' | 'json' | 'manifest'>('csv');
  const [chosenAlgorithm, setChosenAlgorithm] = useState<Algorithm>('sha256');
  const [excludeEmpty, setExcludeEmpty] = useState(true);
  const [saving, setSaving] = useState(false);
  const algorithm = report?.algorithms.includes(chosenAlgorithm)
    ? chosenAlgorithm
    : (report?.algorithms[0] ?? 'sha256');
  const exportReport = async () => {
    if (!report) return;
    setSaving(true);
    try {
      const extension =
        format === 'manifest' ? (algorithm === 'xxhash3' ? 'xxh3' : algorithm) : format;
      const filePath = await save({
        title: 'Save checksum results',
        defaultPath:
          format === 'manifest'
            ? algorithm === 'sha256'
              ? 'SHA256SUMS'
              : `checksums.${extension}`
            : `checksum-report.${extension}`,
        filters: [
          {
            name:
              format === 'manifest'
                ? `${LABELS[algorithm]} checksum manifest`
                : format.toUpperCase(),
            extensions: format === 'manifest' ? ['*'] : [extension],
          },
        ],
      });
      if (!filePath) return;
      await invoke('save_report', {
        filePath,
        data: report,
        format,
        algorithm: format === 'manifest' ? algorithm : null,
        excludeEmpty,
      });
      notify(`Saved to ${filePath}`);
    } catch (e) {
      notify(`Export failed: ${errorMessage(e)}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
        <TextField
          select
          size="small"
          label="Export format"
          value={format}
          onChange={(e) => setFormat(e.target.value as typeof format)}
          sx={{ minWidth: 165 }}
        >
          <MenuItem value="csv">CSV report</MenuItem>
          <MenuItem value="json">JSON report</MenuItem>
          <MenuItem value="manifest">Checksum manifest</MenuItem>
        </TextField>
        {format === 'manifest' && (
          <TextField
            select
            size="small"
            label="Algorithm"
            value={algorithm}
            onChange={(e) => setChosenAlgorithm(e.target.value as Algorithm)}
            sx={{ minWidth: 140 }}
          >
            {(report?.algorithms ?? (['sha256'] as Algorithm[])).map((a) => (
              <MenuItem key={a} value={a}>
                {LABELS[a]}
              </MenuItem>
            ))}
          </TextField>
        )}
        <Button
          variant="outlined"
          disabled={!report || saving || disabled}
          onClick={() => void exportReport()}
        >
          {saving ? 'Saving…' : 'Save report'}
        </Button>
      </Stack>
      {format === 'csv' && (
        <FormControlLabel
          control={
            <Checkbox
              size="small"
              checked={excludeEmpty}
              onChange={(_, checked) => setExcludeEmpty(checked)}
            />
          }
          label="Omit unused algorithm columns"
        />
      )}
    </Stack>
  );
}
