import { h } from './dom.js';
export interface MarkdownOptions {
    allowHeadings?: boolean;
    openArtifact?: (id: string) => void;
}
/** Only explicit http(s)/mailto links are navigable. HTML and embedded images are always plain text. */
export function safeLink(value: string): string | null {
    if (/[\u0000-\u0020\u007f]/u.test(value))
        return null;
    try {
        const url = new URL(value);
        return ['https:', 'http:', 'mailto:'].includes(url.protocol) ? url.href : null;
    }
    catch {
        return null;
    }
}
export function inlineText(text: string, options: MarkdownOptions = {}): DocumentFragment {
    const fragment = document.createDocumentFragment();
    const pattern = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/gu;
    let offset = 0;
    for (const match of text.matchAll(pattern)) {
        const part = match[0];
        const index = match.index;
        fragment.append(document.createTextNode(text.slice(offset, index)));
        if (part.startsWith('`'))
            fragment.append(h('code', {}, part.slice(1, -1)));
        else if (part.startsWith('**'))
            fragment.append(h('strong', {}, part.slice(2, -2)));
        else if (part.startsWith('*'))
            fragment.append(h('em', {}, part.slice(1, -1)));
        else {
            const split = part.indexOf('](');
            const label = part.slice(1, split);
            const target = part.slice(split + 2, -1);
            if (target.startsWith('artifact:') && options.openArtifact) {
                const control = h('button', { class: 'markdown-artifact-link', type: 'button' }, label);
                control.addEventListener('click', () => options.openArtifact?.(target.slice(9)));
                fragment.append(control);
            }
            else {
                const href = safeLink(target);
                fragment.append(href ? h('a', { href, target: '_blank', rel: 'noopener noreferrer' }, label) : document.createTextNode(label));
            }
        }
        offset = index + part.length;
    }
    fragment.append(document.createTextNode(text.slice(offset)));
    return fragment;
}
/** Line-state parser keeps fenced commands intact across empty lines. No raw HTML, eval or DOM injection. */
export function markdown(text: string, allowHeadings = true, options: MarkdownOptions = {}): HTMLElement {
    const root = h('div', { class: 'markdown' });
    const lines = text.replaceAll('\r\n', '\n').split('\n');
    let index = 0;
    const special = (line: string): boolean => /^(?:```|#{1,6} |[-*•] |\d+\. |> ?|\|)/u.test(line);
    while (index < lines.length) {
        const line = lines[index] ?? '';
        if (!line.trim()) {
            index++;
            continue;
        }
        if (line.startsWith('```')) {
            const language = line.slice(3).trim().replace(/[^\w+-]/gu, '');
            const content: string[] = [];
            index++;
            while (index < lines.length && !(lines[index] ?? '').startsWith('```')) {
                content.push(lines[index] ?? '');
                index++;
            }
            if (index < lines.length)
                index++;
            root.append(h('pre', { 'data-language': language || 'text' }, h('code', {}, content.join('\n'))));
            continue;
        }
        const heading = /^(#{1,6}) (.*)$/u.exec(line);
        if (heading) {
            const title = heading[2] ?? '';
            const level = heading[1]?.length ?? 1;
            const element = !allowHeadings ? h('p', {}, inlineText(title, options)) : level === 1 ? h('h1', {}, inlineText(title, options)) : level === 2 ? h('h2', {}, inlineText(title, options)) : h('h3', {}, inlineText(title, options));
            root.append(element);
            index++;
            continue;
        }
        if (/^\|.*\|$/u.test(line) && /^\|[\s:|-]+\|$/u.test(lines[index + 1] ?? '')) {
            const cells = (value: string): string[] => value.replace(/^\||\|$/gu, '').split('|').map((cell) => cell.trim());
            const table = h('table', {}, h('thead', {}, h('tr', {}, ...cells(line).map((cell) => h('th', { scope: 'col' }, inlineText(cell, options))))));
            const body = h('tbody');
            index += 2;
            while (index < lines.length && (lines[index] ?? '').startsWith('|')) {
                body.append(h('tr', {}, ...cells(lines[index] ?? '').map((cell) => h('td', {}, inlineText(cell, options)))));
                index++;
            }
            table.append(body);
            root.append(h('div', { class: 'markdown-table' }, table));
            continue;
        }
        if (/^[-*•] |^\d+\. /u.test(line)) {
            const ordered = /^\d+\. /u.test(line);
            const list = ordered ? h('ol') : h('ul');
            while (index < lines.length && (ordered ? /^\d+\. /u : /^[-*•] /u).test(lines[index] ?? '')) {
                list.append(h('li', {}, inlineText((lines[index] ?? '').replace(/^([-*•]|\d+\.) /u, ''), options)));
                index++;
            }
            root.append(list);
            continue;
        }
        if (line.startsWith('>')) {
            const quote: string[] = [];
            while ((lines[index] ?? '').startsWith('>')) {
                quote.push((lines[index] ?? '').replace(/^> ?/u, ''));
                index++;
            }
            root.append(h('blockquote', {}, inlineText(quote.join('\n'), options)));
            continue;
        }
        const paragraph = [line];
        index++;
        while (index < lines.length && (lines[index] ?? '').trim() && !special(lines[index] ?? '')) {
            paragraph.push(lines[index] ?? '');
            index++;
        }
        root.append(h('p', {}, inlineText(paragraph.join('\n'), options)));
    }
    return root;
}
