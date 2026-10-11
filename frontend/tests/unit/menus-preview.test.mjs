import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { writeRouteHash } from '../../dist/app/app/route-location.js';
import { checkFeatureCss, checkMenuOwnership } from '../../scripts/check-design.mjs';
const target = (hash = '') => {
    const calls = [];
    return { calls, location: { hash }, history: {
            pushState: (...args) => calls.push(['push', ...args]),
            replaceState: (...args) => calls.push(['replace', ...args]),
        } };
};
test('ordinary navigation preserves browser pushState', () => {
    const host = target();
    assert.equal(writeRouteHash('#ideas', false, host), true);
    assert.deepEqual(host.calls, [['push', null, '', '#ideas']]);
});
test('route correction replaces rather than duplicates history', () => {
    const host = target('#chat/deleted');
    assert.equal(writeRouteHash('#chat/main', true, host), true);
    assert.deepEqual(host.calls, [['replace', null, '', '#chat/main']]);
});
test('an already-selected URL does not grow history', () => {
    const host = target('#library/images');
    assert.equal(writeRouteHash('#library/images', false, host), true);
    assert.deepEqual(host.calls, []);
});
test('opaque-origin preview failures do not throw or navigate the parent', () => {
    let attempts = 0;
    const denied = () => { attempts++; throw new DOMException('Opaque preview origin', 'SecurityError'); };
    const host = { location: { hash: '' }, history: { pushState: denied, replaceState: denied } };
    assert.equal(writeRouteHash('#ideas', false, host), false);
    assert.equal(writeRouteHash('#chat/main', true, host), false);
    assert.equal(attempts, 2);
});
test('unavailable history getters also leave local rendering available', () => {
    const host = { location: { hash: '' }, get history() { throw new Error('Unavailable host'); } };
    assert.equal(writeRouteHash('#feed', false, host), false);
});
test('route writer cannot become an arbitrary external navigation helper', () => {
    const host = target();
    for (const url of ['https://example.invalid', '//elsewhere', 'javascript:alert(1)', 'ideas'])
        assert.equal(writeRouteHash(url, false, host), false);
    assert.deepEqual(host.calls, []);
});
test('all action menus use the shared semantic contract, not per-feature colors', () => {
    const css = readFileSync('src/styles/menus.css', 'utf8');
    const contract = JSON.parse(readFileSync('docs/DESIGN-TOKENS.json', 'utf8'));
    const tokens = readFileSync('src/styles/tokens.css', 'utf8');
    const known = new Set([...tokens.matchAll(/(--[a-z0-9-]+)\s*:/g)].map(match => match[1]));
    for (const key of ['--ui-space', '--ui-scale', ...contract.localVariables])
        known.add(key);
    assert.deepEqual(checkFeatureCss('menus.css', css, known), []);
    for (const key of ['--menu-bg', '--menu-radius', '--menu-border', '--menu-item-height', '--menu-icon-size'])
        assert.ok(contract.requiredTokens[key]);
    assert.match(css, /\.menu:not\(\.reaction-menu\) > \.menu-popover/);
    assert.match(css, /prefers-reduced-motion/);
    assert.match(css, /pointer: coarse/);
});
test('menu separators and trigger relationships are owned by the common component', () => {
    const source = readFileSync('src/components/ui.ts', 'utf8');
    for (const marker of ['separatorBefore', "role: 'separator'", "'aria-expanded'", "'aria-controls'", 'setFloatingMenuOpen', 'menu-item-label'])
        assert.ok(source.includes(marker), marker);
});
test('standalone retains its CSP hash and includes the same menu stylesheet', () => {
    const html = readFileSync('dist/hatchling-standalone.html', 'utf8');
    const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
    assert.ok(html.includes(`sha256-${createHash('sha256').update(script).digest('base64')}`));
    assert.ok(html.includes(readFileSync('src/styles/menus.css', 'utf8').trim()));
    assert.match(html, /connect-src 'none'/);
    assert.doesNotMatch(html, /<script[^>]+src=/);
    assert.doesNotMatch(html, /<link rel="stylesheet"/);
});
test('script-blocking viewers receive a readable fallback instead of an empty mount', () => {
    const html = readFileSync('index.html', 'utf8');
    assert.match(html, /class="preview-startup"/);
    assert.match(html, /Attachment viewers that block scripts/);
    assert.match(html, /<div id="app">\s*<section/);
});
test('menu ownership guard rejects the old local border/background overrides', () => {
    assert.equal(checkMenuOwnership('src/styles/details.css', '.side-chat-options .menu-popover { background: var(--surface); border-radius: 8px; }').length, 1);
    assert.deepEqual(checkMenuOwnership('src/styles/menus.css', '.menu-popover { background: var(--menu-bg); }'), []);
});
test('menu ownership guard permits anchor placement and the distinct reaction picker', () => {
    assert.deepEqual(checkMenuOwnership('src/styles/app.css', '.chat-nav-row .menu-popover { left: 100px; top: auto; }'), []);
    assert.deepEqual(checkMenuOwnership('src/styles/approvals.css', '.reaction-menu .menu-popover { border-radius: 40px; }'), []);
});
