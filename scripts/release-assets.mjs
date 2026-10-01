import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { copyFile, mkdir, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

async function filesIn(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await filesIn(full)));
    else if (entry.isFile() && /\.(exe|msi|deb|rpm|appimage|dmg|zip)$/i.test(entry.name))
      result.push(full);
  }
  return result;
}

export async function stageReleaseAssets(input, output) {
  input = path.resolve(input);
  output = path.resolve(output);
  if (output === input || output.startsWith(input + path.sep))
    throw new Error('Release output must be outside the input folder');
  const files = await filesIn(input);
  if (files.length === 0) throw new Error('No release assets found');
  const names = new Set();
  for (const file of files) {
    const name = path.basename(file);
    if (names.has(name)) throw new Error(`Duplicate release asset filename: ${name}`);
    names.add(name);
  }
  await mkdir(output, { recursive: true });
  if ((await readdir(output)).length) throw new Error('Release output folder must be empty');
  const records = [];
  for (const file of files.sort((a, b) => path.basename(a).localeCompare(path.basename(b), 'en'))) {
    const name = path.basename(file);
    const target = path.join(output, name);
    await copyFile(file, target);
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(target)) hash.update(chunk);
    const escaped = /[\\\n\r]/.test(name);
    const encoded = name.replaceAll('\\', '\\\\').replaceAll('\n', '\\n').replaceAll('\r', '\\r');
    records.push(`${escaped ? '\\' : ''}${hash.digest('hex')} *${encoded}`);
  }
  await writeFile(path.join(output, 'SHA256SUMS.txt'), records.join('\n') + '\n');
  return records.length;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [input, output] = process.argv.slice(2);
  if (!input || !output) throw new Error('Usage: node scripts/release-assets.mjs INPUT OUTPUT');
  console.log(`Prepared ${await stageReleaseAssets(input, output)} assets and SHA256SUMS.txt`);
}
