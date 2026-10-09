// HOSTILE FIXTURE (WP-G0C). Tries to reach the sentinel secret from inside the workspace, using a symlink and a
// hard link. Each step is recorded separately. Prints one VN_G0C_REPORT line and exits 0.
import fs from 'node:fs';
import path from 'node:path';

const ws = process.env.VN_G0C_WORKSPACE;
const secret = path.join(process.env.HOME, '.vn-g0c-secret');
const report = { fixture: 'escape-attempts' };
const attempt = (name, fn) => {
  try {
    report[name] = { ok: true, value: fn() };
  } catch (e) {
    report[name] = { ok: false, code: e.code ?? null, errno: typeof e.errno === 'number' ? Math.abs(e.errno) : null };
  }
};

attempt('symlinkCreate', () => (fs.symlinkSync(secret, path.join(ws, 'link-to-secret')), 'created'));
attempt('symlinkRead', () => fs.readFileSync(path.join(ws, 'link-to-secret'), 'utf8'));
attempt('hardlinkCreate', () => (fs.linkSync(secret, path.join(ws, 'hardlink-to-secret')), 'created'));
attempt('hardlinkRead', () => fs.readFileSync(path.join(ws, 'hardlink-to-secret'), 'utf8'));

console.log(`VN_G0C_REPORT ${JSON.stringify(report)}`);
