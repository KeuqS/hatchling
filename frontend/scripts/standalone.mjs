/** Build an additional self-contained preview. Native ESM in dist/ remains the development target. */
import ts from 'typescript';
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]);
const mime = { '.png': 'image/png', '.svg': 'image/svg+xml', '.pdf': 'application/pdf', '.wav': 'audio/wav', '.webm': 'video/webm', '.gz': 'application/gzip' };
const assets = walk('public').map((path) => {
    const extension = path.slice(path.lastIndexOf('.'));
    return [path.replace(/^public\//, './'), `data:${mime[extension] ?? 'application/octet-stream'};base64,${readFileSync(path).toString('base64')}`];
});
const inlineAssets = (text) => {
    for (const [path, data] of assets)
        text = text.replaceAll(path, data);
    return text;
};
const entries = walk('dist/app').filter((path) => path.endsWith('.js')).map((path) => {
    const id = path.replace(/^dist\/app\//, '');
    const input = inlineAssets(readFileSync(path, 'utf8'));
    const output = ts.transpileModule(input, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
    return `${JSON.stringify(id)}: function(module, exports, require) {\n${output}\n}`;
});
const script = `(() => {\nconst modules = {${entries.join(',\n')}};\nconst cache = Object.create(null);\nfunction load(id) {\n if (cache[id]) return cache[id].exports;\n if (!modules[id]) throw new Error('Missing preview module: ' + id);\n const module = {exports: {}}; cache[id] = module;\n const require = (relative) => {const parts = id.split('/'); parts.pop(); for (const part of relative.split('/')) {if (part === '..') parts.pop(); else if (part !== '.') parts.push(part);} return load(parts.join('/'));};\n modules[id](module, module.exports, require); return module.exports;\n}\nif (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => load('main.js'), {once: true}); else load('main.js');\n})();`;
const hash = createHash('sha256').update(script).digest('base64');
// index.html is the one stylesheet-order manifest for both previews.
const sourceHtml = readFileSync('index.html', 'utf8');
const stylePaths = [...sourceHtml.matchAll(/<link rel="stylesheet" href="\.\/styles\/([a-z-]+\.css)">/g)].map(match => `src/styles/${match[1]}`);
if (!stylePaths.length)
    throw new Error('No local stylesheets found in index.html.');
let css = stylePaths.map(path => readFileSync(path, 'utf8')).join('\n');
css = inlineAssets(css.replaceAll('../media/', './media/'));
let html = sourceHtml;
html = html.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/, `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'sha256-${hash}'; style-src 'unsafe-inline'; img-src data: blob:; media-src data: blob:; frame-src data: blob:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">`);
html = html.replace(/<link rel="stylesheet"[^>]*>/g, '').replace('<head>', `<head>\n<style>${css}</style>`);
html = inlineAssets(html).replace(/<script type="module" src="\.\/app\/main\.js"><\/script>/, () => `<script>${script}</script>`);
writeFileSync('dist/hatchling-standalone.html', html);
console.log('Built dist/hatchling-standalone.html — self-contained, no network requests.');
