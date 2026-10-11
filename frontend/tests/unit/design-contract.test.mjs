import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { checkFeatureCss, checkTokenContract } from '../../scripts/check-design.mjs';
const tokens = new Set(['--text', '--surface', '--font-ui', '--space-2']);
test('new family styles use known semantic values', () => {
    assert.deepEqual(checkFeatureCss('new.css', '.card { color: var(--text); background: var(--surface); font-family: var(--font-ui); padding: var(--space-2); }', tokens), []);
});
test('new styles cannot quietly introduce raw colors, fonts or CSS zoom', () => {
    assert.equal(checkFeatureCss('new.css', '.x { color:#abc; background:rgb(1,2,3); font-family:Arial; zoom:.8; }', tokens).length, 3);
});
test('unknown token references fail instead of silently falling back', () => {
    assert.match(checkFeatureCss('new.css', '.x { color:var(--brand-new, red) }', tokens)[0], /unknown/);
});
test('shared primitives cannot drift without a reviewed contract change', () => {
    assert.deepEqual(checkTokenContract(':root { --space-2: 8px; }', { '--space-2': '8px' }), []);
    assert.equal(checkTokenContract(':root { --space-2: 9px; }', { '--space-2': '8px' }).length, 1);
});
test('standalone styles are derived from the same index manifest', () => {
    assert.match(readFileSync('scripts/standalone.mjs', 'utf8'), /stylePaths\.map/);
    assert.match(readFileSync('index.html', 'utf8'), /styles\/loading\.css/);
});
test('the guide and agent entry point record the approved UX invariants', () => {
    const guide = readFileSync('STYLE_GUIDE.md', 'utf8');
    for (const invariant of ['Connector chats', 'above the composer', 'Delivery not confirmed', 'reduced motion', 'Mobile direction', 'PROTOCOL-LOCK.json'])
        assert.ok(guide.includes(invariant), invariant);
    assert.match(readFileSync('AGENTS.md', 'utf8'), /STYLE_GUIDE\.md/);
});
