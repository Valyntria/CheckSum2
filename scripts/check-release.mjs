import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';

const tag = process.env.RELEASE_TAG;
const pkg = JSON.parse(await readFile('package.json', 'utf8'));
const config = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'));
const cargo = await readFile('src-tauri/Cargo.toml', 'utf8');
const rustVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (
  tag !== `v${pkg.version}` ||
  pkg.version !== config.package.version ||
  pkg.version !== rustVersion
) {
  throw new Error('Release tag and JavaScript/Rust/Tauri versions must agree');
}
const target = execFileSync('git', ['rev-parse', '--verify', `refs/tags/${tag}^{commit}`], {
  encoding: 'utf8',
}).trim();
const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
if (target !== head) throw new Error('Checkout does not match the requested existing tag');
console.log(`Validated ${tag} at ${head}`);
