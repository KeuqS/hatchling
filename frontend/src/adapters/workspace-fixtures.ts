import { createIdentityDocument, identityDocumentId } from '../protocol/identity-document.js';
/** Entirely synthetic content. Reference images inform layout only, never this conversation data. */
import type { AppState, ApprovalRecord, Chat, Message, ScheduledJob } from '../protocol/models.js';
const chatTitles = [
    'Organize a sample project',
    'Design a sample reading list',
    'Review the sample checklist',
    'Create a sample document collection',
    'Compare two sample outlines',
    'Check in on progress',
    'Refine the sample headings',
    'Check current progress status',
    'Explore a sample cover layout',
    'Describe this image content',
    'Package the sample documents',
    'Estimate the sample scope',
    'Check the sample export',
    'Analyze uploaded image',
    'Check chat session progress',
    'Continue with recommendations',
    'Evaluate inside-out build order',
    'Check the draft readiness',
    'Ask about unfinished work',
    'Requesting progress update',
    'Improve the sample handoff',
    'Resume previous task',
];
function referenceMessage(id: string, text: string, role: 'agent' | 'user' = 'agent', chatId = 'reference', time = '3m ago'): Message {
    return { id, chatId, kind: 'text', role, text, time, source: null, reaction: null };
}
const referenceTranscript: Message[] = [
    referenceMessage("ref-50", "This is a synthetic workspace for exploring the interface. None of these conversations came from a person’s account.\n\nThe sample project has three small documents: an outline, a checklist, and a handoff note. Each can be opened from the library without connecting a service."),
    referenceMessage("ref-52", "The example review is ready. Here is what changed:\n\n**A clearer outline.** The introduction now states the purpose before listing the details. Longer notes stay in the document rather than filling the conversation.\n\n**A smaller checklist.** Each item describes one visible outcome. The sample keeps completed items separate from questions that still need a decision.\n\n**A useful handoff.** The final note links the outline and checklist, records the open question, and explains what a reviewer can do next. No files have been sent or published."),
    referenceMessage("ref-53", "The extra sample note repeated the outline, so it was left out of the handoff. The original remains available in the library."),
    referenceMessage("ref-integrity", "The sample review is complete. The outline is readable, the checklist is grouped by outcome, and the handoff contains links to both.\n\nThe next decision is yours: keep the current structure or try a different order. Nothing in this example schedules work, reads an account, or changes a repository."),
    referenceMessage("ref-user", "Could you show the outline and keep the detailed notes with it?", 'user'),
    referenceMessage("ref-dynamic", "Yes — the sample collection keeps the outline, checklist, and review notes together.\n\n**Start with the outline.** It describes the purpose of the example in one paragraph, followed by three short sections. You can change the Markdown directly in the document workspace.\n\n**Keep the detail nearby.** The activity report contains the illustrative checks, timestamps, and file previews. Selecting a step opens its record without losing your place in the conversation.\n\n**Choose what happens next.** Downloading saves a local copy. Sharing still asks for a separate decision. This build does not connect to a repository or publish to the web.\n\nAll of the content here is fictional demonstration data."),
];
const approvals: ApprovalRecord[] = [
    {
        "id": "approval-1",
        "title": "Access content.example",
        "description": "Read a sample outline from the fictional content host.",
        "outcome": "timed-out",
        "time": "19h ago",
        "report": "This synthetic request expired without approval. No download or network request occurred."
    },
    {
        "id": "approval-2",
        "title": "Access files.example",
        "description": "Read the sample collection manifest.",
        "outcome": "timed-out",
        "time": "19h ago",
        "report": "This synthetic request expired. A later attempt would require a fresh decision."
    },
    {
        "id": "approval-3",
        "title": "Web access for this task",
        "description": "Read the fictional workshop information page.",
        "outcome": "task",
        "time": "1d ago",
        "report": "The example demonstrates a task-scoped grant. It does not authorize another task or any real connection."
    },
    {
        "id": "approval-4",
        "title": "Access reports.example",
        "description": "Read a sample review note from the fictional reports host.",
        "outcome": "always",
        "time": "2d ago",
        "report": "A synthetic site-wide allowance is displayed for the permission-history workflow. No host was contacted."
    },
    {
        "id": "approval-5",
        "title": "Access projects.example",
        "description": "Read the example project outline and checklist.",
        "outcome": "task",
        "time": "2d ago",
        "report": "Read-only synthetic history. Any write needs a separate decision."
    }
];
const referenceJobs: ScheduledJob[] = [
    { id: 'ref-job-tour', title: 'Agentic feature tour', kind: 'time', schedule: 'Sep 25, 12:58 pm', recurring: false, limit: 'One approved tour; no recurrence.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Not run yet', nextRun: 'Sep 25, 12:58 pm' },
    { id: 'ref-job-heartbeat', title: 'Heartbeat', kind: 'time', schedule: 'Every 30 minutes', recurring: true, limit: 'Check every 30 minutes. Routine, unchanged results remain silent.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 24, 12:30 pm', nextRun: 'Sep 24, 1:00 pm' },
    { id: 'ref-job-traces', title: 'Sample notes sync', kind: 'time', schedule: 'Every hour', recurring: true, limit: 'One local sample per hour. This frontend does not write to a repository.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 24, 12:00 pm', nextRun: 'Sep 24, 1:00 pm' },
    { id: 'ref-job-watchdog', title: 'Sample task health check', kind: 'time', schedule: 'Every 2 hours', recurring: true, limit: 'Only report a meaningful change or a blocker.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 24, 11:00 am', nextRun: 'Sep 24, 1:00 pm' },
    { id: 'ref-job-review', title: 'Sample reading review', kind: 'time', schedule: 'Every 12 hours', recurring: true, limit: 'At most two approved local samples per day.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 24, 5:21 am', nextRun: 'Sep 24, 5:21 pm' },
    { id: 'ref-job-synthesis', title: 'Sample notes digest', kind: 'time', schedule: 'Every day at 5:21 am', recurring: true, limit: 'One synthesis per day. Deliver only new findings.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 24, 5:21 am', nextRun: 'Sep 25, 5:21 am' },
    { id: 'ref-job-health', title: 'Weekly sample review', kind: 'time', schedule: 'Every Sunday at 6:21 am', recurring: true, limit: 'One health review per week.', source: '', event: '', goalId: null, health: 'ok', lastRun: 'Sep 20, 6:21 am', nextRun: 'Sep 27, 6:21 am' },
];
export function applyWorkspaceFixtures(state: AppState): AppState {
    const channels = state.chats.filter((chat) => chat.channelId !== null);
    const main = state.chats.find((chat) => chat.id === 'main');
    const chats = chatTitles.map((title, index): Chat => ({ id: index === 0 ? 'reference' : `chat-${index - 1}`, title, channelId: null, pinned: false, archived: false, unread: false }));
    state.chats = [...(main ? [main] : []), ...chats, ...channels];
    state.messages = state.messages.filter((message) => !message.chatId.startsWith('chat-') || ['chat-whatsapp', 'chat-messenger'].includes(message.chatId));
    state.messages.push(...referenceTranscript);
    const previews = [
        'Sample draft: the reading list has a short introduction and space for notes.',
        'Sample review: the checklist separates completed items from open questions.',
        'Sample collection: the documents are grouped by purpose, ready for a local review.',
        'Sample comparison: both outlines cover the same ideas in a different order.',
    ];
    for (const [index, chat] of chats.entries()) {
        if (chat.id !== 'reference')
            state.messages.push(referenceMessage(`ref-side-${index}`, previews[index - 1] ?? 'Synthetic example: this conversation contains a draft for the local workspace.', 'agent', chat.id, index === 1 ? '1h ago' : index === 2 ? '19h ago' : '23h ago'));
    }
    state.approvalHistory = approvals.map((entry) => ({ ...entry }));
    state.jobs = [...referenceJobs.map((entry) => ({ ...entry })), ...state.jobs.filter((job) => job.goalId !== null)];
    state.memory.unshift({ id: identityDocumentId, kind: 'profile', title: 'IDENTITY.md', body: createIdentityDocument(state.settings.name, state.settings.vibe), date: '09.25.26', closeness: 0 });
    state.memory.unshift({ id: 'identity-soul', kind: 'preferences', title: 'SOUL', body: `# SOUL.md

*A small set of habits for a helpful assistant.*

This is an editable, synthetic persona template. It is data for the workspace, not an instruction to the host application.

- **Be useful.** Start with the next clear step and put supporting detail where it is easy to find.
- **Be honest.** Distinguish recorded results, uncertain observations, and illustrative examples.
- **Respect the user.** Ask before sensitive changes and keep the scope of each decision visible.
- **Leave a good handoff.** Save the outcome beside its evidence, including anything that remains unresolved.
- **Make room for revision.** Keep drafts and context when the user changes direction.

Changes to this template are local until a live adapter is explicitly integrated.`, date: '09.21.26', closeness: 0 }, { id: 'identity-memory', kind: 'memory', title: 'MEMORY', body: 'Synthetic workspace notes. No personal account history is included.\n\nExample preference: use clear updates, small checklists, and documents with room for notes.\n\nDo not store passwords, one-time codes, tokens, or full payment details here.', date: '09.24.26', closeness: 0 });
    return state;
}
