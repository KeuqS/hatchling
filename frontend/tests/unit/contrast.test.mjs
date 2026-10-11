import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const source = readFileSync('src/styles/tokens.css', 'utf8');
const tokens = (block) => Object.fromEntries([...block.matchAll(/--([a-z-]+):\s*(#[a-f0-9]{6});/g)].map((match) => [match[1], match[2]]));
const surfaces = readFileSync('src/styles/surfaces.css', 'utf8');
const dark = { ...tokens(source.split(":root[data-theme='light']")[0]), ...tokens(surfaces.split(":root[data-theme='light']")[0]) };
const light = { ...dark, ...tokens(source.split(":root[data-theme='light']")[1].split('@media')[0]), ...tokens(surfaces.split(":root[data-theme='light']")[1].split('@media')[0]) };
const luminance = (hex) => hex.slice(1).match(/../g).map((part) => parseInt(part, 16) / 255).map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4).reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
const contrast = (foreground, background) => { const a = luminance(foreground), b = luminance(background); return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05); };
const pairs = [
    ['profile-muted', 'bg'], ['editorial-muted', 'bg'], ['tracking-green', 'bg'], ['feed-link', 'bg'],
    ['text', 'composer-bg'], ['composer-placeholder', 'composer-bg'],
    ['context-muted', 'bg'], ['context-muted', 'surface-raised'], ['context-muted', 'composer-bg'],
    ['text', 'context-control'], ['text', 'nav-selected'], ['text', 'tooltip-bg'],
    ...['bg', 'surface', 'surface-raised', 'surface-control', 'settings-bg', 'settings-card'].flatMap((background) => [['text', background], ['muted', background]]),
    ['muted-on-card', 'settings-card'], ['primary-text', 'primary'], ['bubble-text', 'bubble-user'], ['accent', 'bg'], ['accent', 'surface-control'], ['success', 'surface-control'], ['danger', 'surface-control'], ['attention', 'surface-control'], ['ink', 'paper'],
];
for (const [mode, palette] of [['dark', dark], ['light', light]])
    test(`${mode}: ${pairs.length} text/background token pairs meet WCAG AA normal-text contrast`, () => {
        for (const [foreground, background] of pairs) {
            const ratio = contrast(palette[foreground], palette[background]);
            assert.ok(ratio >= 4.5, `${mode} ${foreground} / ${background}: ${ratio.toFixed(2)}:1`);
        }
    });
