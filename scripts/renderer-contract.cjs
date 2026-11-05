#!/usr/bin/env node
// Real process responses consumed by the current host's FormRuntime and notice
// functions. No browser, OS clipboard, installation, or host source writes.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');
const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const { randomUUID } = require('node:crypto');

async function main() {
  const [hostRoot, binaryPath] = process.argv.slice(2);
  assert(hostRoot && binaryPath, 'Usage: node scripts/renderer-contract.cjs HOST_ROOT BINARY');
  const requireHost = createRequire(path.join(path.resolve(hostRoot), 'package.json'));
  const esbuild = requireHost('esbuild');
  const ts = requireHost('typescript');
  const React = requireHost('react');
  const { renderToStaticMarkup } = requireHost('react-dom/server');
  const { JSDOM } = requireHost('jsdom');
  function evaluate(code) {
    const module = { exports: {} };
    vm.runInNewContext(code, { module, exports: module.exports, require: requireHost,
      console, setTimeout, clearTimeout, TextEncoder, TextDecoder, URL }, { timeout: 10000 });
    return module.exports;
  }
  const formFile = path.join(hostRoot, 'src/renderer/src/raycast-api/form-runtime.tsx');
  const bundled = esbuild.buildSync({ entryPoints: [formFile], bundle: true, write: false,
    platform: 'node', format: 'cjs', packages: 'external', jsx: 'automatic' });
  const { FormRuntime } = evaluate(bundled.outputFiles[0].text);
  const runtimeSource = fs.readFileSync(path.join(hostRoot, 'src/renderer/ExtensionRuntimeView.tsx'), 'utf8');
  const ast = ts.createSourceFile('runtime.tsx', runtimeSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  // Compile the actual host functions rather than a second oracle implementation.
  const names = new Set(['effectNotice', 'fallbackActionNotice']);
  const functions = ast.statements.filter(n => ts.isFunctionDeclaration(n) && names.has(n.name?.text))
    .map(n => n.getText(ast)).join('\n');
  assert.equal(functions.includes('function effectNotice'), true);
  const start = runtimeSource.indexOf('const effectNoticeValue = effectNotice(result.effects)');
  const end = runtimeSource.indexOf('if (notice) showActionNotice', start);
  assert(start >= 0 && end > start);
  const selectedNotice = runtimeSource.slice(start, end);
  const notices = evaluate(esbuild.transformSync(`${functions}\nexports.pick = (result, invokedAction) => { ${selectedNotice} return notice; };`, { loader: 'ts', format: 'cjs' }).code);

  const child = spawn(path.resolve(binaryPath), [], { stdio: ['pipe', 'pipe', 'inherit'] });
  const frames = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  const deadline = setTimeout(() => child.kill(), 30000);
  let writeFailure = false;
  const reverse = [];
  async function receive() {
    const next = await frames.next();
    assert.equal(next.done, false, 'Unexpected process EOF');
    return JSON.parse(next.value);
  }
  function send(frame) { child.stdin.write(JSON.stringify(frame) + '\n'); }
  async function request(method, params) {
    const id = 'h:' + randomUUID();
    send({ type: 'request', id, method, params });
    for (;;) {
      const frame = await receive();
      if (frame.type === 'request') {
        reverse.push(frame);
        switch (frame.method) {
          case 'host.clipboard.read': case 'host.selection.read':
            assert.deepEqual(frame.params, {});
            send({ type: 'response', id: frame.id, result: 'one two three\n\nfour five' });
            break;
          case 'host.clipboard.write':
            assert.equal(frame.params.text, 'one\ntwo\nthree\n\nfour\nfive');
            send(writeFailure
              ? { type: 'response', id: frame.id, error: { code: 'host-error', message: 'OS clipboard write denied' } }
              : { type: 'response', id: frame.id, result: { ok: true } });
            break;
          default: throw new Error('Unrecognized reverse RPC: ' + frame.method);
        }
      } else if (frame.type === 'response' && frame.id === id) {
        assert.equal(frame.error, undefined, JSON.stringify(frame));
        return frame.result;
      } else assert(['event', 'cancel'].includes(frame.type), JSON.stringify(frame));
    }
  }
  function render(view, notice) {
    assert.equal(view.mode, 'view');
    assert.equal(view.root.type, 'Form');
    const markup = renderToStaticMarkup(React.createElement(FormRuntime, {
      root: view.root, title: view.title, actionNotice: notice,
      onBack() {}, onSubmitForm() {}, onChangeField() {}, onInvokeAction() {}, onOpenActions() {},
    }));
    return new JSDOM(markup).window.document;
  }
  function formValues(view) {
    return Object.fromEntries(view.root.children.filter(n => n.props?.id)
      .map(n => [n.props.id, n.props.title === 'Text' ? 'one two three\n\nfour five' : n.props.title === 'Width' ? '5' : n.props.value]));
  }
  function assertOutput(document) {
    const label = [...document.querySelectorAll('label')].find(n => n.textContent === 'Rewrapped Text');
    assert(label, 'Generated output must have an actual rendered control');
    const control = document.getElementById(label.htmlFor);
    assert.equal(control.tagName, 'TEXTAREA');
    assert.equal(control.value, 'one\ntwo\nthree\n\nfour\nfive');
  }
  try {
    assert.equal((await receive()).type, 'hello');
    const grants = ['host.clipboard.read', 'host.clipboard.write', 'host.selection.read'];
    await request('initialize', { protocol: 1, extensionId: 'com.tezbar.native.text-rewrap',
      platform: 'aarch64-apple-darwin', packageDir: path.dirname(path.resolve(binaryPath)),
      dataDir: path.resolve('target/renderer-contract-data'), tempDir: path.resolve('target/renderer-contract-temp'),
      grantedCapabilities: grants, reason: 'command', migrationPaths: {} });
    for (const command of ['text-rewrap-form', 'text-rewrap-clipboard', 'text-rewrap-selection']) {
      for (const encoded of ['false', false, 'true', true]) {
        const checked = encoded === true || encoded === 'true';
        const view = await request('session.open', { sessionId: 'renderer', commandName: command,
          preferences: { defaultCopyToClipboard: encoded }, argumentValues: {} });
        const document = render(view);
        assert.equal(document.querySelector('input[type=checkbox]').checked, checked);
        const before = reverse.length;
        const submitted = await request('session.call', { sessionId: 'renderer', operation: 'extension:invoke-action',
          payload: { actionId: view.actions.find(a => a.kind === 'submit-form').id, formValues: formValues(view) } });
        assert.equal(reverse.slice(before).filter(f => f.method === 'host.clipboard.write').length, checked ? 1 : 0);
        assertOutput(render(submitted));
        const copy = submitted.actions.find(a => a.kind === 'copy');
        writeFailure = true;
        const failed = await request('session.call', { sessionId: 'renderer', operation: 'extension:invoke-action', payload: { actionId: copy.id } });
        const notice = notices.pick(failed, copy);
        assert.equal(notice.tone, 'error');
        assert.match(notice.message, /^Failed · Unable to copy/);
        assert.doesNotMatch(notice.message, /Copied to clipboard/);
        const failedDocument = render(failed, notice);
        assertOutput(failedDocument);
        assert(failedDocument.body.textContent.includes(notice.message), 'Failure notice must actually render');
        writeFailure = false;
        const retry = await request('session.call', { sessionId: 'renderer', operation: 'extension:invoke-action', payload: { actionId: copy.id,
          // Editing the preview cannot replace the v2-owned copy source.
          formValues: { [failed.root.children.find(n => n.props?.title === 'Rewrapped Text').props.id]: 'edited preview' } } });
        assert.equal(notices.pick(retry, copy).tone, 'success');
        assertOutput(render(retry));
        await request('session.dispose', { sessionId: 'renderer' });
      }
    }
    await request('shutdown', {});
    const code = await new Promise(resolve => child.once('exit', resolve));
    assert.equal(code, 0);
    console.log('PASS: real-process preferences, multiline/blank paragraphs, failed Copy notice and retry through current host FormRuntime + notice selection (12 scenarios; SSR DOM, no desktop QA).');
  } finally {
    clearTimeout(deadline);
    child.stdin.end();
    if (child.exitCode === null) child.kill();
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
