/** Exercise the actual static server without external requests or browser-policy changes. */
import { spawn } from 'node:child_process';
import { readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
const port = Number(process.env.SMOKE_PORT ?? 4179);
const origin = `http://127.0.0.1:${port}`;
const child = spawn(process.execPath, ['scripts/serve.mjs'], { env: { ...process.env, PORT: String(port) }, stdio: ['ignore', 'pipe', 'pipe'] });
const walk = async (dir) => (await Promise.all((await readdir(dir, { withFileTypes: true })).map((entry) => entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]))).flat();
try {
    await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Static server did not start in time.')), 5000);
        child.once('error', reject);
        child.once('exit', (code) => reject(new Error(`Static server exited: ${code}`)));
        child.stdout.once('data', () => { clearTimeout(timer); resolve(); });
    });
    const requests = [];
    for (const file of await walk('dist')) {
        if (file.endsWith('.map') || file.endsWith('.d.ts'))
            continue;
        const path = file.replace(/^dist\//, '');
        const response = await fetch(`${origin}/${path}`);
        requests.push({ path, status: response.status, content_type: response.headers.get('content-type'), bytes: (await response.arrayBuffer()).byteLength });
    }
    const traversal = await fetch(`${origin}/..%2fpackage.json`);
    const missing = await fetch(`${origin}/definitely-missing.asset`);
    const report = { static_asset_requests: requests.length, all_assets_200: requests.every((item) => item.status === 200), traversal_blocked: traversal.status === 403, missing_file_404: missing.status === 404, requests };
    await writeFile('artifacts/http-results.json', JSON.stringify(report, null, 2) + '\n');
    if (!report.all_assets_200 || !report.traversal_blocked || !report.missing_file_404)
        throw new Error('Static HTTP checks failed.');
    console.log(`PASS: ${requests.length} static assets; traversal rejected; missing asset returns 404.`);
}
finally {
    child.kill('SIGTERM');
}
