import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { stageReleaseAssets } from './release-assets.mjs';

test('stages nested installers with one checksum per final filename', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'checksum-release-'));
  try {
    const input = path.join(root, 'input');
    const output = path.join(root, 'output');
    for (const name of ['macos/CheckSum.app.zip', 'msi/CheckSum.msi', 'CheckSum-portable.zip']) {
      const file = path.join(input, name);
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, name);
    }
    assert.equal(await stageReleaseAssets(input, output), 3);
    const lines = (await readFile(path.join(output, 'SHA256SUMS.txt'), 'utf8')).trim().split('\n');
    assert.equal(lines.length, 3);
    for (const line of lines) {
      const [hash, name] = line.split(' *');
      assert.equal(name, path.basename(name));
      assert.equal(
        hash,
        createHash('sha256')
          .update(await readFile(path.join(output, name)))
          .digest('hex'),
      );
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects duplicate filenames and empty release inputs', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'checksum-release-'));
  try {
    const input = path.join(root, 'input');
    const output = path.join(root, 'output');
    await mkdir(input);
    await assert.rejects(stageReleaseAssets(input, output), /No release assets/);
    for (const sub of ['one', 'two']) {
      await mkdir(path.join(input, sub));
      await writeFile(path.join(input, sub, 'same.zip'), sub);
    }
    await assert.rejects(stageReleaseAssets(input, output), /Duplicate/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
