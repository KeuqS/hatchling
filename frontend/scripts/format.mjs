import ts from 'typescript';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const excluded = new Set(['node_modules', 'dist', 'preview', '.git', 'artifacts', '__pycache__']);
const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((entry) => excluded.has(entry.name) ? [] : entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]);
const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed, removeComments: false });
let failures = 0;
for (const path of walk('.').filter((file) => /\.(ts|mjs|css|html|json|md|yml|py)$/.test(file))) {
    const text = readFileSync(path, 'utf8');
    let clean = text;
    if (/\.(ts|mjs)$/.test(path)) {
        const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, path.endsWith('.ts') ? ts.ScriptKind.TS : ts.ScriptKind.JS);
        clean = printer.printFile(source);
        for (let pass = 0; pass < 3; pass++) {
            const next = printer.printFile(ts.createSourceFile(path, clean, ts.ScriptTarget.Latest, true, path.endsWith('.ts') ? ts.ScriptKind.TS : ts.ScriptKind.JS));
            if (next === clean)
                break;
            clean = next;
        }
    }
    else if (path.endsWith('.json'))
        clean = JSON.stringify(JSON.parse(text), null, 2);
    clean = clean.replace(/\r\n/g, '\n').replace(/[\t ]+$/gm, '').replace(/\n*$/, '\n');
    if (text !== clean) {
        if (process.argv.includes('--write'))
            writeFileSync(path, clean);
        else {
            console.error(`${path}: run npm run format`);
            failures++;
        }
    }
}
if (failures)
    process.exitCode = 1;
else
    console.log('Formatting passed (TypeScript printer + JSON + whitespace hygiene).');
