import type { AppState, Command, CommandResult } from '../../protocol/models.js';
import { containsSensitiveValue } from '../../protocol/rules.js';
import { textMessage } from '../fixtures.js';
import { fail, nextId, ok, say } from './shared.js';
import { reduceTrust } from './trust.js';
export function reduceChat(state: AppState, command: Command): CommandResult | null {
    switch (command.type) {
        case 'chat.read': {
            const chat = state.chats.find((item) => item.id === command.id);
            if (!chat)
                return fail('Conversation not found.');
            chat.unread = false;
            return ok('');
        }
        case 'settings.accent':
            state.settings.accent = command.value;
            return ok('');
        case 'settings.chat-panel':
            state.settings.keepChatPanelVisible = command.value;
            return ok('');
        case 'chat.send': {
            const chat = state.chats.find((item) => item.id === command.chatId);
            if (!chat || chat.archived)
                return fail('Open an active conversation before sending.');
            if (chat.channelId)
                return fail('This conversation is read-only here. Reply from its messaging app.');
            const text = command.text.trim();
            if (!text)
                return fail('Write a message first.');
            if (containsSensitiveValue(text))
                return fail('Do not put payment details, passwords or codes in chat. Use the provider’s secure page or browser takeover.');
            state.messages.push(textMessage(nextId(state, 'user'), text, 'user', chat.id));
            const rename = text.match(/^change (?:my|your) name to\s+(.+)$/i);
            if (rename?.[1]) {
                const name = rename[1].trim().replace(/[.!]$/, '');
                if (!name || name.length > 60)
                    return fail('Use a name of 60 characters or fewer.');
                state.settings.name = name;
                say(state, `Done — you can call me ${name}.`, chat.id);
                return ok('');
            }
            if (/^change (?:my|your) avatar to/i.test(text)) {
                say(state, 'Describe the avatar you have in mind, or attach a picture. This frontend can use an attached image locally; it does not generate images.', chat.id);
                return ok('');
            }
            const pending = state.purchases.find((item) => item.state === 'proposed');
            if (/^(yes|yes please|approve|looks good)[.!]?$/i.test(text) && pending && chat.id === 'main') {
                const result = reduceTrust(state, { type: 'purchase.approve', id: pending.id, revision: pending.revision, total: pending.total });
                say(state, result?.message ?? 'The review could not be completed.', chat.id);
                return result;
            }
            if (state.turn === 'working') {
                say(state, 'Got it. I’ll use that direction for the remaining work, without starting a second activity thread.', chat.id);
                return ok('Direction added to the current turn.');
            }
            if (state.turn === 'interrupted' || state.turn === 'stalled') {
                state.turn = 'idle';
                say(state, 'Got it. We’ll stop the previous path and use this as the next direction. Nothing was submitted.', chat.id);
                return ok('The next direction is saved.');
            }
            if (/^(hi|hello|hey|thanks|thank you)[.! ]*$/i.test(text))
                say(state, /thank/i.test(text) ? 'You’re welcome. We can leave it there.' : 'Hey. What’s on your mind?', chat.id);
            else if (/pack/i.test(text))
                say(state, 'Keep it light: a warm layer, comfortable shoes, a book, and a small water bottle.\n\nThe sample packing app in your library is ready to adjust.', chat.id);
            else if (/book|buy|purchase|checkout/i.test(text))
                say(state, 'We’ll need an exact review before any commitment. Open the purchase scenario to walk through a full sample checkout — no real order or payment will be made.', chat.id);
            else if (/change|edit|revise/i.test(text))
                say(state, 'Tell me the change you want to make. Markdown and slide decks can also be edited directly in the viewer; other formats get a refreshed preview through the adapter.', chat.id);
            else
                say(state, 'I’ve kept that in this conversation. This is a scripted frontend, so I’m not making an unverified claim that work ran.\n\nUse the scenario menu to explore research, approvals, secure handoffs, or browser takeover with inspectable results.', chat.id);
            return ok('Message saved locally.');
        }
        case 'chat.attach': {
            const chat = state.chats.find((item) => item.id === command.chatId);
            if (!chat || chat.channelId)
                return fail('Reply to channel conversations in their messaging app.');
            if (!state.artifacts.some((item) => item.id === command.artifactId))
                return fail('Attachment not found.');
            if ((() => { const request = state.messages.filter(message => message.chatId === chat.id && message.role === 'user').at(-1); return request?.kind === 'text' && /^change (?:my|your) avatar to/i.test(request.text); })()) {
                const avatar = state.artifacts.find(item => item.id === command.artifactId);
                if (avatar?.format === 'image' && avatar.asset) {
                    if (!/^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/u.test(avatar.asset) || avatar.asset.length > 700000)
                        return fail('Choose a PNG, JPEG or WebP smaller than 500 KB for an avatar.');
                    if (command.avatarAccent !== undefined && !/^#[\da-f]{6}$/iu.test(command.avatarAccent))
                        return fail('The avatar color must be a six-digit hex value.');
                    state.settings.avatar = avatar.asset;
                    if (command.avatarAccent)
                        state.settings.avatarAccent = command.avatarAccent;
                    say(state, 'The attached image is now my avatar in this local workspace.', chat.id);
                }
            }
            state.messages.push({ id: nextId(state, 'attachment'), chatId: chat.id, time: '9:41 am', source: null, reaction: null, kind: 'attachment', role: 'user', text: 'Here is the file I want to share in this local conversation.', artifactId: command.artifactId });
            return ok('Attachment added locally. Nothing was uploaded.');
        }
        case 'chat.create': {
            if (!command.title.trim())
                return fail('Give the conversation a title.');
            const id = nextId(state, 'chat');
            state.chats.splice(1, 0, { id, title: command.title.trim(), channelId: null, pinned: false, archived: false, unread: false });
            say(state, 'A fresh conversation, with its own context. What would you like to work on?', id);
            return ok('Side chat created.', id);
        }
        case 'chat.rename':
        case 'chat.toggle-pin':
        case 'chat.archive':
        case 'chat.delete': {
            const chat = state.chats.find((item) => item.id === command.id);
            if (!chat)
                return fail('Conversation not found.');
            if (chat.id === 'main')
                return fail('The main chat stays available. Create a side chat to organize separate work.');
            if (command.type === 'chat.rename') {
                if (!command.title.trim())
                    return fail('A title is required.');
                chat.title = command.title.trim();
                return ok('Conversation renamed.');
            }
            if (command.type === 'chat.toggle-pin') {
                chat.pinned = !chat.pinned;
                return ok(chat.pinned ? 'Conversation pinned.' : 'Conversation unpinned.');
            }
            if (command.type === 'chat.archive') {
                chat.archived = !chat.archived;
                return ok(chat.archived ? 'Conversation archived.' : 'Conversation restored.');
            }
            if (chat.channelId)
                return fail('Disconnect the messaging channel before deleting its conversation.');
            state.chats = state.chats.filter((item) => item.id !== chat.id);
            state.messages = state.messages.filter((item) => item.chatId !== chat.id);
            return ok('Side chat deleted');
        }
        case 'message.react': {
            const message = state.messages.find((item) => item.id === command.id);
            if (!message)
                return fail('Message not found.');
            if (!['👍', '❤️', '😊', '😂', '😮', '😢', '🙏', '🎉', '🔥', '👏', '💡', '✅', ''].includes(command.reaction))
                return fail('Choose one of the available reactions.');
            message.reaction = message.reaction === command.reaction ? null : command.reaction;
            return ok('Reaction updated.');
        }
        case 'message.choose': {
            const message = state.messages.find((item) => item.id === command.id);
            if (message?.kind !== 'choice' || !message.options.includes(command.option))
                return fail('That option is not part of this question.');
            if (message.selected)
                return fail('This question has already been answered. You can steer the conversation with a new message.');
            message.selected = command.option;
            state.messages.push(textMessage(nextId(state, 'choice'), command.option, 'user', message.chatId));
            say(state, `Got it — ${command.option.toLowerCase()}. I’ll keep that choice with this plan.`, message.chatId);
            return ok('Choice saved.');
        }
        case 'message.secure-complete': {
            const message = state.messages.find((item) => item.id === command.id);
            if (message?.kind !== 'secure')
                return fail('Secure handoff not found.');
            message.completed = true;
            state.turn = 'idle';
            say(state, 'The sample secure handoff is complete. I have a protected reference, not the credential value.', message.chatId);
            return ok('Secure handoff demonstrated without collecting credentials.');
        }
        case 'message.code-complete': {
            const message = state.messages.find((item) => item.id === command.id);
            if (message?.kind !== 'code-request')
                return fail('Code request not found.');
            message.completed = true;
            return ok('Sample site step complete. No code was entered into chat or memory.');
        }
        case 'turn.interrupt':
            state.turn = 'interrupted';
            for (const thread of state.threads)
                if (thread.state === 'running')
                    thread.state = 'interrupted';
            return ok('Work interrupted. The current context is kept.');
        case 'turn.resume': {
            const resumable = state.threads.filter((thread) => ['live-research', 'live-ci', 'live-bundle', 'live-collect'].includes(thread.id) && (thread.state === 'running' || thread.state === 'interrupted'));
            for (const thread of resumable)
                thread.state = 'running';
            state.turn = resumable.length ? 'working' : 'idle';
            if (!resumable.length)
                say(state, 'The saved context is ready. Tell me the next direction; no background task was restarted.');
            return ok(resumable.length ? 'Resuming the saved sample activity.' : 'Ready for your next direction.');
        }
        default: return null;
    }
}
