import test from 'node:test';
import assert from 'node:assert/strict';
import { authorization, contractFields, lintSource } from '../../scripts/lint.mjs';
const anywhere = 'src/features/example/view.ts';
const live = 'src/adapters/live/transport.ts';
const generated = 'src/adapters/live/protocol/generated.ts';
const flagged = (path, text, pattern) => assert.match(lintSource(path, text).join('\n'), pattern, text);
const clean = (path, text) => assert.deepEqual(lintSource(path, text), [], text);
test('strict rules hold everywhere', () => {
    flagged(anywhere, 'const same = (a: number, b: number): boolean => a == b;', /Loose equality/);
    flagged(anywhere, 'const differ = (a: number, b: number): boolean => a != b;', /Loose equality/);
    clean(anywhere, 'const same = (a: number, b: number): boolean => a === b && a !== -b;');
    flagged(anywhere, "console.log('x');", /Console output/);
    flagged(anywhere, 'export default 1;', /Default exports/);
    flagged(anywhere, 'export default function f(): number { return 1; }', /Default exports/);
    flagged(anywhere, 'var x = 1;', /var is forbidden/);
    clean(anywhere, 'let x = 1; const y = x;');
    flagged(anywhere, 'const f: Function = () => 1;', /Function and Object types/);
    flagged(anywhere, 'const o: Object = {};', /Function and Object types/);
    flagged(anywhere, 'debugger;', /debugger/);
    flagged(anywhere, 'for (const key in {}) {}', /for\.\.\.in/);
    flagged(anywhere, 'const o: { a?: number } = {}; delete o.a;', /delete is forbidden/);
    flagged(anywhere, 'outer: for (;;) { break outer; }', /Labels/);
    flagged(anywhere, 'export function f() { return 1; }', /declare their return type/);
    clean(anywhere, 'export function f(): number { return 1; }\nfunction g() { return 2; }');
    flagged(anywhere, '// @ts-expect-error\nconst x: number = "a";', /Suppression comments/);
});
test('the live adapter may not swallow errors or blur conditions', () => {
    flagged(live, 'try { work(); } catch { }', /Empty catch/);
    flagged(live, 'const yes = !!value;', /Double negation/);
    clean(live, 'try { work(); } catch (error) { report(error); }\nconst yes = value !== undefined;');
    // The same code elsewhere is held to the repository-wide rules only.
    clean(anywhere, 'try { work(); } catch { }\nconst yes = !!value;');
});
test('the live adapter carries no authorization logic: the broker owns every decision', () => {
    for (const source of [
        'const authorized = true;',
        'function authorize(): void {}',
        'const permissions: string[] = [];',
        'type Privilege = string;',
        'const allowList = new Set<string>();',
        'const deny_list = 1;',
        'const credentials = 1;',
        'function isAllowed(): boolean { return true; }',
        'const canAccess = 1;',
        'const accessControl = 1;',
        'const acl = 1;',
        'function checkAcl(): void {}',
        'const authenticated = 1;',
    ])
        flagged(live, source, /Authorization belongs to the broker/);
    clean(live, 'const oracle = 1; const miracle = oracle; const decision = miracle; const approve = decision;');
    // Elsewhere, and in the generated wire types, those words are data.
    clean(anywhere, 'const permissions: string[] = [];');
    clean(generated, 'export type Grant = { readonly permissions: string; readonly authorized: boolean; };');
    assert.equal(authorization.test('decide'), false);
});
test('the live adapter may fill the frozen contract fields, and nothing more ', () => {
    // The domain contract names these fields; a complete snapshot must carry them.
    clean(live, 'const state = { permissions: [], websitePermissions: [], networkPermissions: [], settings: { allowlist: [] } };');
    for (const name of contractFields)
        assert.ok(authorization.test(name), `${name} would otherwise be flagged`);
    // Reading one, computing with one, or naming anything else so is still a decision.
    for (const source of [
        'const n = state.permissions.length;',
        'const { allowlist } = settings;',
        'const value = { allowlist };',
        'const value = { permission: [] };',
        'const value = { grantPermissions: [] };',
        'function permissions(): void {}',
    ])
        flagged(live, source, /Authorization belongs to the broker/);
});
