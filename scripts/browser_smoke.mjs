// Run against a built app: node scripts/browser_smoke.mjs http://127.0.0.1:8080/rubik-cage/
// Uses Chrome's debugging protocol and Node built-ins; no npm dependencies.
import {spawn} from 'node:child_process';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import assert from 'node:assert/strict';

const base = process.argv[2] ?? 'http://127.0.0.1:8080/rubik-cage/';
const profile = await mkdtemp(join(tmpdir(), 'cage-browser-'));
const chrome = spawn(process.env.CHROME ?? 'google-chrome', ['--headless=new', '--no-sandbox', '--disable-gpu', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], {stdio: 'ignore'});
const delay = ms => new Promise(r => setTimeout(r, ms));
let ws;
try {
  let port;
  for (let i = 0; i < 100; i++) {
    try { port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; break; }
    catch { await delay(100); }
  }
  assert(port, 'Chrome did not start');
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  ws = new WebSocket(pages.find(p => p.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  const exceptions = [];
  const workers = new Set();
  ws.onmessage = e => {
    const m = JSON.parse(e.data);
    if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.reject(m.error) : p.resolve(m.result); }
    if (m.method === 'Runtime.exceptionThrown') exceptions.push(m.params.exceptionDetails);
    if (m.method === 'Target.targetCreated' && m.params.targetInfo.type === 'worker') workers.add(m.params.targetInfo.targetId);
  };
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence; pending.set(id, {resolve, reject}); ws.send(JSON.stringify({id, method, params}));
  });
  const js = async expression => {
    const r = await call('Runtime.evaluate', {expression, returnByValue: true, awaitPromise: true});
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  const wait = async (expression, message) => {
    for (let i = 0; i < 300; i++) { if (await js(expression)) return; await delay(100); }
    throw new Error(`Timeout: ${message}`);
  };
  await call('Runtime.enable'); await call('Page.enable');
  await call('Target.setDiscoverTargets', {discover: true});
  await call('Page.navigate', {url: base});
  await wait('!!document.querySelector(".cage")', 'app mount');

  // Legacy format intentionally exercises saved-position migration (derived hash = 0).
  function legacy(p1, p2, bottom = '.........') {
    const bytes = [];
    for (let x = 0; x < 3; x++) for (let y = 0; y < 3; y++) for (let z = 0; z < 3; z++) {
      const color = z === 0 ? bottom[y * 3 + x] : '.';
      color === '.' ? bytes.push(0) : bytes.push(1, color === 'B' ? 4 : 2);
    }
    bytes.push(4, 0, 2, 1, p1, p2, 4, 0, 0, 0);
    return bytes;
  }
  async function importPosition(bytes) {
    await js(`(() => {
      const original = HTMLInputElement.prototype.click;
      HTMLInputElement.prototype.click = function() {
        if (this.type !== 'file') return original.call(this);
        const data = new DataTransfer();
        data.items.add(new File([new Uint8Array(${JSON.stringify(bytes)})], 'position.bin'));
        this.files = data.files; this.dispatchEvent(new Event('change'));
      };
      try { [...document.querySelectorAll('button')].find(b => b.textContent === 'Import position').click(); }
      finally { HTMLInputElement.prototype.click = original; }
    })()`);
    await delay(150);
  }
  const enableLists = () => js(`document.querySelectorAll('.player-panel input[type=checkbox]').forEach(e => { if (!e.checked) e.click(); })`);
  const rows = () => js(`[...document.querySelectorAll('.active-turn .move-list li')].map(e => e.textContent)`);
  const clickMove = async text => {
    assert(await js(`(() => { const e = [...document.querySelectorAll('.active-turn .move-list li')].find(e => e.textContent.startsWith(${JSON.stringify(text)})); if (!e) return false; e.click(); return true; })()`), `Missing move: ${text}`);
    await delay(60);
  };
  const clickButton = text => js(`[...document.querySelectorAll('button')].find(b => b.textContent === ${JSON.stringify(text)}).click()`);

  await enableLists();
  await wait("document.querySelectorAll('.active-turn .move-list li').length === 15", 'all opening moves');
  const opening = await rows();
  for (const row of opening) {
    const drop = row.match(/^Drop at (\d),(\d): /);
    const expected = drop ? (drop[1] === '1' || drop[2] === '1' ? 'Draw' : 'Win in 11') : 'Loss in 12';
    assert(row.endsWith(`: ${expected}`), `Opening evaluation: ${row}, expected ${expected}`);
  }
  assert(!await js('document.body.innerText.includes("Search incomplete")'));
  for (const move of ['Flip:', 'Rotate Down CW:', 'Rotate Up CCW:']) {
    await clickMove(move);
    await wait("document.querySelector('.active-turn .move-list li')?.textContent.endsWith('Win in 11')", 'P2 wins after an empty-board move');
    assert((await rows()).every(r => !/Unknown|Calculating/.test(r)));
    await clickButton('Undo last move');
    await delay(80);
    assert.deepEqual(await rows(), opening);
  }
  // Play the newly certified draw, then return to the opening without losing it.
  await clickMove('Drop at 0,1:');
  await wait("document.querySelector('.active-turn .move-list li')?.textContent.endsWith('Draw')", 'edge drop preserves a draw');
  await clickButton('Undo last move');
  await delay(80);
  assert.deepEqual(await rows(), opening);
  for (let n = 11; n > 0; n--) {
    await wait(`document.querySelector('.active-turn .move-list li')?.textContent.match(/(?:Win|Loss) in ${n}$/)`, `12,12 best move at ${n} plies`);
    const moves = await rows();
    const best = moves[0];
    if (best.includes('Loss')) {
      assert(moves.every(r => /Loss in \d+$/.test(r)), 'every defense is proved losing');
      assert.equal(Math.max(...moves.map(r => Number(r.match(/Loss in (\d+)/)[1]))), n);
    }
    await clickMove(best.split(':')[0] + ':');
  }
  assert(await js('document.body.innerText.includes("Blue won!")'));

  await importPosition(legacy(3, 1));
  await enableLists();
  await wait('document.querySelector(".active-turn .move-list li")?.textContent.includes("Win in 9")', 'cached small-game evaluation');
  await clickMove('Drop at 0,0:');
  await clickMove('Drop at 0,0:');
  const before = (await rows()).find(r => r.startsWith('Rotate Up CCW:'));
  assert.match(before, /Win in \d+/);
  const n = Number(before.match(/Win in (\d+)/)[1]);
  await clickMove('Rotate Up CCW:');
  const replies = await rows();
  assert(replies.every(r => /Loss in \d+/.test(r)));
  assert.equal(Math.max(...replies.map(r => Number(r.match(/Loss in (\d+)/)[1]))), n - 1);
  await clickMove('Flip:');
  await clickButton('Restart the game'); await delay(80);
  assert.equal(await js('document.querySelectorAll(".player-panel:first-child .cubie-icon").length'), 3);
  assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), 1);

  for (let n = 9; n > 0; n--) {
    const best = (await rows())[0];
    assert.match(best, new RegExp(`(?:Win|Loss) in ${n}$`));
    await clickMove(best.split(':')[0] + ':');
  }
  assert(await js('document.body.innerText.includes("Blue won!")'));
  assert.equal((await rows()).length, 0);

  // Import an uncached game while the provider is mounted; move/undo during work.
  await importPosition(legacy(3, 2));
  await clickMove('Drop at 0,0:');
  await clickButton('Undo last move');
  await wait('!!document.querySelector(".active-turn .move-list li") && [...document.querySelectorAll(".active-turn .move-list li")].every(e => !/Unknown|Calculating/.test(e.textContent))', 'worker completion after move and undo');
  assert((await rows()).some(r => r.endsWith('Draw')));
  assert.equal(workers.size, 1, 'worker reused across positions and imports');
  await clickButton('Restart the game'); await delay(80);
  assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), 2);

  await importPosition(legacy(0, 0, 'RBBR.BR.B'));
  await wait('document.body.innerText.includes("Draw: both players have a line.")', 'simultaneous-line terminal');
  assert.equal((await rows()).length, 0);
  assert(await js('[...document.querySelectorAll(".cage > button, .layer button")].every(e => e.disabled)'));
  assert.deepEqual(exceptions, [], 'browser runtime exceptions');
  console.log('PASS: all 15 opening evaluations, certified edge-drop draw, verified 11-ply full-inventory game, one reusable worker, legacy import, cached replay, exact 9-ply line, fresh solve, undo/restart, simultaneous draw');
} finally {
  ws?.close();
  const exited = chrome.exitCode !== null ? Promise.resolve() : new Promise(resolve => chrome.once('exit', resolve));
  chrome.kill('SIGTERM'); await exited;
  await rm(profile, {recursive: true, force: true, maxRetries: 5, retryDelay: 100});
}
