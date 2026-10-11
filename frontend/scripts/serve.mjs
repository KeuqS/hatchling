import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
const root = resolve('dist');
const port = Number(process.env.PORT ?? 4173);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.json': 'application/json', '.md': 'text/markdown', '.csv': 'text/csv', '.pdf': 'application/pdf', '.wav': 'audio/wav', '.webm': 'video/webm', '.gz': 'application/gzip' };
createServer(async (req, res) => {
    try {
        const pathname = decodeURIComponent(new URL(req.url ?? '/', 'http://localhost').pathname);
        let file = resolve(root, `.${pathname}`);
        if (file !== root && !file.startsWith(root + sep)) {
            res.writeHead(403).end();
            return;
        }
        if ((await stat(file)).isDirectory())
            file = resolve(file, 'index.html');
        const content = await readFile(file);
        res.writeHead(200, { 'Content-Type': types[extname(file)] ?? 'application/octet-stream', 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' });
        res.end(content);
    }
    catch {
        res.writeHead(404, { 'Content-Type': 'text/plain' }).end('Not found');
    }
}).listen(port, '0.0.0.0', () => console.log(`Hatchling: http://localhost:${port}`));
