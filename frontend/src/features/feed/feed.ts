import { appendContentState } from '../../components/content-state.js';
import { loadingImage } from '../../components/loading-image.js';
import type { AppContext } from '../../app/context.js';
import type { DeepReadonly, FeedPost } from '../../protocol/models.js';
import { h, valueOf } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { pictogram } from '../../components/pictograms.js';
import { button, iconButton, inlineText, markdown, menu, textareaField } from '../../components/ui.js';
export function renderFeed(ctx: AppContext): HTMLElement {
    const prompt = h('section', { class: 'feed-prompt', hidden: !ctx.ui.feedPromptOpen, 'aria-label': 'Your feed prompt' }, h('div', { class: 'eyebrow' }, 'Your feed prompt'), h('p', {}, ctx.state.settings.feedPrompt), h('div', { class: 'feed-prompt-actions' }, button('Edit', () => editPrompt(ctx), { icon: 'edit' }), button(ctx.contentLoading.read('feed').phase === 'loading' ? 'Generating…' : 'Generate', () => { void ctx.run({ type: 'feed.generate' }); }, { icon: 'refresh', variant: 'primary', disabled: ctx.contentLoading.read('feed').phase === 'loading' }), button('Done', () => { ctx.ui.feedPromptOpen = false; ctx.render(); }, { variant: 'ghost' })));
    const options = menu('Feed options', [
        { label: ctx.ui.feedPromptOpen ? 'Hide feed prompt' : 'Your feed prompt', icon: 'edit', action: () => { ctx.ui.feedPromptOpen = !ctx.ui.feedPromptOpen; ctx.render(); } },
        { label: 'Generate sample edition', icon: 'refresh', action: () => { void ctx.run({ type: 'feed.generate' }); } },
        { label: 'About this feed', icon: 'help', action: () => { ctx.dialogs.open('Synthetic sample feed', h('p', {}, 'All posts are synthetic examples. Their illustrations are original local diagrams. Highlighted phrases explain the sample; they are not sources or real news links. No personal interests were inferred.')); } },
    ]);
    const page = h('div', { class: 'page feed-page', 'data-scroll': 'feed' }, h('header', { class: 'feed-header' }, h('div', { class: 'feed-header-inner' }, h('h1', {}, 'Feed'), options)));
    const stream = h('div', { class: 'feed-stream' }, prompt);
    if (appendContentState(ctx, stream, 'feed')) {
        page.append(stream);
        return page;
    }
    let previousDate = '';
    for (const post of ctx.state.posts) {
        if (post.date !== previousDate) {
            stream.append(h('h2', { class: 'feed-date' }, post.date));
            previousDate = post.date;
        }
        stream.append(postRow(ctx, post));
    }
    stream.append(h('p', { class: 'sample-footnote' }, 'Synthetic sample feed · fictional editorial examples, not news.'));
    page.append(stream);
    return page;
}
function provenance(ctx: AppContext, post: DeepReadonly<FeedPost>): void {
    ctx.dialogs.open('Why did I see this?', h('div', { class: 'stack' }, h('p', {}, post.reason), h('p', { class: 'muted text-small' }, `${post.kicker} · ${post.category}`)));
}
/** Highlighted phrases are local provenance buttons, never fabricated outbound URLs. */
function referenceBody(ctx: AppContext, post: DeepReadonly<FeedPost>): HTMLElement {
    const body = h('div', { class: 'markdown' });
    const paragraph = h('p');
    let remaining = post.body;
    for (const phrase of post.highlights ?? []) {
        const index = remaining.indexOf(phrase);
        if (index < 0)
            continue;
        paragraph.append(inlineText(remaining.slice(0, index)), button(phrase, () => provenance(ctx, post), { variant: 'link', className: 'reference-source', title: 'Read sample note · not a news link' }));
        remaining = remaining.slice(index + phrase.length);
    }
    paragraph.append(inlineText(remaining));
    body.append(paragraph);
    return body;
}
function postRow(ctx: AppContext, post: DeepReadonly<FeedPost>): HTMLElement {
    const reacted = iconButton('heart', post.reacted ? 'Remove reaction' : 'React to post', () => { void ctx.run({ type: 'feed.react', id: post.id }, true); }, post.reacted ? 'reacted' : '');
    reacted.id = `react-${post.id}`;
    reacted.setAttribute('aria-pressed', String(post.reacted));
    const options = menu(`Post options: ${post.title}`, [
        { label: 'Why did I see this?', icon: 'help', action: () => provenance(ctx, post) },
        { label: 'Share', icon: 'share', action: () => sharePost(ctx, post) },
    ]);
    const content = h('div', { class: 'feed-post-body' }, h('h3', {}, post.title), post.reference ? referenceBody(ctx, post) : markdown(post.body));
    const media = post.media;
    if (media) {
        const picture = h('button', { class: 'feed-media', type: 'button', 'aria-label': `Enlarge image: ${post.title}` }, loadingImage({ src: media.src, alt: media.alt, width: media.width, height: media.height, lazy: true }));
        picture.addEventListener('click', () => ctx.dialogs.open('Sample illustration', h('div', { class: 'stack feed-lightbox' }, loadingImage({ src: media.src, alt: media.alt, width: media.width, height: media.height, retry: true }), h('p', { class: 'muted text-small' }, 'Original local illustration made for this synthetic feed. It does not depict an actual product or news event.'))));
        content.append(picture);
    }
    content.append(h('div', { class: 'post-actions' }, reacted, button('Discuss', () => {
        void ctx.run({ type: 'chat.create', title: post.title }, true).then((result) => {
            if (result.ok && result.id) {
                ctx.navigate('chat', result.id);
                ctx.ui.draft = `Let’s discuss “${post.title}”. ${post.reference ? 'This is a synthetic editorial example, not news. ' : ''}`;
                ctx.render();
            }
        });
    }, { icon: 'chat', variant: 'ghost' })));
    return h('article', { class: 'feed-post', 'data-post': post.id }, h('span', { class: 'feed-post-icon' }, post.icon === 'shield' ? pictogram('shield') : icon(post.icon, 38)), content, options);
}
function sharePost(ctx: AppContext, post: DeepReadonly<FeedPost>): void {
    const text = `${post.title}\n\n${post.body}${post.reference ? '\n\nSynthetic editorial example, not news.' : ''}`;
    const body = h('div', { class: 'stack' }, h('p', {}, 'Copy this post as plain text. This does not create a public link.'), textareaField('Post text', 'shareText', text, 8));
    const ref = ctx.dialogs.open('Share a post', body);
    body.append(button('Copy post', () => { void navigator.clipboard.writeText(text).then(() => { ctx.toast('Post copied.'); ref.close(); }).catch(() => ctx.toast('Select and copy the text in the field above.', true)); }, { icon: 'copy', variant: 'primary' }));
}
function editPrompt(ctx: AppContext): void {
    const form = h('form', { class: 'stack' }, textareaField('What should hatchling write about?', 'prompt', ctx.state.settings.feedPrompt, 6), h('p', { class: 'muted text-small' }, 'This direction is saved locally. Feed generation uses deterministic example writing in this build.'), h('button', { class: 'button primary', type: 'submit' }, 'Save prompt'));
    const ref = ctx.dialogs.open('Your feed, your direction', form);
    form.addEventListener('submit', (event) => {
        event.preventDefault();
        void ctx.run({ type: 'feed.prompt', text: valueOf(form, 'prompt') }).then((result) => {
            if (result.ok)
                ref.close();
        });
    });
}
