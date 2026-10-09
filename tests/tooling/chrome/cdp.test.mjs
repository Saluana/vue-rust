import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { CdpSession, chromeSkipReason, launchChrome } from '../../../tooling/chrome/chrome.mjs';

const skip = chromeSkipReason();
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed
if (skip) process.stderr.write(`[tooling/chrome] SKIPPED cdp.test.mjs: ${skip}\n`);

describe('CdpSession', () => {
  let browser;
  let cdp;
  before(async () => {
    if (skip) return;
    browser = await launchChrome();
    cdp = await CdpSession.connect(browser.wsUrl);
  });
  after(async () => {
    if (skip) return;
    await cdp?.close();
    await browser?.close();
  });

  test('send resolves with the result; protocol errors reject naming the method', skipOpt, async () => {
    const { product } = await cdp.send('Browser.getVersion');
    assert.match(product, /^Chrome\/\d+\.\d+\./);
    await assert.rejects(cdp.send('Nope.nope'), /Nope\.nope failed: /);
  });

  test('waitForEvent resolves on a matching event and rejects after its timeout', skipOpt, async () => {
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    const created = cdp.waitForEvent('Target.targetCreated', (p) => p.targetInfo.type === 'page', 5000);
    await cdp.send('Target.createTarget', { url: 'about:blank' });
    assert.equal((await created).targetInfo.type, 'page');
    await assert.rejects(
      cdp.waitForEvent('Target.targetDestroyed', () => false, 100),
      /timed out after 100 ms waiting for Target\.targetDestroyed/,
    );
  });

  test('on() receives events until unsubscribed', skipOpt, async () => {
    const seen = [];
    const off = cdp.on('Target.targetCreated', (p) => seen.push(p.targetInfo.id));
    const next = cdp.waitForEvent('Target.targetCreated', () => true, 5000);
    await cdp.send('Target.createTarget', { url: 'about:blank' });
    await next;
    assert.equal(seen.length, 1);
    off();
    const again = cdp.waitForEvent('Target.targetCreated', () => true, 5000);
    await cdp.send('Target.createTarget', { url: 'about:blank' });
    await again;
    assert.equal(seen.length, 1);
  });

  test('sending after close() rejects', skipOpt, async () => {
    const other = await launchChrome();
    const session = await CdpSession.connect(other.wsUrl);
    await session.close();
    await assert.rejects(session.send('Browser.getVersion'), /CDP connection closed/);
    await other.close();
  });
});
