import test from 'node:test';
import assert from 'node:assert/strict';
import { ComposerDrafts } from '../../dist/app/features/chat/drafts.js';
test('drafts and attachments belong to the originating chat', () => {
    const drafts = new ComposerDrafts();
    drafts.save('main', 'Main note', ['a']);
    drafts.save('side', 'Side note', ['b']);
    assert.deepEqual(drafts.read('main'), { text: 'Main note', attachments: ['a'] });
    assert.deepEqual(drafts.read('side'), { text: 'Side note', attachments: ['b'] });
});
test('a failed request cannot overwrite a newer draft', () => {
    const drafts = new ComposerDrafts();
    drafts.save('side', 'New thought', ['new']);
    assert.deepEqual(drafts.recover('side', 'Unsent earlier message', ['old']), { text: 'Unsent earlier message\n\nNew thought', attachments: ['old', 'new'] });
});
test('late recovery into an inactive chat does not touch the current chat', () => {
    const drafts = new ComposerDrafts();
    drafts.save('main', 'Unrelated note', ['main-file']);
    drafts.recover('side', 'Recovered note', ['side-file']);
    assert.deepEqual(drafts.read('main'), { text: 'Unrelated note', attachments: ['main-file'] });
    assert.deepEqual(drafts.read('side'), { text: 'Recovered note', attachments: ['side-file'] });
});
test('failed attachment recovery does not replay already-sent text or duplicate files', () => {
    const drafts = new ComposerDrafts();
    drafts.save('main', 'Current draft', ['same']);
    assert.deepEqual(drafts.recover('main', '', ['same', 'failed']), { text: 'Current draft', attachments: ['same', 'failed'] });
});
test('draft reads and writes never expose the mutable array', () => {
    const drafts = new ComposerDrafts(), files = ['a'];
    drafts.save('main', 'Hello', files);
    files.push('b');
    const first = drafts.read('main');
    first.attachments.push('c');
    assert.deepEqual(drafts.read('main').attachments, ['a']);
});
test('duplicate text recovery and empty initial state stay well-defined', () => {
    const drafts = new ComposerDrafts();
    assert.deepEqual(drafts.read('new'), { text: '', attachments: [] });
    drafts.recover('new', 'Hello', []);
    drafts.recover('new', 'Hello', []);
    assert.equal(drafts.read('new').text, 'Hello');
});
