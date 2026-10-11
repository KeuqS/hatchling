import { identityDocumentId, parseIdentityDocument } from '../../protocol/identity-document.js';
import type { AppState, Command, CommandResult } from '../../protocol/models.js';
import { containsSensitiveValue } from '../../protocol/rules.js';
import { artifact, row } from '../fixtures.js';
import { fail, nextId, ok, say } from './shared.js';
export function reduceWorkspace(state: AppState, command: Command): CommandResult | null {
    switch (command.type) {
        case 'identity.avatar': {
            const value = command.value;
            if (value !== '' && (!/^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/u.test(value) || value.length > 700000))
                return fail('Choose a PNG, JPEG or WebP smaller than 500 KB.');
            if (command.accent !== undefined && !/^#[\da-f]{6}$/iu.test(command.accent))
                return fail('The avatar accent must be a six-digit hex color.');
            state.settings.avatar = value;
            state.settings.avatarAccent = value ? command.accent ?? state.settings.avatarAccent : '#a78c57';
            return ok('Avatar updated locally.');
        }
        case 'support.download': {
            if (!command.title.trim() || command.body.length > 50000)
                return fail('The local support note is invalid.');
            return { ok: true, message: 'Local note downloaded. Nothing was sent.', download: { filename: command.title.replace(/[^a-z0-9.-]/giu, '-').slice(0, 100) + '.md', mime: 'text/markdown', text: command.body, asset: null } };
        }
        case 'goal.create': {
            if (!command.title.trim())
                return fail('Give your goal a title.');
            const id = nextId(state, 'goal');
            state.goals.push({ id, kind: 'user', title: command.title.trim(), description: command.description.trim(), category: command.category, status: 'open', pulse: command.due ? `Scheduled for your chosen date, ${command.due}.` : 'Waiting for your choice of the first useful milestone.', pulseKind: command.due ? 'scheduled' : 'waiting', attention: true, due: command.due || null, timeline: [{ date: 'Sep 23', text: 'Created from your explicit goal description.' }] });
            return ok('Goal created. No recurring reminders were authorized.', id);
        }
        case 'goal.update':
        case 'goal.close':
        case 'goal.reopen': {
            const goal = state.goals.find((item) => item.id === command.id);
            if (!goal)
                return fail('Goal not found.');
            if (command.type === 'goal.update') {
                if (!command.title.trim())
                    return fail('The title cannot be empty.');
                goal.title = command.title.trim();
                goal.description = command.description;
                goal.due = command.due || null;
                goal.timeline.unshift({ date: 'Sep 23', text: 'Updated from your instructions.' });
                return ok('Goal updated.');
            }
            goal.status = command.type === 'goal.close' ? 'closed' : 'open';
            goal.attention = false;
            goal.pulse = goal.status === 'closed' ? 'Closed at your request; monitoring has stopped.' : 'Waiting for your choice of the next milestone.';
            goal.pulseKind = goal.status === 'closed' ? 'milestone' : 'waiting';
            goal.timeline.unshift({ date: 'Sep 23', text: goal.status === 'closed' ? 'You closed this responsibility.' : 'You reopened this responsibility.' });
            if (goal.status === 'closed')
                for (const job of state.jobs.filter((item) => item.goalId === goal.id))
                    job.health = 'disabled';
            return ok(goal.status === 'closed' ? 'Closed. Goal-owned scheduled work was disabled.' : 'Goal reopened.');
        }
        case 'idea.activate':
        case 'idea.dismiss':
        case 'idea.restore':
        case 'idea.edit': {
            const idea = state.ideas.find((item) => item.id === command.id);
            if (!idea)
                return fail('Idea not found.');
            if (command.type === 'idea.edit') {
                if (!command.summary.trim())
                    return fail('Describe the scope to keep.');
                idea.summary = command.summary.trim();
                return ok('Idea scope updated before activation.');
            }
            if (command.type === 'idea.dismiss') {
                idea.state = 'dismissed';
                return ok('Idea dismissed. Restore it from the Dismissed filter.');
            }
            if (command.type === 'idea.restore') {
                idea.state = 'suggested';
                return ok('Idea restored to the suggestion list.');
            }
            if (!['suggested', 'needs_user', 'failed'].includes(idea.state))
                return fail('This idea is already active or completed.');
            idea.state = 'activated';
            idea.outcome = 'Accepted. Preparing the bounded sample result.';
            return ok('Idea activated. The sample worker will attach its completion evidence.');
        }
        case 'feed.prompt': {
            if (!command.text.trim())
                return fail('Give your feed a direction.');
            state.settings.feedPrompt = command.text.trim();
            return ok('Feed prompt saved.');
        }
        case 'feed.generate': {
            state.posts.unshift({ id: nextId(state, 'post'), title: 'Make the next step small enough to finish', body: 'Choose one useful result, give it a clear stopping point, and keep the evidence beside it. There is room for a bigger plan later.\n\nThis is a scripted editorial example, generated from the saved demo feed direction — not a researched news update.', kicker: 'A fresh sample edition', category: 'Work & everyday life', icon: 'spark', reason: `Your saved feed direction: ${state.settings.feedPrompt}`, date: 'Wednesday morning', reacted: false });
            return ok('A new sample feed edition is ready.');
        }
        case 'feed.react': {
            const post = state.posts.find((item) => item.id === command.id);
            if (!post)
                return fail('Post not found.');
            post.reacted = !post.reacted;
            return ok(post.reacted ? 'Saved your reaction.' : 'Reaction removed.');
        }
        case 'memory.save': {
            if (!command.title.trim() || !command.body.trim())
                return fail('A title and a note are required.');
            if (containsSensitiveValue(`${command.title}\n${command.body}`))
                return fail('Do not store passwords, tokens, security codes or account numbers in memory. Record only the item’s existence and protected location.');
            const entry = state.memory.find((item) => item.id === command.id);
            if (command.expectedBody !== undefined && entry?.body !== command.expectedBody)
                return fail('This document changed since you started editing. Your draft is safe; review the latest version before saving.');
            if (command.id === identityDocumentId) {
                const identity = parseIdentityDocument(command.body);
                if (!identity || command.title !== 'IDENTITY.md')
                    return fail('Keep one Name (up to 60 characters), Character, Vibe (up to 240 characters), and Emoji line in IDENTITY.md.');
                state.settings.name = identity.name;
                state.settings.vibe = identity.vibe;
            }
            if (entry?.kind === 'logs')
                return fail('Source logs are append-only. Add a correcting note instead.');
            if (entry) {
                entry.title = command.title;
                entry.body = command.body;
            }
            else
                state.memory.unshift({ id: nextId(state, 'memory'), kind: 'memory', title: command.title, body: command.body, date: 'Sep 23, 2026', closeness: 0 });
            return ok('Memory note saved without sensitive values.');
        }
        case 'memory.delete': {
            if (command.id === identityDocumentId)
                return fail('The identity document is part of the workspace. Edit it instead of deleting it.');
            const entry = state.memory.find((item) => item.id === command.id);
            if (entry?.kind === 'logs')
                return fail('Append-only source logs cannot be individually rewritten or deleted.');
            state.memory = state.memory.filter((item) => item.id !== command.id);
            return ok('Memory entry removed.');
        }
        case 'job.update':
        case 'job.create': {
            if (!command.title.trim() || !command.schedule.trim())
                return fail('A title and explicit schedule are required.');
            if (command.kind === 'hook' && (!command.source.trim() || !command.event.trim()))
                return fail('Select a source and an event.');
            if (command.kind === 'time' && !command.recurring && /every|daily|weekly|monthly|repeat/i.test(command.schedule))
                return fail('That is a recurring schedule. Enable recurrence and approve it explicitly.');
            if (command.type === 'job.update') {
                const job = state.jobs.find((entry) => entry.id === command.id);
                if (!job)
                    return fail('Scheduled work not found.');
                job.title = command.title.trim();
                job.kind = command.kind;
                job.schedule = command.schedule.trim();
                job.recurring = command.recurring;
                job.source = command.source;
                job.event = command.event;
                job.nextRun = command.schedule.trim();
                job.limit = command.recurring ? 'Only this newly approved schedule; meaningful results only' : 'Exactly one run, then disabled';
                return ok('Updated to the schedule you approved.', job.id);
            }
            const id = nextId(state, 'job');
            state.jobs.push({ id, title: command.title.trim(), kind: command.kind, schedule: command.schedule.trim(), recurring: command.recurring, limit: command.recurring ? 'Only this approved schedule; meaningful results only' : 'Exactly one run, then disabled', source: command.source, event: command.event, goalId: null, health: 'ok', lastRun: 'Not yet run', nextRun: command.schedule.trim() });
            return ok(`Set: ${command.schedule.trim()}. ${command.recurring ? 'This recurrence was explicitly approved.' : 'One run only.'}`, id);
        }
        case 'job.toggle':
        case 'job.run':
        case 'job.delete': {
            const job = state.jobs.find((item) => item.id === command.id);
            if (!job)
                return fail('Scheduled work not found.');
            if (command.type === 'job.delete') {
                state.jobs = state.jobs.filter((item) => item.id !== job.id);
                return ok('Scheduled work removed.');
            }
            if (command.type === 'job.toggle') {
                job.health = job.health === 'disabled' ? 'ok' : 'disabled';
                return ok(job.health === 'disabled' ? 'Scheduled work paused.' : 'Scheduled work enabled.');
            }
            if (job.health !== 'ok')
                return fail('Enable this job before running it.');
            job.lastRun = 'Today, 9:41 am';
            if (!job.recurring) {
                job.health = 'disabled';
                job.nextRun = 'Completed its one authorized run';
            }
            state.notifications.unshift({ id: nextId(state, 'delivery'), title: job.title, text: 'The sample work you arranged to receive is ready. This delivery is recorded once in the ledger.', time: '9:41 am', read: false, urgent: false, evidenceId: 't-digest', offer: null, stage: 'delivered' });
            state.messages.push({ id: nextId(state, 'scheduled-message'), chatId: 'main', time: '9:41 am', source: null, reaction: null, role: 'agent', kind: 'scheduled', text: `Your arranged result is ready: ${job.title}.`, evidenceId: 't-digest' });
            return ok('The arranged result was delivered.');
        }
        case 'notification.read':
        case 'notification.answer': {
            const notification = state.notifications.find((item) => item.id === command.id);
            if (!notification)
                return fail('Notification not found.');
            notification.read = true;
            if (command.type === 'notification.answer') {
                say(state, 'Here is the update you asked to bring into chat. The project handoff is ready to review.');
                return ok('Brought the requested update into the main chat.');
            }
            return ok('Marked as read.');
        }
        case 'settings.interface-size':
            if (!['compact', 'comfortable', 'large'].includes(command.value))
                return fail('Choose Compact, Comfortable or Large.');
            state.settings.interfaceSize = command.value;
            return ok('Interface size updated.');
        case 'settings.theme':
            state.settings.theme = command.value;
            return ok('Appearance updated.');
        case 'identity.update':
            if (!command.name.trim() || command.name.trim().length > 60 || !command.vibe.trim() || command.vibe.trim().length > 240 || /[\r\n]/u.test(command.name + command.vibe))
                return fail('Use a name up to 60 characters and a vibe up to 240 characters.');
            state.settings.name = command.name.trim();
            state.settings.vibe = command.vibe.trim();
            return ok('Identity updated.');
        case 'settings.name':
            if (!command.value.trim() || command.value.trim().length > 60 || /[\r\n]/u.test(command.value))
                return fail('Use a single-line display name up to 60 characters.');
            state.settings.name = command.value.trim();
            return ok('Display name updated.');
        case 'settings.language':
            state.settings.language = command.value;
            return ok('Language preference saved. This build’s interface is English.');
        case 'settings.allowlist':
            state.settings.allowlist = command.value.split('\n').map((line) => line.trim()).filter(Boolean);
            return ok('Navigation allowlist updated.');
        case 'settings.telemetry':
            state.settings.telemetry = command.value;
            return ok('Preference saved. This frontend sends no telemetry.');
        case 'settings.captcha':
            state.settings.captchaStanding = command.value;
            return ok(command.value ? 'Standing challenge permission enabled for allowlisted sites.' : 'Every challenge will ask for permission.');
        case 'settings.digest':
            state.settings.digestTime = command.time;
            state.settings.quietStart = command.start;
            state.settings.quietEnd = command.end;
            state.settings.dailyAllowance = Math.min(10, Math.max(1, command.allowance));
            return ok('Digest time, waking hours and daily allowance saved.');
        case 'secure.add':
            if (!command.service.trim() || containsSensitiveValue(command.service))
                return fail('Name the service, without entering a secret.');
            state.secureItems.push(`${command.service.trim()} · protected credential reference`);
            return ok('A protected reference was added. No credential value was collected.');
        case 'secure.remove':
            state.secureItems = state.secureItems.filter((item) => item !== command.service);
            return ok('Protected reference removed.');
        case 'data.export': return { ok: true, message: 'Your local demo data is ready to download.', download: { filename: 'hatchling-demo-data.json', mime: 'application/json', text: JSON.stringify(state, null, 2), asset: null } };
        default: return null;
    }
}
export function advanceIdeas(state: AppState, finish: boolean): void {
    for (const idea of state.ideas) {
        if (idea.state === 'activated') {
            idea.state = 'executing';
            idea.outcome = 'Building the sample document and checking the completion evidence.';
        }
        if (idea.state !== 'executing' || !finish)
            continue;
        const id = `result-${idea.id}`;
        state.artifacts = state.artifacts.filter((item) => item.id !== id);
        state.artifacts.unshift(artifact(id, `${idea.items[0]?.title ?? 'A small next step'}.md`, 'markdown', `# ${idea.items[0]?.title ?? 'A small next step'}\n\n${idea.summary}\n\n## Why this is useful\n\n${idea.rationale}\n\n## A bounded next step\n\nDefine the result, review the scope, then make one change you can verify.\n\n## Completion evidence\n\nThis sample document was created and saved in your local library. No messages, purchases, or public links were created.`, 'A finished result from the scripted idea worker.'));
        idea.state = 'completed';
        idea.artifactId = id;
        idea.outcome = 'Completed. The sample document was saved to your library; no external actions were taken.';
        state.threads.unshift({ id: `thread-${idea.id}`, name: 'Build a useful next step', subtitle: 'Saved a concrete document for your review', statusTitle: 'Saved document', state: 'completed', verdict: 'success', finishMessage: 'The document is ready to keep or edit. No external actions were taken.', time: '9:41 am', evidenceArtifactId: id, lanes: [{ id: 'main', name: 'Main agent', parentId: null, rows: [row(`row-${idea.id}`, 'Saved the idea’s finished document', 'The completion evidence is attached', 'file')] }] });
    }
}
