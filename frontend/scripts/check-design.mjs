/** Focused guard for the shared design contract, not a general CSS linter. */
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
export function checkFeatureCss(path, source, knownTokens) {
    const errors = [];
    const css = source.replace(/\/\*[\s\S]*?\*\//g, '');
    if (/#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|oklch|lab)\s*\(/i.test(css))
        errors.push(`${path}: use a semantic color token, not a raw color.`);
    if (/\bzoom\s*:/i.test(css))
        errors.push(`${path}: CSS zoom is not an interface-size system.`);
    for (const match of css.matchAll(/font-family\s*:\s*([^;}]+)/g))
        if (!/^var\(--font-(?:ui|mono)\)$/.test(match[1].trim()))
            errors.push(`${path}: use the existing font tokens.`);
    for (const match of css.matchAll(/var\((--[a-z0-9-]+)/g))
        if (!knownTokens.has(match[1]))
            errors.push(`${path}: unknown shared or declared local token ${match[1]}.`);
    return [...new Set(errors)];
}
/** Ordinary menu surfaces have exactly one owner, including legacy stylesheets. */
export function checkMenuOwnership(path, source) {
    if (path === 'src/styles/menus.css')
        return [];
    const errors = [];
    const css = source.replace(/\/\*[\s\S]*?\*\//g, '');
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
        const selectors = rule[1].trim().split(',');
        const ordinary = selectors.some(selector => selector.includes('.menu-popover') && !/(?:^|[ >+~])\.(?:reaction-menu|message-reactions)\b/.test(selector) && !selector.includes('::backdrop'));
        if (ordinary && /(?:^|;)\s*(?:background(?:-[\w-]+)?|border(?:-[\w-]+)?|box-shadow|font(?:-[\w-]+)?|padding(?:-[\w-]+)?|min-width)\s*:/.test(rule[2]))
            errors.push(`${path}: action-menu appearance belongs in src/styles/menus.css, not ${rule[1].trim()}.`);
    }
    return errors;
}
export function checkTokenContract(source, required) {
    return Object.entries(required).flatMap(([name, value]) => source.includes(`${name}: ${value};`) ? [] : [`Design token changed or missing: ${name}. Update the guide, contract and tests together.`]);
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
    const contract = JSON.parse(readFileSync('docs/DESIGN-TOKENS.json', 'utf8'));
    const errors = checkTokenContract(readFileSync(contract.tokensFile, 'utf8'), contract.requiredTokens);
    const styles = readdirSync('src/styles').filter(name => name.endsWith('.css')).map(name => `src/styles/${name}`);
    const allCss = styles.map(path => readFileSync(path, 'utf8')).join('\n');
    const tokens = new Set([...allCss.matchAll(/(--[a-z0-9-]+)\s*:/g)].map(match => match[1]));
    for (const name of contract.localVariables)
        tokens.add(name);
    let checked = 0;
    for (const path of styles) {
        errors.push(...checkMenuOwnership(path, readFileSync(path, 'utf8')));
        if (path === contract.tokensFile || contract.legacyStyles.includes(path))
            continue;
        checked++;
        errors.push(...checkFeatureCss(path, readFileSync(path, 'utf8'), tokens));
    }
    for (const path of [contract.guide, 'docs/UI-CHECKLIST.md', 'docs/DESIGN-SYSTEM.md'])
        if (!existsSync(path))
            errors.push(`Missing design documentation: ${path}`);
    for (const path of ['AGENTS.md', 'README.md'])
        if (!readFileSync(path, 'utf8').includes('STYLE_GUIDE.md'))
            errors.push(`${path} must point to the style guide.`);
    const html = readFileSync('index.html', 'utf8');
    const linked = [...html.matchAll(/<link rel="stylesheet" href="\.\/styles\/([a-z-]+\.css)">/g)].map(match => `src/styles/${match[1]}`);
    for (const path of styles)
        if (!linked.includes(path))
            errors.push(`Stylesheet missing from both-build manifest: ${path}`);
    if (linked.length !== new Set(linked).size)
        errors.push('A stylesheet is linked twice in index.html.');
    for (const error of errors)
        console.error(error);
    console.log(`${checked} new-family stylesheets, ${Object.keys(contract.requiredTokens).length} shared primitives and design documentation checked; ${errors.length} errors.`);
    if (errors.length)
        process.exitCode = 1;
}
