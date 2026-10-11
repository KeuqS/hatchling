import type { Accent, DeepReadonly, Settings } from '../../protocol/models.js';
export const accents: readonly {
    id: Accent;
    label: string;
    color: string;
}[] = [
    { id: 'avatar', label: 'Match my avatar', color: '#a78c57' },
    { id: 'blue', label: 'Blue', color: '#0576d9' },
    { id: 'azure', label: 'Azure', color: '#0082eb' },
    { id: 'violet', label: 'Violet', color: '#9d59d8' },
    { id: 'pink', label: 'Pink', color: '#d50096' },
    { id: 'orange', label: 'Orange', color: '#b86400' },
    { id: 'green', label: 'Green', color: '#568700' },
    { id: 'stone', label: 'Stone', color: '#907a62' },
    { id: 'white', label: 'White', color: '#f1f1f1' },
];
function luminance(hex: string): number {
    const channels = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255);
    return channels.reduce((sum, value, index) => sum + (value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4) * ([.2126, .7152, .0722][index] ?? 0), 0);
}
export function accentColors(settings: DeepReadonly<Pick<Settings, 'accent' | 'avatarAccent'>>, light: boolean): {
    background: string;
    foreground: string;
} {
    const choice = accents.find((item) => item.id === settings.accent);
    const background = settings.accent === 'white' ? (light ? '#e4e7eb' : '#f1f1f1') : settings.accent === 'avatar' && /^#[\da-f]{6}$/iu.test(settings.avatarAccent) ? settings.avatarAccent : choice?.color ?? '#f1f1f1';
    const l = luminance(background);
    const blackContrast = (l + .05) / .05;
    const whiteContrast = 1.05 / (l + .05);
    return { background, foreground: blackContrast >= whiteContrast ? '#000000' : '#ffffff' };
}
export function applyAppearance(settings: DeepReadonly<Settings>): void {
    document.documentElement.dataset['theme'] = settings.theme;
    document.documentElement.dataset['interfaceSize'] = settings.interfaceSize;
    document.documentElement.dataset['accent'] = settings.accent;
    const light = settings.theme === 'light' || (settings.theme === 'system' && window.matchMedia('(prefers-color-scheme: light)').matches);
    const palette = accentColors(settings, light);
    document.documentElement.style.setProperty('--bubble-user', palette.background);
    document.documentElement.style.setProperty('--bubble-text', palette.foreground);
    document.documentElement.style.setProperty('--theme-color', palette.background);
    document.documentElement.style.setProperty('--theme-on-color', palette.foreground);
}
