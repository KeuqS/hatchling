import test from 'node:test';
import assert from 'node:assert/strict';
import { chatBreadcrumb, resolveChatId } from '../../dist/app/features/chat/chat-header.js';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
test('Main chat always says Chats, independently of its stored title', () => {
    assert.deepEqual(chatBreadcrumb({ id: 'main', title: 'A backend supplied title' }), { label: 'Chats', isMain: true, description: 'Main chat' });
});
test('side chat uses its actual title and is not the agent identity', () => {
    assert.deepEqual(chatBreadcrumb({ id: 'side', title: 'Human language improvement backlog' }), { label: 'Human language improvement backlog', isMain: false, description: 'Side chat: Human language improvement backlog' });
});
test('a side chat named Main chat still gets the side chat Back affordance', () => {
    assert.equal(chatBreadcrumb({ id: 'not-main', title: 'Main chat' }).isMain, false);
});
test('missing snapshot entry has neutral Chats navigation without inventing a title', () => {
    assert.deepEqual(chatBreadcrumb(undefined), { label: 'Chats', isMain: true, description: 'Main chat' });
});
test('stale chat links resolve explicitly to main, not the first array element', () => {
    const chats = [{ id: 'private-channel' }, { id: 'main' }, { id: 'side' }];
    assert.equal(resolveChatId(chats, 'side'), 'side');
    assert.equal(resolveChatId(chats, 'deleted'), 'main');
    assert.equal(resolveChatId(chats, '__proto__'), 'main');
});
test('no implicit recipient exists when Main and the requested chat are both absent', () => {
    assert.equal(resolveChatId([{ id: 'different' }], 'missing'), undefined);
    assert.equal(resolveChatId([], 'missing'), undefined);
});
test('unicode and markup-like titles stay unchanged as text data', () => {
    const title = 'Notes 日本語 <script>alert(1)</script>';
    assert.equal(chatBreadcrumb({ id: 'j', title }).label, title);
});
test('the protocol is byte-for-byte unchanged from v8', () => {
    const manifest = JSON.parse(readFileSync('docs/PROTOCOL-LOCK.json', 'utf8'));
    assert.equal(manifest.adapterExport, 'ProtocolAdapter');
    for (const [path, expected] of Object.entries(manifest.files))
        assert.equal(createHash('sha256').update(readFileSync(path)).digest('hex'), expected, path);
});
