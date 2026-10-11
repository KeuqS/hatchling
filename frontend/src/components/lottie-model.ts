/** Deliberately small, interoperable Lottie subset for our own icon assets. No expressions or external assets. */
export interface Keyframe {
    t: number;
    s: number[];
    e?: number[];
    h?: 0 | 1;
    i?: {
        x: number[];
        y: number[];
    };
    o?: {
        x: number[];
        y: number[];
    };
}
export type Property = {
    a: 0;
    k: number | number[];
} | {
    a: 1;
    k: Keyframe[];
};
export interface BezierShape {
    v: number[][];
    i: number[][];
    o: number[][];
    c: boolean;
}
export type Shape = {
    ty: 'sh';
    ks: {
        a: 0;
        k: BezierShape;
    };
    nm: string;
} | {
    ty: 'el';
    p: Property;
    s: Property;
    nm: string;
} | {
    ty: 'rc';
    p: Property;
    s: Property;
    r: Property;
    nm: string;
} | {
    ty: 'st';
    c: Property;
    o: Property;
    w: Property;
    lc: 2;
    lj: 2;
    nm: string;
} | {
    ty: 'fl';
    c: Property;
    o: Property;
    r: 1;
    nm: string;
} | {
    ty: 'tm';
    s: Property;
    e: Property;
    o: Property;
    m: 1;
    nm: string;
};
export interface LottieLayer {
    ty: 4;
    ind: number;
    nm: string;
    ddd: 0;
    sr: 1;
    ip: number;
    op: number;
    st: 0;
    ks: {
        o: Property;
        r: Property;
        p: Property;
        a: Property;
        s: Property;
    };
    shapes: Shape[];
}
export interface LottieAnimation {
    v: '5.12.2';
    fr: 60;
    ip: 0;
    op: number;
    w: 24;
    h: 24;
    nm: string;
    ddd: 0;
    assets: never[];
    layers: LottieLayer[];
}
