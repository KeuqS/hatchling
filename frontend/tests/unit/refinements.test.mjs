import test from 'node:test';
import assert from 'node:assert/strict';
import { floatingPosition } from '../../dist/app/components/floating.js';
import { formatMarkdown } from '../../dist/app/features/library/markdown-editor.js';
import { visibleSystemRows } from '../../dist/app/features/library/system.js';
import { createFixtures } from '../../dist/app/adapters/fixtures.js';
const viewport = { width: 800, height: 600 };
test('floating chrome flips below a close button at the viewport top', () => {
    assert.deepEqual(floatingPosition({ left: 756, top: 8, width: 28, height: 28 }, { width: 120, height: 32 }, viewport), { left: 672, top: 44 });
});
test('floating chrome clamps both horizontal edges and stays above a bottom control', () => {
    assert.deepEqual(floatingPosition({ left: 0, top: 570, width: 20, height: 20 }, { width: 150, height: 30 }, viewport), { left: 8, top: 532 });
});
test('rail tooltips prefer right without exceeding a narrow viewport', () => {
    assert.deepEqual(floatingPosition({ left: 10, top: 100, width: 30, height: 30 }, { width: 150, height: 30 }, viewport, 'right'), { left: 48, top: 100 });
    const point = floatingPosition({ left: 270, top: 100, width: 30, height: 30 }, { width: 200, height: 30 }, { width: 320, height: 600 }, 'right');
    assert.ok(point.left >= 8 && point.left + 200 <= 312);
});
test('floating placement is finite for very short viewports and oversized content', () => {
    const position = floatingPosition({ left: 999, top: 999, width: 0, height: 0 }, { width: 1000, height: 1000 }, { width: 160, height: 80 });
    assert.deepEqual(position, { left: 8, top: 8 });
});
test('Markdown bold wraps a selection and restores it without cursor drift', () => {
    const edit = formatMarkdown('hello world', 6, 11, 'bold');
    assert.deepEqual(edit, { value: 'hello **world**', start: 8, end: 13 });
    assert.deepEqual(formatMarkdown(edit.value, edit.start, edit.end, 'bold'), { value: 'hello world', start: 6, end: 11 });
});
test('Markdown italic at an empty caret provides a position between delimiters', () => {
    assert.deepEqual(formatMarkdown('abc', 1, 1, 'italic'), { value: 'a**bc', start: 2, end: 2 });
});
test('Markdown headings replace existing list or heading prefixes', () => {
    assert.equal(formatMarkdown('one\n- two\nthree', 6, 8, 'h2').value, 'one\n## two\nthree');
    assert.equal(formatMarkdown('### title', 0, 9, 'h1').value, '# title');
});
test('Markdown formatting toggles an existing heading and clamps invalid bounds', () => {
    assert.equal(formatMarkdown('# title', -9, 99, 'h1').value, 'title');
    assert.deepEqual(formatMarkdown('abc', 100, 101, 'bold'), { value: 'abc****', start: 5, end: 5 });
});
test('Markdown numbered lists number each selected line and toggle off together', () => {
    const ordered = formatMarkdown('alpha\nbeta\ngamma', 0, 16, 'number');
    assert.equal(ordered.value, '1. alpha\n2. beta\n3. gamma');
    assert.equal(formatMarkdown(ordered.value, ordered.start, ordered.end, 'number').value, 'alpha\nbeta\ngamma');
});
test('Markdown formatting treats HTML as text and never evaluates source', () => {
    const source = '<img src=x onerror=alert(1)>';
    assert.equal(formatMarkdown(source, 0, source.length, 'bold').value, `**${source}**`);
});
test('file tree keeps collapsed roots in place and expands metadata children at depth one', () => {
    const state = createFixtures();
    const before = structuredClone(state);
    assert.equal(visibleSystemRows(state, '', '', 'asc', new Set()).length, 16);
    const expanded = visibleSystemRows(state, '', '', 'asc', new Set(['dir-Downloads']));
    assert.equal(expanded.length, 21);
    const files = expanded.filter((row) => row.depth === 1);
    assert.equal(files.length, 5);
    assert.ok(files.every(({ entry }) => entry.reference.type === 'GZ' && entry.bytes === null && !entry.artifactId));
    assert.deepEqual(state, before);
});
test('independent file disclosures retain paths and do not show trashed artifacts', () => {
    const state = createFixtures();
    state.artifacts.find((artifact) => artifact.id === 'backlog').trashUntil = '2026-10-25';
    const rows = visibleSystemRows(state, '', '', 'asc', new Set(['dir-docs', 'dir-Downloads']));
    assert.ok(!rows.some(({ entry }) => entry.artifactId === 'backlog'));
    assert.equal(rows.filter(({ entry }) => entry.reference).length, 5);
});
test('file traversal ignores disconnected cycles and unknown expansions', () => {
    const state = createFixtures();
    state.systemEntries.push({ id: 'cycle', parentId: 'cycle', name: 'loop', kind: 'folder', visibility: 'internal', modified: null, bytes: null, description: 'Defensive fixture' });
    assert.equal(visibleSystemRows(state, '', '', 'asc', new Set(['cycle', 'missing'])).length, 16);
});
