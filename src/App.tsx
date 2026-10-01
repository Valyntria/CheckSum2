import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/tauri';
import {
  Alert,
  Box,
  Button,
  Container,
  CssBaseline,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Snackbar,
  Stack,
  Tab,
  Tabs,
  ThemeProvider,
  Typography,
  createTheme,
} from '@mui/material';
import Brightness4Icon from '@mui/icons-material/Brightness4';
import Brightness7Icon from '@mui/icons-material/Brightness7';
import { SingleFileTab } from './components/SingleFileTab';
import { FolderScanTab } from './components/FolderScanTab';
import { GpgVerifyTab } from './components/GpgVerifyTab';
import { Selection } from './lib/types';
import { errorMessage } from './lib/utils';

export default function App() {
  const [dark, setDark] = useState(() => {
    try {
      return localStorage.getItem('theme-mode') !== 'light';
    } catch {
      return true;
    }
  });
  const theme = useMemo(() => createTheme({ palette: { mode: dark ? 'dark' : 'light' } }), [dark]);
  const [tab, setTab] = useState(0);
  const [about, setAbout] = useState(false);
  const [version, setVersion] = useState('');
  const [message, setMessage] = useState('');
  const notify = useCallback((text: string) => setMessage(text), []);
  const [fileSelection, setFileSelection] = useState<Selection | null>(null);
  const [folderSelection, setFolderSelection] = useState<Selection | null>(null);
  const dropRevision = useRef(0);
  useEffect(() => {
    try {
      localStorage.setItem('theme-mode', dark ? 'dark' : 'light');
    } catch {
      /* Optional persistence. */
    }
  }, [dark]);
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<string[]>('tauri://file-drop', (event) => {
      const path = event.payload[0];
      if (!path) return;
      const revision = ++dropRevision.current;
      void invoke<string>('path_kind', { path })
        .then((kind) => {
          if (disposed || revision !== dropRevision.current) return;
          if (kind === 'file') {
            setFileSelection({ path, revision });
            setTab(0);
          } else if (kind === 'directory') {
            setFolderSelection({ path, revision });
            setTab(1);
          } else notify('Select a regular file or folder.');
          if (event.payload.length > 1)
            notify('Opened the first dropped item. Use Folder Scan to process many files.');
        })
        .catch((e) => {
          if (!disposed) notify(errorMessage(e));
        });
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((e) => {
        if (!disposed) notify(errorMessage(e));
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [notify]);

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <Container maxWidth="md" sx={{ py: 2 }}>
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
          <Typography variant="h6">CheckSum</Typography>
          <Box>
            <IconButton
              aria-label={dark ? 'Use light theme' : 'Use dark theme'}
              onClick={() => setDark(!dark)}
            >
              {dark ? <Brightness7Icon /> : <Brightness4Icon />}
            </IconButton>
            <Button onClick={() => setAbout(true)}>About</Button>
          </Box>
        </Stack>
        <Tabs
          value={tab}
          onChange={(_, value: number) => setTab(value)}
          variant="scrollable"
          scrollButtons="auto"
          aria-label="Checksum tools"
          sx={{ mb: 2, borderBottom: 1, borderColor: 'divider' }}
        >
          {['Single File', 'Folder / Manifest', 'GPG Verify'].map((label, i) => (
            <Tab label={label} id={`tool-tab-${i}`} aria-controls={`tool-panel-${i}`} key={label} />
          ))}
        </Tabs>
        <Box role="tabpanel" id="tool-panel-0" aria-labelledby="tool-tab-0" hidden={tab !== 0}>
          <SingleFileTab selection={fileSelection} notify={notify} />
        </Box>
        <Box role="tabpanel" id="tool-panel-1" aria-labelledby="tool-tab-1" hidden={tab !== 1}>
          <FolderScanTab selection={folderSelection} notify={notify} />
        </Box>
        <Box role="tabpanel" id="tool-panel-2" aria-labelledby="tool-tab-2" hidden={tab !== 2}>
          <GpgVerifyTab notify={notify} />
        </Box>
      </Container>
      <Dialog open={about} onClose={() => setAbout(false)}>
        <DialogTitle>CheckSum {version && `v${version}`}</DialogTitle>
        <DialogContent>
          <Typography>
            Local file hashing, checksum manifests, and detached GPG verification. File contents are
            processed on your computer. GPG key retrieval is disabled.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setAbout(false)}>Close</Button>
        </DialogActions>
      </Dialog>
      <Snackbar open={Boolean(message)} autoHideDuration={6500} onClose={() => setMessage('')}>
        <Alert severity="info" onClose={() => setMessage('')} sx={{ overflowWrap: 'anywhere' }}>
          {message}
        </Alert>
      </Snackbar>
    </ThemeProvider>
  );
}
