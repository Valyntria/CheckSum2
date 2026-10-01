import { useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';
import { FileEntry, Notify } from '../lib/types';
import { HashRows } from './HashRows';

export function ResultList({ entries, notify }: { entries: FileEntry[]; notify: Notify }) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 160,
    overscan: 5,
    getItemKey: (i) => entries[i].relative_path,
  });
  return (
    <Box
      ref={parentRef}
      sx={{ height: 480, overflow: 'auto' }}
      role="region"
      aria-label="File results"
      tabIndex={0}
    >
      <Box sx={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}>
        {virtualizer.getVirtualItems().map((item) => {
          const entry = entries[item.index];
          return (
            <Box
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              sx={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${item.start}px)`,
                pb: 1,
              }}
            >
              <Paper variant="outlined" sx={{ p: 1.5 }}>
                <Stack direction="row" spacing={1} alignItems="center" mb={1}>
                  <Typography variant="body2" sx={{ overflowWrap: 'anywhere', flex: 1 }}>
                    {entry.relative_path}
                  </Typography>
                  <Chip
                    size="small"
                    label={entry.status}
                    color={
                      entry.status === 'matched'
                        ? 'success'
                        : ['changed', 'missing', 'error'].includes(entry.status)
                          ? 'error'
                          : 'default'
                    }
                  />
                </Stack>
                {entry.error && (
                  <Typography variant="body2" color="error" mb={1}>
                    {entry.error}
                  </Typography>
                )}
                {entry.status === 'changed' && (
                  <Typography
                    variant="caption"
                    display="block"
                    sx={{ overflowWrap: 'anywhere', mb: 1 }}
                  >
                    Expected: {entry.expected}
                  </Typography>
                )}
                <HashRows hashes={entry.hashes} notify={notify} />
              </Paper>
            </Box>
          );
        })}
      </Box>
    </Box>
  );
}
