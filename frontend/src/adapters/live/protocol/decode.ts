// Frames in and out of the live adapter . Outgoing frames are built
// from the generated types; incoming text is parsed and checked against the
// wire table before anything reads it.
import { conforms } from './conform.js';
import type { UiRequest, UiResponse } from './generated.js';
import { WIRE_DEFINITIONS } from './wire.js';
/** Whether a parsed value is a frame the UI sends. */
export function isUiRequest(value: unknown): value is UiRequest {
    return conforms(value, { $ref: '#/$defs/UiRequest' }, WIRE_DEFINITIONS);
}
/** Whether a parsed value is a frame the UI receives. */
export function isUiResponse(value: unknown): value is UiResponse {
    return conforms(value, { $ref: '#/$defs/UiResponse' }, WIRE_DEFINITIONS);
}
/** One received text frame, or `undefined` if it is not JSON or not a frame this build understands. */
export function decodeResponse(text: string): UiResponse | undefined {
    let value: unknown;
    try {
        value = JSON.parse(text);
    }
    catch {
        return undefined;
    }
    return isUiResponse(value) ? value : undefined;
}
/** One frame to send, as text. */
export function encodeRequest(request: UiRequest): string {
    return JSON.stringify(request);
}
