// Run against a built app: node scripts/browser_smoke.mjs http://127.0.0.1:8080/rubik-cage/
// Uses Chrome's debugging protocol and Node built-ins; no npm dependencies.
import {spawn} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
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
  const wait = async (expression, message, iterations = 300) => {
    for (let i = 0; i < iterations; i++) { if (await js(expression)) return; await delay(100); }
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
    assert((await rows()).every(r => !/Unknown|Calculating|Queued/.test(r)));
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
  await wait('!!document.querySelector(".active-turn .move-list li") && [...document.querySelectorAll(".active-turn .move-list li")].every(e => !/Unknown|Calculating|Queued/.test(e.textContent))', 'worker completion after move and undo');
  assert((await rows()).some(r => r.endsWith('Draw')));
  assert.equal(workers.size, 1, 'worker reused across positions and imports');
  await clickButton('Restart the game'); await delay(80);
  assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), 2);

  await importPosition(legacy(0, 0, 'RBBR.BR.B'));
  await wait('document.body.innerText.includes("Draw: both players have a line.")', 'simultaneous-line terminal');
  assert.equal((await rows()).length, 0);
  assert(await js('[...document.querySelectorAll(".cage > button, .layer button")].every(e => e.disabled)'));
  // Settings stay collapsed until requested and can start a game from a terminal board.
  assert.equal(await js('!!document.querySelector("#game-settings-panel")'), false);
  const settingsInput = async (player, value) => {
    await js(`(() => { const e = document.querySelector('[name="p${player}-cubies"]'); e.value = ${JSON.stringify(String(value))}; e.dispatchEvent(new Event('input', {bubbles:true})); })()`);
    await delay(30);
  };
  const assertPresetMatchesFields = async () => {
    const fields = await js('document.querySelectorAll(".settings-fields input").length');
    assert.equal(await js(`document.querySelector('[name="game-preset"]').value`), fields === 2 ? 'single' : 'multi', 'preset agrees with starting-stock fields on opening');
  };
  const chooseGame = async (m, n) => {
    await clickButton('Game settings'); await delay(50);
    await assertPresetMatchesFields();
    await settingsInput(1, m); await settingsInput(2, n);
    await clickButton('Start new game'); await delay(60);
    assert.equal(await js('!!document.querySelector("#game-settings-panel")'), false);
    assert.equal(await js('document.querySelectorAll(".player-panel:first-child .cubie-icon").length'), m);
    assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), n);
    assert(await js('[...document.querySelectorAll("button")].find(b => b.textContent === "Undo last move").disabled'));
    assert(await js('document.querySelector(".player-panel:first-child").classList.contains("active-turn")'));
  };
  await clickButton('Game settings'); await delay(50);
  await assertPresetMatchesFields();
  for (const invalid of [13, -1, 2.5, '']) {
    await settingsInput(1, invalid);
    assert(await js('document.querySelector("#game-settings-panel button[type=submit]").disabled'));
  }
  await clickButton('Cancel'); await delay(50);
  assert(await js('document.body.innerText.includes("Draw: both players have a line.")'), 'cancel leaves the game intact');

  await chooseGame(3, 3);
  await wait('document.querySelectorAll(".active-turn .move-list li").length === 15 && [...document.querySelectorAll(".active-turn .move-list li")].every(e => e.textContent.endsWith("Draw"))', '3,3 opening including all flips and rotations');
  await clickMove('Flip:');
  await wait('[...document.querySelectorAll(".active-turn .move-list li")].length > 0 && [...document.querySelectorAll(".active-turn .move-list li")].every(e => e.textContent.endsWith("Draw"))', 'player-swapped 3,3 opening');
  await clickButton('Undo last move'); await delay(80);
  assert((await rows()).every(r => r.endsWith('Draw')));

  await chooseGame(0, 0);
  await wait('[...document.querySelectorAll(".active-turn .move-list li")].length === 7 && [...document.querySelectorAll(".active-turn .move-list li")].every(e => e.textContent.endsWith("Draw"))', 'zero-cubie game solved on demand');
  await chooseGame(5, 4);
  await wait('document.body.innerText.includes("Calculating")', 'custom game starts background evaluation');
  await wait('document.body.innerText.includes("Calculating… batch 2")', 'automatic continuation after the first batch');
  await wait('document.body.innerText.includes("Search paused after its work budget")', 'bounded search pauses visibly', 600);
  assert((await rows()).some(r => r.includes('Unknown (search paused)')));
  assert(await js('[...document.querySelectorAll("button")].some(b => b.textContent === "Continue search")'));
  await chooseGame(12, 12);
  await chooseGame(5, 4);
  assert(await js('document.body.innerText.includes("Search paused")'), 'pause and resume controls survive changing games');
  await clickButton('Continue search');
  await wait('document.body.innerText.includes("Calculating… batch 9")', 'explicit continuation advances the budget');
  await chooseGame(3, 0); // Switch inventory while the resumed request is still running.
  await wait('document.querySelector(".active-turn .move-list li")?.textContent.endsWith("Win in 5")', 'new inventory evaluated after a pending request');
  assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), 0);
  await clickMove('Drop at 0,0:');
  await clickButton('Game settings'); await delay(50);
  assert.equal(await js(`document.querySelector('[name="p1-cubies"]').value`), '3', 'settings show starting stocks, not remaining pieces');
  await clickButton('Cancel'); await delay(50);
  assert.equal(await js('document.querySelectorAll(".player-panel:first-child .cubie-icon").length'), 2);
  await clickButton('Restart the game'); await delay(60);
  assert.equal(await js('document.querySelectorAll(".player-panel:first-child .cubie-icon").length'), 3);
  assert.equal(await js('document.querySelectorAll(".player-panel:last-child .cubie-icon").length'), 0);

  await chooseGame(12, 12);
  assert.deepEqual(await rows(), opening, 'returning to standard size reuses the standard table');
  assert.equal(workers.size, 1, 'settings reuse the existing worker');
  // Preset selection and its starting-stock controls stay synchronized.
  const preset = async value => {
    await clickButton('Game settings'); await delay(50);
    await assertPresetMatchesFields();
    await js(`(() => { const e = document.querySelector('[name="game-preset"]'); e.value = ${JSON.stringify(value)}; e.dispatchEvent(new Event('change', {bubbles:true})); })()`);
    await delay(50);
    await assertPresetMatchesFields();
  };
  await preset('multi');
  assert.equal(await js('document.querySelectorAll(".settings-fields input").length'), 6);
  assert(await js('[...document.querySelectorAll(".settings-fields input")].every(e => e.value === "3")'));
  await clickButton('Cancel'); await delay(50);
  assert.deepEqual(await rows(), opening, 'canceling a preset preserves the current game');
  await preset('multi');
  await clickButton('Start new game'); await delay(80);
  await enableLists();
  assert.equal((await rows()).length, 31);
  assert((await rows()).some(r => /Unknown|Calculating|Queued/.test(r)));
  await wait('document.body.innerText.includes("Calculating")', 'general search starts in the shared worker');
  await clickButton('Pause search');
  await wait('[...document.querySelectorAll("button")].some(b => b.textContent === "Continue search" && !b.disabled)', 'manual pause finishes its short batch');
  assert((await rows()).some(r => r.includes('Unknown (search paused)')));
  await clickButton('Continue search');
  await wait('document.body.innerText.includes("Calculating")', 'general search resumes');
  assert.equal(await js('document.querySelectorAll(".color-selector").length'), 6);
  if (process.env.MULTICOLOR_SCREENSHOT) {
    await writeFile(process.env.MULTICOLOR_SCREENSHOT, Buffer.from((await call('Page.captureScreenshot', {captureBeyondViewport: true})).data, 'base64'));
    await call('Emulation.setDeviceMetricsOverride', {width:1400, height:1000, deviceScaleFactor:1, mobile:false});
    await writeFile(process.env.MULTICOLOR_SCREENSHOT.replace(/\.png$/, '.desktop.png'), Buffer.from((await call('Page.captureScreenshot', {captureBeyondViewport:true})).data, 'base64'));
    await call('Emulation.clearDeviceMetricsOverride');
  }
  const cssColors = await js(`[...document.querySelectorAll('.color-selector .cubie-icon')].map(e => getComputedStyle(e).backgroundColor)`);
  assert.equal(new Set(cssColors).size, 6, 'all six colors have distinct rendered styles');
  const reserve = color => js(`document.querySelector('.color-reserve[data-color="${color}"] .color-selector').textContent`);
  await js(`document.querySelector('.active-turn .color-selector[data-color="Green"]').click()`);
  await delay(40);
  assert(await js(`document.querySelector('.slot[aria-label="Drop Green at 0,0"]') !== null`));
  // Keyboard board interaction spends the selected exact color.
  await js(`document.querySelector('.slot[aria-label="Drop Green at 0,0"]').dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true}))`);
  await delay(80);
  assert.match(await reserve('Green'), /Green: 2/);
  assert.match(await reserve('White'), /White: 3/);
  assert.match(await reserve('Blue'), /Blue: 3/);
  assert.equal(await js(`document.querySelectorAll('.slot[aria-label="Green"]').length`), 1);
  await clickMove('Drop Orange at 2,2:');
  assert.match(await reserve('Orange'), /Orange: 2/);
  // Move previews use their own color even when a different color is selected.
  await js(`document.querySelector('.active-turn .color-selector[data-color="White"]').click()`);
  await js(`(() => { const row = [...document.querySelectorAll('.active-turn .move-list li')].find(e => e.textContent.startsWith('Drop Blue at 0,1:')); row.dispatchEvent(new MouseEvent('mouseenter', {bubbles:true})); })()`);
  await delay(60);
  assert(await js(`[...document.querySelectorAll('.slot.highlighted')].some(e => e.style.getPropertyValue('--highlight-color').trim() === 'var(--cubie-blue)')`));
  await clickButton('Undo last move'); await delay(60);
  assert.match(await reserve('Orange'), /Orange: 3/);
  assert.equal(await js('document.querySelectorAll(".slot.highlighted").length'), 0);
  await clickMove('Drop Yellow at 2,1:');
  // Export through the actual UI, then restart and import its bytes.
  await js(`window.__originalOpen = window.open; window.open = url => { window.__positionBytes = fetch(url).then(r => r.arrayBuffer()).then(b => [...new Uint8Array(b)]); return null; }`);
  await clickButton('Export position');
  const multiBytes = await js('window.__positionBytes');
  await js('window.open = window.__originalOpen');
  assert.equal(new TextDecoder().decode(new Uint8Array(multiBytes.slice(0, 8))), 'RCGPOS02');
  await clickButton('Restart the game'); await delay(60);
  assert.match(await reserve('Green'), /Green: 3/);
  assert.match(await reserve('Yellow'), /Yellow: 3/);
  await importPosition(multiBytes);
  assert.match(await reserve('Green'), /Green: 2/);
  assert.match(await reserve('Yellow'), /Yellow: 2/);
  await clickButton('Game settings'); await delay(60);
  assert(await js('[...document.querySelectorAll(".settings-fields input")].every(e => e.value === "3")'), 'settings recover per-color initial inventories');
  await clickButton('Cancel');

  // Frozen v2 wire fixture: six colors, exclusive owners and exact reserves.
  function multiPosition(bottom, remaining = [3,3,3,3,3,3]) {
    const bytes = [...new TextEncoder().encode('RCGPOS02'), 1];
    const codes = {W:0, Y:1, R:2, O:3, B:4, G:5};
    for (let x = 0; x < 3; x++) for (let y = 0; y < 3; y++) for (let z = 0; z < 3; z++) {
      const c = z === 0 ? bottom[y * 3 + x] : '.';
      c === '.' ? bytes.push(0) : bytes.push(1, codes[c]);
    }
    for (const owner of [0,1,1,1,0,0]) bytes.push(1, owner);
    bytes.push(...remaining, 0, 0); // P1 turn, no previous move
    return bytes;
  }
  // Mixed material is insufficient even though each player has several cubies.
  await importPosition(multiPosition('.........', [2,2,2,2,2,2]));
  await wait('[...document.querySelectorAll(".active-turn .move-list li")].length === 31 && [...document.querySelectorAll(".active-turn .move-list li")].every(e => e.textContent.endsWith("Draw"))', 'per-color material draw');
  // A nonterminal exact result from the general worker, not a bundled table.
  await importPosition(multiPosition('.........', [3,0,0,0,0,0]));
  await wait('[...document.querySelectorAll(".active-turn .move-list li")].some(e => e.textContent === "Drop White at 0,0: Win in 5")', 'general nonterminal five-ply win');
  // Only one Green left: exhausting it selects another available owned color.
  await importPosition(multiPosition('.........', [0,0,0,0,3,1]));
  await js(`document.querySelector('.active-turn .color-selector[data-color="Green"]').click()`);
  await delay(30); await clickMove('Drop Green at 0,0:');
  await clickMove('Flip:');
  assert(await js(`document.querySelector('.active-turn .color-selector[data-color="Green"]').disabled`));
  assert(await js(`document.querySelector('.active-turn .color-selector[data-color="Blue"]').getAttribute('aria-pressed') === 'true'`));
  assert(!(await rows()).some(r => r.startsWith('Drop Green')));
  assert((await rows()).some(r => r.startsWith('Drop Blue')));
  await clickButton('Undo last move'); await clickButton('Undo last move'); await delay(60);
  assert(await js(`document.querySelector('.active-turn .color-selector[data-color="Green"]').getAttribute('aria-pressed') === 'true'`));

  await importPosition(multiPosition('WBG......', [2,3,3,3,2,2]));
  assert.equal(await js('!!document.querySelector(".cage > h2")'), false, 'mixed owned colors do not win');
  await importPosition(multiPosition('GG.......', [3,3,3,3,3,1]));
  assert((await rows()).includes('Drop Green at 2,0: Win in 1'), 'terminal move values remain available');
  await clickMove('Drop Green at 2,0:');
  assert(await js('document.body.innerText.includes("Player 1 wins with Green!")'));
  assert.equal(await js('document.querySelectorAll(".winning-line").length'), 3);
  await importPosition(multiPosition('WWW...BBB', [0,3,3,3,0,3]));
  assert(await js('document.body.innerText.includes("Player 1 wins with")'), 'two winning colors owned by one player are a win');
  await importPosition(multiPosition('WWW...RRR', [0,3,0,3,3,3]));
  assert(await js('document.body.innerText.includes("Draw: both players have a line.")'));

  // Switch from active classic search to multi-color play, then back to the table.
  await preset('single'); await clickButton('Start new game'); await delay(60);
  await chooseGame(6, 5);
  await wait('document.body.innerText.includes("Calculating")', 'background work before multi-color switch');
  await preset('multi'); await clickButton('Start new game'); await delay(80);
  assert((await rows()).some(r => /Unknown|Calculating|Queued/.test(r)));
  await clickMove('Drop Blue at 0,0:');
  await clickButton('Undo last move'); await delay(60);
  assert.equal((await rows()).length, 31);
  await preset('single'); await clickButton('Start new game'); await delay(80);
  assert.deepEqual(await rows(), opening, 'classic table survives multi-color play and pending requests');
  assert.equal(workers.size, 1, 'multi-color play does not spawn another worker');

  assert.deepEqual(exceptions, [], 'browser runtime exceptions');
  console.log('PASS: all 15 opening evaluations, certified edge-drop draw, verified 11-ply full-inventory game, one reusable worker, legacy import, cached replay, exact 9-ply line, fresh solve, undo/restart, simultaneous draw, custom settings, complete 3,3 opening, player-swap reuse, automatic and explicit continuation, inventory switching, multi-color selection/stocks/keyboard/previews/export/import/wins/general-search/material-draw/pause/resume/Unknown/switching');
} finally {
  ws?.close();
  const exited = chrome.exitCode !== null ? Promise.resolve() : new Promise(resolve => chrome.once('exit', resolve));
  chrome.kill('SIGTERM'); await exited;
  await rm(profile, {recursive: true, force: true, maxRetries: 5, retryDelay: 100});
}
