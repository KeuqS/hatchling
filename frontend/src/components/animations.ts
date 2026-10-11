import type { IconName } from '../protocol/models.js';
import type { LottieAnimation, LottieLayer, Property, Shape } from './lottie-model.js';
const fixed = (value: number | number[]): Property => ({ a: 0, k: value });
const frames = (values: readonly [
    number,
    number[]
][]): Property => ({ a: 1, k: values.map(([t, s], index) => ({ t, s, ...(values[index + 1] ? { e: values[index + 1]?.[1] ?? s, o: { x: [.32], y: [0] }, i: { x: [.68], y: [1] } } : {}) })) });
function path(name: string, points: number[][], closed = false): Shape { return { ty: 'sh', nm: name, ks: { a: 0, k: { v: points, i: points.map(() => [0, 0]), o: points.map(() => [0, 0]), c: closed } } }; }
function curve(name: string, v: number[][], i: number[][], o: number[][], c = false): Shape { return { ty: 'sh', nm: name, ks: { a: 0, k: { v, i, o, c } } }; }
const stroke = (): Shape => ({ ty: 'st', nm: 'Original outline', c: fixed([1, 1, 1, 1]), o: fixed(100), w: fixed(1.7), lc: 2, lj: 2 });
const fill = (): Shape => ({ ty: 'fl', nm: 'Ink', c: fixed([1, 1, 1, 1]), o: fixed(100), r: 1 });
const circle = (x: number, y: number, size: number): Shape => ({ ty: 'el', nm: 'Circle', p: fixed([x, y]), s: fixed([size, size]) });
const rect = (x: number, y: number, width: number, height: number, radius: number): Shape => ({ ty: 'rc', nm: 'Rounded frame', p: fixed([x, y]), s: fixed([width, height]), r: fixed(radius) });
const trim = (duration: number): Shape => ({ ty: 'tm', nm: 'Ink reveal', s: fixed(0), e: frames([[0, [0]], [duration * .66, [100]], [duration, [100]]]), o: fixed(0), m: 1 });
function layer(name: string, shapes: Shape[], overrides: Partial<LottieLayer['ks']> = {}, duration = 65): LottieLayer {
    return { ty: 4, ind: 0, nm: name, ddd: 0, sr: 1, ip: 0, op: duration, st: 0, ks: { o: fixed(100), r: fixed(0), p: fixed([12, 12, 0]), a: fixed([12, 12, 0]), s: fixed([100, 100, 100]), ...overrides }, shapes };
}
function animation(name: string, layers: LottieLayer[], duration = 65): LottieAnimation {
    return { v: '5.12.2', fr: 60, ip: 0, op: duration, w: 24, h: 24, nm: `Hatchling original · ${name}`, ddd: 0, assets: [], layers: layers.map((item, index) => ({ ...item, ind: index + 1, op: duration })) };
}
/** Hand-authored coordinates, not copied/traced from the reference-only kit. */
export const animationLibrary: Partial<Record<IconName, LottieAnimation>> = {
    chat: animation('Chat typing wave', [
        layer('Speech outline', [curve('Speech', [[12, 3], [21, 11], [12, 20], [7, 19], [3, 21], [4, 16], [3, 11]], [[-5, 0], [0, -5], [5, 0], [2, 1], [0, 0], [0, 0], [0, 2]], [[5, 0], [0, 5], [-2, 0], [0, 0], [0, 0], [-1, -2], [0, -5]], true), stroke()]),
        ...[7.5, 12, 16.5].map((x, index) => layer(`Typing dot ${index + 1}`, [circle(x, 11, 1.9), fill()], { o: frames([[0, [0]], [7 + index * 5, [0]], [13 + index * 5, [100]], [42 + index * 5, [100]], [58, [0]], [65, [0]]]), p: frames([[0, [12, 12, 0]], [15 + index * 5, [12, 10, 0]], [29 + index * 5, [12, 12, 0]], [65, [12, 12, 0]]]) })),
    ]),
    feed: animation('Feed ink reveal', [layer('Back sheet', [path('Back', [[5, 6], [3, 6], [3, 21], [16, 21]]), stroke()]), layer('Front sheet', [rect(13, 10, 14, 18, 2), stroke()]), layer('Headline', [path('Line one', [[10, 6], [17, 6]]), path('Line two', [[10, 10], [16, 10]]), path('Line three', [[10, 14], [13, 14]]), stroke(), trim(65)])]),
    goal: animation('Goals check draw', [layer('Checkbox', [rect(12, 12, 18, 18, 3), stroke()]), layer('Check', [path('Tick', [[7, 12], [10.5, 15.5], [17, 8]]), stroke(), trim(65)])]),
    idea: animation('Ideas lightbulb wobble', [layer('Bulb', [curve('Bulb outline', [[8, 16], [6, 8], [12, 3], [18, 8], [16, 16]], [[0, -3], [0, 5], [-3.5, 0], [0, -3.5], [0, -3]], [[0, -3], [0, -3.5], [3.5, 0], [0, 5], [0, 0]], false), path('Base', [[8, 16], [16, 16], [15, 20], [9, 20], [8, 16]]), path('Foot', [[10, 22], [14, 22]]), stroke()], { a: fixed([12, 18, 0]), p: fixed([12, 18, 0]), r: frames([[0, [0]], [13, [-12]], [28, [9]], [44, [-4]], [64, [0]]]) })]),
    library: animation('Library shapes cycle', [
        layer('Diamond', [path('Diamond', [[6, 2], [10, 6], [6, 10], [2, 6]], true), stroke()], { a: fixed([6, 6, 0]), p: fixed([6, 6, 0]), r: frames([[0, [0]], [14, [90]], [40, [180]], [64, [180]]]), s: frames([[0, [100, 100, 100]], [12, [55, 55, 100]], [28, [108, 108, 100]], [64, [100, 100, 100]]]) }),
        layer('Triangle', [path('Triangle', [[18, 2], [22, 10], [14, 10]], true), stroke()], { a: fixed([18, 7, 0]), p: fixed([18, 7, 0]), s: frames([[0, [100, 100, 100]], [24, [45, 45, 100]], [40, [108, 108, 100]], [64, [100, 100, 100]]]) }),
        layer('Circle', [circle(6, 18, 7), stroke()], { a: fixed([6, 18, 0]), p: fixed([6, 18, 0]), s: frames([[0, [100, 100, 100]], [32, [50, 50, 100]], [48, [115, 115, 100]], [64, [100, 100, 100]]]) }),
        layer('Square', [rect(18, 18, 7, 7, 1), stroke()], { a: fixed([18, 18, 0]), p: fixed([18, 18, 0]), r: frames([[0, [0]], [26, [0]], [48, [90]], [64, [90]]]) }),
    ]),
    search: animation('Search lens glance', [layer('Magnifier', [circle(10, 10, 15), path('Handle', [[15.5, 15.5], [21, 21]]), stroke()], { p: frames([[0, [12, 12, 0]], [14, [13, 11, 0]], [31, [11, 12, 0]], [50, [12, 12, 0]]]) }, 51)], 51),
    arrow: animation('Compose stroke', [layer('Send', [path('Stem', [[12, 20], [12, 4]]), path('Head', [[6, 10], [12, 4], [18, 10]]), stroke(), trim(82)], {}, 141)], 141),
    spark: animation('Hatching beat', [layer('Shell', [path('Egg shell', [[4, 13], [8, 10], [12, 14], [16, 10], [20, 13], [18, 20], [6, 20]], true), stroke()], { s: frames([[0, [100, 100, 100]], [60, [86, 86, 100]], [145, [110, 110, 100]], [240, [100, 100, 100]], [300, [100, 100, 100]]]), r: frames([[0, [0]], [55, [-8]], [100, [8]], [160, [0]], [300, [0]]]) }, 300), layer('Chick', [circle(12, 7, 7), circle(10.7, 6.5, .7), circle(13.3, 6.5, .7), stroke()], { p: frames([[0, [12, 15, 0]], [100, [12, 11, 0]], [180, [12, 12, 0]], [300, [12, 12, 0]]]) }, 300)], 300),
};
