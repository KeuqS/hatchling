import type { AppState, Artifact, Command, CommandResult, DownloadPayload } from '../../protocol/models.js';
import { canPublish, editableFormat, safeFilename } from '../../protocol/rules.js';
import { artifact } from '../fixtures.js';
import { fail, nextId, ok } from './shared.js';
function escape(text: string): string { return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;'); }
function slideDownload(item: Artifact): string {
    return `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>${escape(item.title)}</title><style>body{margin:0;background:#181818;color:#fff;font-family:Arial,sans-serif}section{position:relative;aspect-ratio:16/9;max-width:1100px;margin:40px auto;background:#243531;overflow:hidden;break-after:page}p{position:absolute;margin:0;white-space:pre-wrap}@media print{section{margin:0;width:100vw}body{background:white}}</style>${item.slides.map((slide) => `<section aria-label="Slide">${slide.map((layer) => `<p style="left:${layer.x}%;top:${layer.y}%;width:${layer.width}%;font-size:${layer.size}px">${escape(layer.text)}</p>`).join('')}</section>`).join('')}</html>`;
}
export function getDownload(item: Artifact): DownloadPayload {
    if (item.format === 'archive' && item.asset)
        return { filename: `${safeFilename(item.title)}.tar.gz`, mime: 'application/gzip', text: null, asset: item.asset };
    if (item.asset) {
        const mime = item.asset.startsWith('data:') ? item.asset.slice(5, item.asset.indexOf(';')) : (item.format === 'pdf' ? 'application/pdf' : item.format === 'image' ? 'image/png' : item.format === 'audio' ? 'audio/wav' : item.format === 'video' ? 'video/webm' : 'application/octet-stream');
        const extension = mime === 'image/jpeg' ? 'jpg' : mime === 'image/webp' ? 'webp' : mime === 'image/png' ? 'png' : mime === 'application/pdf' ? 'pdf' : mime === 'audio/wav' ? 'wav' : mime === 'video/webm' ? 'webm' : 'bin';
        return { filename: `${safeFilename(item.title.replace(/\.[a-z]+$/i, ''))}.${extension}`, mime, text: null, asset: item.asset };
    }
    if (item.format === 'slides')
        return { filename: `${safeFilename(item.title)}.html`, mime: 'text/html', text: slideDownload(item), asset: null };
    const ext = item.format === 'markdown' ? 'md' : item.format === 'csv' ? 'csv' : item.format === 'code' ? 'ts' : 'txt';
    const hasCodeExtension = item.format === 'code' && /\.[a-z\d]{1,10}$/iu.test(item.title);
    const title = hasCodeExtension || item.title.endsWith(`.${ext}`) ? item.title : `${item.title}.${ext}`;
    return { filename: safeFilename(title), mime: ext === 'csv' ? 'text/csv' : 'text/plain', text: item.content, asset: null };
}
export function reduceContent(state: AppState, command: Command): CommandResult | null {
    switch (command.type) {
        case 'artifact.import': {
            if (!command.title.trim() || command.dataUrl.length > 3000000)
                return fail('Choose a local file under 2 MB.');
            if (command.dataUrl && !/^data:(image\/(png|jpeg|webp)|application\/pdf|audio\/[a-z0-9.+-]+|video\/webm);base64,/i.test(command.dataUrl))
                return fail('This preview type is not allowed. Use a text, image, PDF, or recorded media file.');
            const id = nextId(state, 'import');
            const item = artifact(id, command.title, command.format, command.content, 'Imported locally by you; not uploaded to a service.');
            item.asset = command.dataUrl || null;
            state.artifacts.unshift(item);
            return ok('File added locally.', id);
        }
        case 'artifact.create': {
            if (!command.title.trim())
                return fail('Give the artifact a title.');
            if (!['markdown', 'html', 'app', 'slides', 'csv'].includes(command.format))
                return fail('Choose an available sample creation format.');
            const id = nextId(state, 'artifact');
            const item = artifact(id, command.title.trim(), command.format, `# ${command.title.trim()}\n\nA new sample document in your local workspace.\n\n## Purpose\n\nCapture the decision and the evidence that supports it.\n\n## Next step\n\nReview the scope, make the first useful change, and keep the result.`, 'Created in this scripted workspace.');
            if (command.format === 'slides')
                item.slides = [[{ id: 'heading', text: command.title.trim(), x: 8, y: 25, width: 84, size: 40 }, { id: 'body', text: 'A clear idea. A small next step.', x: 8, y: 65, width: 84, size: 22 }]];
            if (command.format === 'csv')
                item.content = 'Milestone,Status,Evidence\nScope agreed,Ready,Review notes\nFirst useful result,Planned,Acceptance checklist';
            if (command.format === 'app')
                item.content = 'Define the small next step\nReview the result\nKeep the evidence';
            state.artifacts.unshift(item);
            return ok('Artifact created in the local workspace.', id);
        }
        case 'artifact.open':
        case 'artifact.pin':
        case 'artifact.trash':
        case 'artifact.restore':
        case 'artifact.download': {
            const item = state.artifacts.find((entry) => entry.id === command.id);
            if (!item)
                return fail('This artifact is no longer available.');
            if (command.type === 'artifact.open') {
                item.opened = `${state.clock}-${state.revision}`;
                return ok('', item.id);
            }
            if (command.type === 'artifact.pin') {
                item.pinned = !item.pinned;
                return ok(item.pinned ? 'Pinned to the top of your library.' : 'Unpinned.');
            }
            if (command.type === 'artifact.trash') {
                item.trashUntil = '2026-10-23';
                return ok('Moved to Trash. You can restore it for 30 days in this demo.');
            }
            if (command.type === 'artifact.restore') {
                item.trashUntil = null;
                return ok('Restored to your library.');
            }
            if (item.kind === 'app' || item.kind === 'page')
                return fail('Hosted pages and interactive apps open live. Only document and media artifacts have offline downloads.');
            return { ok: true, message: 'Download ready.', download: getDownload(item) };
        }
        case 'artifact.app-toggle': {
            const item = state.artifacts.find((entry) => entry.id === command.id);
            if (item?.kind !== 'app')
                return fail('This is not an interactive app.');
            const lines = item.content.split('\n');
            const line = lines[command.index];
            if (line === undefined)
                return fail('Item not found.');
            lines[command.index] = line.startsWith('[x] ') ? line.slice(4) : `[x] ${line}`;
            item.content = lines.join('\n');
            return ok('App data saved privately in the local adapter.');
        }
        case 'artifact.save': {
            const item = state.artifacts.find((entry) => entry.id === command.id);
            if (!item)
                return fail('Artifact not found.');
            if (item.format !== 'markdown')
                return fail('This format is view-only. Ask the agent for a revision or download it for local editing.');
            item.content = command.content;
            item.version++;
            return ok('Saved locally. An existing published snapshot was not updated.');
        }
        case 'artifact.save-slide': {
            const item = state.artifacts.find((entry) => entry.id === command.id);
            const layer = item?.slides[command.slide]?.find((entry) => entry.id === command.layerId);
            if (!item || !editableFormat(item.format) || !layer)
                return fail('The selected slide layer is not available.');
            layer.text = command.text;
            layer.width = Math.max(10, Math.min(100, command.width));
            layer.x = Math.max(0, Math.min(100 - layer.width, command.x));
            layer.y = Math.max(0, Math.min(85, command.y));
            layer.size = Math.max(12, Math.min(80, command.size));
            item.version++;
            return ok('Slide layer saved. Its offline deck download has been rebuilt.');
        }
        case 'artifact.publish': {
            const item = state.artifacts.find((entry) => entry.id === command.id);
            if (!item)
                return fail('Artifact not found.');
            if (!canPublish(item.kind))
                return fail('Interactive apps and their private data can never be published.');
            if (item.version !== command.approvedVersion)
                return fail('The artifact changed. Review the current version and approve again.');
            item.publishedVersion = command.approvedVersion;
            const id = `${item.id}-v${item.version}`;
            state.publications = state.publications.filter((publication) => publication.id !== id);
            state.publications.push({ id, artifactId: item.id, title: item.title, content: item.content, version: item.version });
            return ok('A local snapshot preview was approved. Public hosting requires a live adapter.', id);
        }
        default: return null;
    }
}
