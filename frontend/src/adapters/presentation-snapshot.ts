import type { AppState, DeepReadonly, Message } from '../protocol/models.js';
import { splitAgentBubbles } from '../protocol/rules.js';
/** Normalize the scripted text convention into ordinary message rows at the adapter seam.
 * A live adapter maps its authoritative content into these same UI records; it need not use this token.
 */
export function presentationSnapshot(state: AppState): DeepReadonly<AppState> {
    const messages: Message[] = state.messages.flatMap((message): Message[] => {
        if (message.kind !== 'text' || message.role !== 'agent')
            return [message];
        const parts = splitAgentBubbles(message.text);
        return parts.map((text, index): Message => ({ ...message, id: index === 0 ? message.id : `${message.id}:bubble:${index}`, text, reaction: index === 0 ? message.reaction : null }));
    });
    return { ...state, messages };
}
