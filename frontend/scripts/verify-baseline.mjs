import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
/** Pure comparison shared by the CLI and negative tests. This does not bless a protocol change. */
export function verifyBaseline(baseline, read, { allowBootstrap = false, actualFiles = [] } = {}) {
    const errors = [];
    for (const [path, expected] of Object.entries(baseline.files)) {
        if (allowBootstrap && path === 'src/main.ts')
            continue;
        try {
            const actual = createHash('sha256').update(read(path)).digest('hex');
            if (actual !== expected)
                errors.push(`Changed frozen file: ${path}`);
        }
        catch {
            errors.push(`Missing frozen file: ${path}`);
        }
    }
    for (const path of actualFiles) {
        if (Object.hasOwn(baseline.files, path))
            continue;
        const liveFile = path === 'src/adapters/live-adapter.ts' || path.startsWith('src/adapters/live/');
        if (!liveFile)
            errors.push(`New file outside the approved adapter area: ${path}`);
    }
    return errors;
}
const walk = (directory) => readdirSync(directory, { withFileTypes: true }).flatMap(entry => entry.isDirectory() ? walk(`${directory}/${entry.name}`) : [`${directory}/${entry.name}`]);
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
    const frontend = process.argv.includes('--frontend');
    const baseline = JSON.parse(readFileSync(frontend ? 'docs/FRONTEND-LOCK.json' : 'docs/PROTOCOL-LOCK.json', 'utf8'));
    const allowBootstrap = process.argv.includes('--allow-bootstrap');
    const errors = verifyBaseline(baseline, path => readFileSync(path), { allowBootstrap: frontend && allowBootstrap, actualFiles: frontend ? walk('src') : [] });
    if (errors.length) {
        console.error(errors.join('\n'));
        console.error('Propose the change and obtain review. Do not regenerate the baseline to make this check pass.');
        process.exitCode = 1;
    }
    else {
        console.log(`${Object.keys(baseline.files).length} frozen files verified. ${frontend ? 'Presentation boundary intact.' : 'ProtocolAdapter contract unchanged from v8.'}`);
        if (frontend && allowBootstrap)
            console.log('src/main.ts was excluded explicitly. Review its factory/import diff separately.');
    }
}
