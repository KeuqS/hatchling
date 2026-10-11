export interface ComposerDraft {
    text: string;
    attachments: readonly string[];
}
/** Session-only view state. A slow/failed request must never restore text into a different chat. */
export class ComposerDrafts {
    private readonly saved = new Map<string, ComposerDraft>();
    save(chatId: string, text: string, attachments: readonly string[]): void {
        this.saved.set(chatId, { text, attachments: [...attachments] });
    }
    read(chatId: string): ComposerDraft {
        const draft = this.saved.get(chatId);
        return { text: draft?.text ?? '', attachments: [...(draft?.attachments ?? [])] };
    }
    recover(chatId: string, failedText: string, failedAttachments: readonly string[]): ComposerDraft {
        const current = this.read(chatId);
        const text = !failedText || current.text === failedText ? current.text : !current.text.trim() ? failedText : `${failedText}\n\n${current.text}`;
        this.save(chatId, text, [...new Set([...failedAttachments, ...current.attachments])]);
        return this.read(chatId);
    }
}
