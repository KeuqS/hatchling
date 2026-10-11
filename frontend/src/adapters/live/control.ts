// Who is driving, on the client: the takeover state machine for any
// surface the agent and the user can both drive. The transition table is
// hatchling-core's, generated into CONTROL_TRANSITIONS, so the broker and the
// UI apply the same rules. The UI never moves control itself: it asks, and
// applies the handoff the broker reports, if the table allows it.
import { CONTROL_TRANSITIONS } from './protocol/generated.js';
import type { Actor, Control, Handoff } from './protocol/generated.js';
/** The state after `handoff` from `from`, or `undefined` if the table does not allow it. */
export function controlAfter(from: Control, handoff: Handoff): Control | undefined {
    return CONTROL_TRANSITIONS.find(row => row.from === from && row.handoff === handoff)?.to;
}
/** Whether `actor` drives in `control`. Nobody drives while control changes hands. */
export function drives(control: Control, actor: Actor): boolean {
    return control === actor;
}
/** Whether control is changing hands. */
export function switching(control: Control): boolean {
    return control === 'to_user' || control === 'to_agent';
}
