// Run against the standalone WASM audit: node scripts/browser_audit.mjs http://127.0.0.1:8777/
// Uses Chrome's debugging protocol and Node built-ins; no npm dependencies.
import {spawn} from 'node:child_process';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import assert from 'node:assert/strict';

const base = process.argv[2] ?? 'http://127.0.0.1:8777/';
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
  ws.onmessage = e => {
    const m = JSON.parse(e.data);
    if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.reject(m.error) : p.resolve(m.result); }
    if (m.method === 'Runtime.exceptionThrown') exceptions.push(m.params.exceptionDetails);
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
  await call('Page.navigate', {url: base});
  await wait('typeof globalThis.auditResult === "string" || !!globalThis.auditError', 'WASM search audit', 1200);
  assert.equal(await js('globalThis.auditError'), undefined);
  assert.deepEqual(exceptions, []);
  process.stdout.write(await js('globalThis.auditResult'));
} finally {
  ws?.close();
  const exited = chrome.exitCode !== null ? Promise.resolve() : new Promise(resolve => chrome.once('exit', resolve));
  chrome.kill('SIGTERM'); await exited;
  await rm(profile, {recursive: true, force: true, maxRetries: 5, retryDelay: 100});
}
