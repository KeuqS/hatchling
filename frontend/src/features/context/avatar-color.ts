/** Select a stable, visible accent from a small raster sample; ignore transparent background. */
export function avatarAccentFromPixels(pixels: Uint8ClampedArray): string {
    const bins = new Map<string, {
        r: number;
        g: number;
        b: number;
        count: number;
        weight: number;
    }>();
    let red = 0, green = 0, blue = 0, count = 0;
    for (let i = 0; i + 3 < pixels.length; i += 4) {
        const r = pixels[i] ?? 0, g = pixels[i + 1] ?? 0, b = pixels[i + 2] ?? 0;
        if ((pixels[i + 3] ?? 0) < 128)
            continue;
        red += r;
        green += g;
        blue += b;
        count++;
        const spread = Math.max(r, g, b) - Math.min(r, g, b);
        if (spread < 24 || Math.max(r, g, b) < 40)
            continue;
        const key = `${Math.floor(r / 32)}:${Math.floor(g / 32)}:${Math.floor(b / 32)}`;
        const bin = bins.get(key) ?? { r: 0, g: 0, b: 0, count: 0, weight: 0 };
        bin.r += r;
        bin.g += g;
        bin.b += b;
        bin.count++;
        bin.weight += .2 + spread / 255;
        bins.set(key, bin);
    }
    const strongest = [...bins.values()].toSorted((a, b) => b.weight - a.weight)[0];
    if (!count)
        return '#a78c57';
    const values = strongest ? [strongest.r / strongest.count, strongest.g / strongest.count, strongest.b / strongest.count] : [red / count, green / count, blue / count];
    return '#' + values.map(value => Math.round(value).toString(16).padStart(2, '0')).join('');
}
/** Samples local raster bytes only; no cross-origin image fetch and no uploaded credentials. */
export function avatarAccentFromImage(source: string): Promise<string> {
    return new Promise(resolve => {
        if (!/^data:image\/(png|jpeg|webp);base64,/u.test(source)) {
            resolve('#a78c57');
            return;
        }
        const image = new Image();
        image.addEventListener('error', () => resolve('#a78c57'), { once: true });
        image.addEventListener('load', () => {
            const canvas = document.createElement('canvas');
            canvas.width = 64;
            canvas.height = 64;
            const context = canvas.getContext('2d');
            if (!context) {
                resolve('#a78c57');
                return;
            }
            context.drawImage(image, 0, 0, 64, 64);
            resolve(avatarAccentFromPixels(context.getImageData(0, 0, 64, 64).data));
        }, { once: true });
        image.src = source;
    });
}
