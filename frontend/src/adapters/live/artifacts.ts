// Artifacts and files, from `artifact_listed` events and the
// content `open_artifact` fetches, mapped onto the existing artifact views.
//
// Content is data. An HTML artifact is shown as its source, as code, never
// rendered. Text the viewer renders as markdown arrives with its link forms
// broken apart, so artifact text can neither link out nor open anything.
// Images are inline bytes of a named type, shown from a data URL built here,
// so showing one fetches nothing.
import type { Artifact, ArtifactFormat, ArtifactKind, DownloadPayload } from '../../protocol/models.js';
import type { ArtifactBody, FileFormat, ImageType, InlineImage, UiArtifact } from './protocol/generated.js';
import { inertLinks } from './text.js';
const FORMATS: Readonly<Record<FileFormat, readonly [
    ArtifactKind,
    ArtifactFormat
]>> = {
    markdown: ['document', 'markdown'],
    text: ['document', 'text'],
    csv: ['document', 'csv'],
    code: ['document', 'code'],
    // Source, never a rendered page.
    html: ['document', 'code'],
    image: ['media', 'image'],
    pdf: ['document', 'pdf'],
    document: ['document', 'word'],
    spreadsheet: ['document', 'spreadsheet'],
};
/** The formats the viewer shows as plain text; every other one goes through the markdown renderer. */
const PLAIN: ReadonlySet<ArtifactFormat> = new Set(['code', 'text', 'csv', 'spreadsheet']);
const MEDIA_TYPES: Readonly<Record<ImageType, string>> = { png: 'image/png', jpeg: 'image/jpeg', webp: 'image/webp', gif: 'image/gif' };
const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/u;
/** A data URL for an inline image, or `null` if its bytes are not base64. */
export function imageUrl(image: InlineImage): string | null {
    return BASE64.test(image.base64) ? `data:${MEDIA_TYPES[image.media_type]};base64,${image.base64}` : null;
}
/** An inline image from a data URL the user's device produced, or `undefined` if it is not one of the image types. */
export function imageFromUrl(url: string): InlineImage | undefined {
    const match = /^data:image\/(png|jpeg|webp|gif);base64,([A-Za-z0-9+/]*={0,2})$/u.exec(url);
    const type = match?.[1];
    const base64 = match?.[2];
    return (type === 'png' || type === 'jpeg' || type === 'webp' || type === 'gif') && base64 !== undefined ? { media_type: type, base64 } : undefined;
}
/**
 * The wire format for a file the user adds, or `undefined` for one the app
 * cannot send: the wire carries a file's text, or an image.
 */
export function fileFormat(format: ArtifactFormat): FileFormat | undefined {
    switch (format) {
        case 'markdown':
        case 'text':
        case 'csv':
        case 'code':
        case 'html':
        case 'image':
            return format;
        case 'pdf':
        case 'word':
        case 'spreadsheet':
        case 'archive':
        case 'slides':
        case 'app':
        case 'video':
        case 'audio':
            return undefined;
    }
}
function megabytes(bytes: number): string {
    return `${Number((bytes / 1048576).toFixed(1))} MB`;
}
interface Shelved {
    listing: UiArtifact;
    readonly bodies: Map<number, ArtifactBody>;
}
/** What the views show for one revision's content. */
function shown(listing: UiArtifact, body: ArtifactBody | undefined): Pick<Artifact, 'content' | 'asset' | 'description'> {
    const [, format] = FORMATS[listing.format];
    switch (body?.type) {
        case undefined:
            return { content: '', asset: null, description: 'Open it to load its content.' };
        case 'text':
            return { content: PLAIN.has(format) ? body.text : inertLinks(body.text), asset: null, description: '' };
        case 'image':
            return { content: '', asset: imageUrl(body), description: '' };
        case 'unpreviewable':
            return { content: '', asset: null, description: `This file (${megabytes(body.bytes)}) has no preview in the app yet.` };
        case 'too_large':
            return { content: '', asset: null, description: `This file is too large to show here: ${megabytes(body.bytes)}, and the limit is ${megabytes(body.limit)}.` };
    }
}
/** The artifacts the broker has listed, with the content fetched so far. */
export class ArtifactShelf {
    readonly #items = new Map<string, Shelved>();
    /** A listing replaces the last one; content already fetched for a revision is kept. */
    list(listing: UiArtifact): void {
        const item = this.#items.get(listing.artifact_id);
        if (item === undefined)
            this.#items.set(listing.artifact_id, { listing, bodies: new Map() });
        else
            item.listing = listing;
    }
    /** The newest revision, or `undefined` for an artifact the broker never listed. */
    latest(artifactId: string): number | undefined {
        return this.#items.get(artifactId)?.listing.revisions.at(-1)?.revision;
    }
    /** Whether the broker listed this revision of this artifact. */
    hasRevision(artifactId: string, revision: number): boolean {
        return this.#items.get(artifactId)?.listing.revisions.some(entry => entry.revision === revision) ?? false;
    }
    /** Keep one revision's content. */
    keep(artifactId: string, revision: number, body: ArtifactBody): void {
        this.#items.get(artifactId)?.bodies.set(revision, body);
    }
    /** The newest revision's content, if it was fetched. */
    body(artifactId: string): ArtifactBody | undefined {
        const item = this.#items.get(artifactId);
        if (item === undefined)
            return undefined;
        const latest = item.listing.revisions.at(-1);
        return latest === undefined ? undefined : item.bodies.get(latest.revision);
    }
    /** A download of the newest revision's content, if it was fetched and can be saved. */
    download(artifactId: string): DownloadPayload | undefined {
        const item = this.#items.get(artifactId);
        const body = this.body(artifactId);
        if (item === undefined || body === undefined)
            return undefined;
        const name = item.listing.title.replace(/[^A-Za-z0-9._ -]/gu, '').trim() || 'download';
        if (body.type === 'text')
            return { filename: name, mime: item.listing.format === 'markdown' ? 'text/markdown' : 'text/plain', text: body.text, asset: null };
        if (body.type !== 'image')
            return undefined;
        const asset = imageUrl(body);
        return asset === null ? undefined : { filename: name, mime: MEDIA_TYPES[body.media_type], text: null, asset };
    }
    /** Fresh artifact records, in the order each was first listed. */
    artifacts(): Artifact[] {
        return [...this.#items.values()].map(({ listing, bodies }) => {
            const [kind, format] = FORMATS[listing.format];
            const first = listing.revisions[0];
            const latest = listing.revisions.at(-1);
            return {
                id: listing.artifact_id,
                title: listing.title,
                kind,
                format,
                ...shown(listing, latest === undefined ? undefined : bodies.get(latest.revision)),
                created: first?.created_at ?? '',
                opened: '',
                pinned: false,
                trashUntil: null,
                version: latest?.revision ?? 0,
                publishedVersion: listing.shared_revision ?? null,
                slides: [],
            };
        });
    }
}
