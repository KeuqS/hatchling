/** Synthetic examples only. Reference images inform geometry, not content. */
import type { AppState, Goal, Idea, SystemEntry, FeedPost } from '../protocol/models.js';
const referenceGoals: Goal[] = [
    {
        "id": "ref-goal-example",
        "kind": "tracking",
        "title": "Sample workshop materials",
        "description": "Fictional workshop plan for exploring the tracking UI.",
        "category": "Money",
        "status": "open",
        "pulse": "Waiting for the example materials list before estimating the workshop budget. No purchase is scheduled.",
        "pulseKind": "blocked",
        "attention": false,
        "due": null,
        "timeline": [
            {
                "date": "Sample day",
                "text": "The fictional workshop needs a materials list before the next step."
            }
        ]
    },
    {
        "id": "ref-goal-runtime",
        "kind": "tracking",
        "title": "Sample document collection",
        "description": "Fictional collection for exploring project tracking.",
        "category": "Career",
        "status": "open",
        "pulse": "The example outline is ready; the next step is a review of the checklist and handoff notes.",
        "pulseKind": "milestone",
        "attention": false,
        "due": null,
        "timeline": [
            {
                "date": "Sample day",
                "text": "A synthetic outline was added to this demonstration workspace."
            }
        ]
    }
];
type ReferenceIdea = Pick<Idea, 'title' | 'summary' | 'domain' | 'icon' | 'section'>;
const referenceIdeas: ReferenceIdea[] = [
    {
        "title": "I can turn sample notes into a clear review",
        "summary": "A short review of the example collection, with decisions, questions, and links to the local documents.",
        "domain": "Career",
        "icon": "clipboard",
        "section": ""
    },
    {
        "title": "I can compare the two sample outlines",
        "summary": "A side-by-side note showing how the same fictional material reads in two different orders, without changing the originals.",
        "domain": "Career",
        "icon": "clipboard",
        "section": ""
    },
    {
        "title": "I can sketch a small learning experiment",
        "summary": "A one-page exercise with a question, a short method, and space to record observations. The example stays in this local workspace.",
        "domain": "Learning & hobbies",
        "icon": "microscope",
        "section": ""
    },
    {
        "title": "I can draft an integration checklist",
        "summary": "A document that separates presentation, data contracts, and external work, so a future integration has clear review points.",
        "domain": "Devices",
        "icon": "compass",
        "section": ""
    },
    {
        "title": "I can organize the sample project brief",
        "summary": "A practical structure for the fictional project: purpose first, then outcomes, open questions, and an explicit next step.",
        "domain": "Productivity",
        "icon": "lock",
        "section": "Productivity"
    },
    {
        "title": "I can make a quieter plan for the sample day",
        "summary": "An editable outline with two focus blocks and an open afternoon. It demonstrates planning without reading a calendar or creating events.",
        "domain": "Productivity",
        "icon": "calendar",
        "section": "Productivity"
    },
    {
        "title": "I can keep the sample documents together",
        "summary": "A small collection of example notes with clear names and a handoff page. Nothing is sent, paid, or filed outside the local demo.",
        "domain": "Productivity",
        "icon": "folder",
        "section": "Productivity"
    }
];
/** Build a useful tree from adapter-owned records; never expose or probe the host filesystem. */
function fileIndex(state: AppState): SystemEntry[] {
    const userFolders = new Set(['assets', 'docs', 'Downloads', 'memory', 'user', 'workspace']);
    const names = ['agents', 'assets', 'channels', 'config', 'data', 'docs', 'Downloads', 'dreams', 'hooks', 'memory', 'drafts.scratch', 'subscriptions', 'user', 'sample-results', 'workspace', 'sample-reviews'];
    const entries: SystemEntry[] = names.map((name) => ({ id: `dir-${name}`, parentId: null, name, kind: 'folder', visibility: userFolders.has(name) ? 'user' : 'internal', modified: null, bytes: null, artifactId: null, memoryId: null, description: userFolders.has(name) ? 'User-visible sample files from this local workspace.' : 'Protected internal directory. Only the existence of this directory and safe descriptive metadata are available in this scripted index.' }));
    entries.push({ id: 'workspace-documents', parentId: 'dir-workspace', name: 'documents', kind: 'folder', visibility: 'user', modified: null, bytes: null, artifactId: null, memoryId: null, description: 'Documents saved in the local sample workspace.' });
    for (const item of state.artifacts.filter((entry) => entry.kind === 'document')) {
        for (const parentId of ['dir-docs', 'workspace-documents']) {
            entries.push({ id: `${parentId}-${item.id}`, parentId, name: item.title, kind: 'document', visibility: 'user', modified: item.created.slice(0, 10), bytes: new TextEncoder().encode(item.content).length, artifactId: item.id, memoryId: null, description: item.description });
        }
    }
    for (const note of state.memory) {
        entries.push({ id: `note-${note.id}`, parentId: 'dir-memory', name: note.title, kind: 'note', visibility: 'user', modified: note.date, bytes: new TextEncoder().encode(note.body).length, artifactId: null, memoryId: note.id, description: 'A user-visible local memory note. No secrets are stored here.' });
    }
    const archiveReferences = [
        ['sample-notes.tar.gz', '335.9 KB'], ['sample-outline-v1.tar.gz', '285.7 KB'],
        ['sample-outline-v2.tar.gz', '285.5 KB'], ['sample-collection-v2.tar.gz', '330.4 KB'], ['sample-collection-v1.tar.gz', '288.4 KB'],
    ];
    for (const [index, pair] of archiveReferences.entries()) {
        const [name = '', size = ''] = pair;
        entries.push({ id: `reference-archive-${index}`, parentId: 'dir-Downloads', name, kind: 'document', visibility: 'user', modified: null, bytes: null, artifactId: null, memoryId: null, reference: { type: 'GZ', modified: 'Wed', size }, description: 'Synthetic archive listing. No archive bytes exist for this metadata-only example; use the bundle scenario for a real local sample download.' });
    }
    entries.push({ id: 'index-overview', parentId: 'dir-agents', name: 'Workspace overview', kind: 'index', visibility: 'internal', modified: null, bytes: null, artifactId: null, memoryId: null, description: 'Read-only sample metadata. One scripted agent is configured. Network actions, credential access, code execution, and production workers are not connected.' });
    return entries;
}
export function applySurfaceFixtures(state: AppState): AppState {
    for (const goal of state.goals)
        goal.sampleOnly = true;
    state.goals.unshift(...structuredClone(referenceGoals));
    for (const idea of state.ideas)
        idea.sampleOnly = true;
    state.ideas.unshift(...referenceIdeas.map((idea, index): Idea => ({ ...idea, id: `ref-idea-${index}`, rationale: 'Synthetic suggestion. Activation creates a labeled local example, not an external action.', signal: `Synthetic fact: the sample workspace includes the suggestion “${idea.title}”.`, items: [{ title: 'Local demonstration brief', summary: 'An editable document showing the chosen scope and the boundary to a future live adapter.' }], state: 'suggested', outcome: '', artifactId: null })));
    state.posts.unshift(...([{ "id": "ref-feed-companion", "title": "A little more room for your next idea", "body": "This fictional edition starts with a small thought: an outline can be useful without being finished. Leave a clear opening, a few places to collect details, and one question worth returning to. The sample notebook below keeps those parts together. It is a local illustration, not a product announcement or a recommendation.", "kicker": "Synthetic editorial", "category": "Learning", "icon": "fingerprint", "reason": "This example demonstrates the dated feed layout. It is not inferred from a user profile or drawn from a real news story.", "date": "Thursday morning", "reacted": false, "reference": true, "highlights": ["sample notebook"], "media": { "src": "./media/feed-notebook.svg", "alt": "Original diagram of a sample notebook and pencil", "width": 303, "height": 170 } }, { "id": "ref-feed-security", "title": "Make a useful handoff easy to find", "body": "A good sample handoff names the outcome, puts the supporting notes nearby, and states what remains undecided. This fictional project has an outline and a checklist; its handoff links both, without promising that an external review happened. Open the example to explore the layout, then change the Markdown or keep a local copy.", "kicker": "Synthetic editorial", "category": "Productivity", "icon": "shield", "reason": "This example demonstrates a second editorial section with a compact image. No repository activity or security event is implied.", "date": "Wednesday evening", "reacted": false, "reference": true, "highlights": ["sample handoff"], "media": { "src": "./media/feed-handoff.svg", "alt": "Original diagram of linked sample document cards", "width": 326, "height": 183 } }] satisfies FeedPost[]));
    state.systemEntries = fileIndex(state);
    return state;
}
/** Keep the demo index consistent with newly created or edited artifacts and memory. */
export function refreshSystemIndex(state: AppState): void { state.systemEntries = fileIndex(state); }
