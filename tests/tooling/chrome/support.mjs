// Helpers shared by the tests in this directory. Not a test file (not matched by *.test.mjs).
import { execFile, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

// Pids whose command line contains dir. Independent of tooling/chrome/chrome.mjs.
export async function pgrepDir(dir) {
  try {
    const { stdout } = await execFileAsync('pgrep', ['-f', dir]);
    return stdout.split('\n').filter(Boolean).map(Number);
  } catch (err) {
    if (err.code === 1) return []; // pgrep exits 1 when nothing matches
    throw err;
  }
}

// True when pid exists and is not a zombie.
export function isAlive(pid) {
  try {
    const stat = execFileSync('ps', ['-o', 'stat=', '-p', String(pid)], { encoding: 'utf8' }).trim();
    return stat !== '' && !stat.startsWith('Z');
  } catch {
    return false; // ps exits 1 for an unknown pid
  }
}

export async function eventually(check, ms = 5000) {
  const deadline = Date.now() + ms;
  for (;;) {
    if (await check()) return;
    if (Date.now() > deadline) throw new Error(`condition not met within ${ms} ms`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

export function tempDir(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

// First line written to a stream (the stream is left flowing, so the child never blocks).
export function firstLine(stream) {
  return new Promise((resolve, reject) => {
    let buf = '';
    const onData = (chunk) => {
      buf += chunk;
      const i = buf.indexOf('\n');
      if (i !== -1) {
        stream.off('data', onData);
        resolve(buf.slice(0, i));
      }
    };
    stream.setEncoding('utf8');
    stream.on('data', onData);
    stream.once('error', reject);
  });
}
