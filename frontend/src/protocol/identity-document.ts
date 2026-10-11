/** A document is data, never instructions to the host. Keep editable text and the profile in sync. */
export interface IdentityFields {
    name: string;
    character: string;
    vibe: string;
    emoji: string;
}
export const identityDocumentId = 'identity-profile';
export function createIdentityDocument(name: string, vibe: string): string {
    return `# IDENTITY.md\n\n*Fill this in as you figure out who you are.*\n\n- **Name:** ${name}\n- **Character:** the chick in the egg — a freshly-hatched agent, small but sharp\n- **Vibe:** ${vibe}\n- **Emoji:** 🐣`;
}
function field(body: string, key: string): string | null {
    const pattern = new RegExp(`^[\\t ]*[-*][\\t ]+(?:\\*\\*)?${key}:(?:\\*\\*)?[\\t ]*(.+)$`, 'gimu');
    const matches = [...body.matchAll(pattern)];
    return matches.length === 1 ? matches[0]?.[1]?.trim() ?? null : null;
}
export function parseIdentityDocument(body: string): IdentityFields | null {
    const name = field(body, 'Name');
    const character = field(body, 'Character');
    const vibe = field(body, 'Vibe');
    const emoji = field(body, 'Emoji');
    if (!name || name.length > 60 || !vibe || vibe.length > 240 || !character || character.length > 500 || !emoji || emoji.length > 32)
        return null;
    return { name, character, vibe, emoji };
}
/** Patch only these two canonical lines. Preserve custom prose and other user-edited fields. */
export function patchIdentityDocument(body: string, name: string, vibe: string): string {
    return body.replace(/^([\t ]*[-*][\t ]+(?:\*\*)?Name:(?:\*\*)?[\t ]*)[^\n]*$/gimu, (_line: string, prefix: string) => prefix + name)
        .replace(/^([\t ]*[-*][\t ]+(?:\*\*)?Vibe:(?:\*\*)?[\t ]*)[^\n]*$/gimu, (_line: string, prefix: string) => prefix + vibe);
}
