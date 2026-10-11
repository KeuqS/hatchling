// Generates the live adapter's wire types from protocol/schema.json .
//
// The schema is itself generated, from hatchling-core's Rust types, by
// `cargo run -p hatchling-dev -- protocol-schema --write`. This script turns it
// into src/adapters/live/protocol/generated.ts. `--check` regenerates in memory
// and fails if the checked-in file differs: hand edits on either side are drift.
//
// Deterministic by construction: definitions and properties are emitted in
// sorted order whatever order the input lists them in, and the text goes
// through the same TypeScript printer as scripts/format.mjs. Unknown schema
// keywords are refused, never skipped: a construct this script does not
// understand would otherwise become a silently wrong type.
//
// It also writes src/adapters/live/protocol/wire.ts: the shapes the
// UI frames reach, as data, so the live adapter checks every frame it receives
// against the same schema its types come from before trusting it.
import ts from 'typescript';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
export const SCHEMA_PATH = 'protocol/schema.json';
export const OUTPUT_PATH = 'src/adapters/live/protocol/generated.ts';
export const WIRE_PATH = 'src/adapters/live/protocol/wire.ts';
/** The frames the UI exchanges: the wire table holds what these reach. */
export const WIRE_ROOTS = ['UiRequest', 'UiResponse'];
/** Keywords the runtime check (src/adapters/live/protocol/conform.ts) interprets. */
const CHECKED = new Set(['type', 'const', 'properties', 'required', '$ref', 'oneOf', 'anyOf', 'items', 'additionalProperties', 'minimum', 'maximum']);
/** Documentation only: dropped from the wire table. */
const PROSE = new Set(['description', 'title', 'format', 'default']);
/** Keywords that shape a type. */
const SHAPE = new Set(['type', 'const', 'properties', 'required', '$ref', 'oneOf', 'anyOf', 'items', 'prefixItems', 'additionalProperties']);
/** Keywords that only describe or constrain values, carried as documentation or dropped. */
const NOTE = new Set(['description', 'title', 'format', 'minimum', 'maximum', 'minItems', 'maxItems', 'default']);
const identifier = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
function fail(path, message) {
    throw new Error(`${path}: ${message}`);
}
function sorted(object) {
    return Object.keys(object).sort();
}
function quote(text) {
    return `'${text.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n')}'`;
}
function propertyName(name) {
    return identifier.test(name) ? name : quote(name);
}
function doc(description, indent = '') {
    if (typeof description !== 'string' || description.trim() === '')
        return '';
    const lines = description.replace(/\*\//g, '* /').split('\n');
    return `${indent}/**\n${lines.map(line => `${indent} * ${line}`.trimEnd()).join('\n')}\n${indent} */\n`;
}
function primitive(type, path) {
    switch (type) {
        case 'string': return 'string';
        case 'integer':
        case 'number': return 'number';
        case 'boolean': return 'boolean';
        case 'null': return 'null';
        case 'object': return 'object';
        case 'array': return 'readonly unknown[]';
        default: return fail(path, `unsupported type ${JSON.stringify(type)}`);
    }
}
function literal(value, path) {
    if (typeof value === 'string')
        return quote(value);
    if (typeof value === 'number' || typeof value === 'boolean' || value === null)
        return JSON.stringify(value);
    return fail(path, 'a const must be a string, number, boolean or null');
}
/** No parentheses needed: members are literals, names, object types and
 * intersections, and `&` binds tighter than `|`. */
function union(parts) {
    return [...new Set(parts)].join(' | ');
}
function objectType(schema, path) {
    const required = new Set(schema.required ?? []);
    const properties = schema.properties ?? {};
    for (const name of required)
        if (!(name in properties))
            fail(path, `required property ${JSON.stringify(name)} is not declared`);
    const members = sorted(properties).map(name => {
        const property = properties[name];
        const optional = required.has(name) ? '' : '?';
        return `${doc(property.description, '    ')}    readonly ${propertyName(name)}${optional}: ${typeOf(property, `${path}/properties/${name}`)};`;
    });
    const extra = schema.additionalProperties;
    if (extra !== undefined && extra !== false)
        members.push(`    readonly [key: string]: ${extra === true ? 'unknown' : typeOf(extra, `${path}/additionalProperties`)};`);
    return members.length === 0 ? 'Readonly<Record<string, never>>' : `{\n${members.join('\n')}\n}`;
}
/** The TypeScript type for one schema node. */
export function typeOf(schema, path) {
    if (schema === true)
        return 'unknown';
    if (schema === false)
        return 'never';
    if (typeof schema !== 'object' || schema === null || Array.isArray(schema))
        return fail(path, 'a schema must be an object or a boolean');
    for (const key of Object.keys(schema))
        if (!SHAPE.has(key) && !NOTE.has(key))
            fail(path, `unsupported schema keyword ${JSON.stringify(key)}`);
    if ('$ref' in schema) {
        const match = /^#\/\$defs\/([A-Za-z_][A-Za-z0-9_]*)$/.exec(schema.$ref);
        if (!match)
            fail(path, `unsupported reference ${JSON.stringify(schema.$ref)}`);
        const others = Object.keys(schema).filter(key => key !== '$ref' && !NOTE.has(key));
        if (others.length === 0)
            return match[1];
        // Draft 2020-12 applies a $ref and its siblings together. serde's
        // internally tagged newtype variants come out this way: the inner
        // type, plus the tag field beside it.
        if (others.some(key => !['properties', 'required', 'type'].includes(key)) || (schema.type !== undefined && schema.type !== 'object'))
            fail(path, `$ref with ${others.join(', ')} is not supported`);
        return `${match[1]} & ${objectType(schema, path)}`;
    }
    if ('const' in schema)
        return literal(schema.const, path);
    const alternatives = schema.oneOf ?? schema.anyOf;
    if ('oneOf' in schema && 'anyOf' in schema)
        fail(path, 'oneOf and anyOf together are not supported');
    const hasProperties = 'properties' in schema || 'required' in schema || 'additionalProperties' in schema;
    if (alternatives !== undefined) {
        if (!Array.isArray(alternatives) || alternatives.length === 0)
            fail(path, 'oneOf/anyOf must be a non-empty array');
        const options = union(alternatives.map((alternative, index) => typeOf(alternative, `${path}/${'oneOf' in schema ? 'oneOf' : 'anyOf'}/${index}`)));
        // A flattened enum: shared fields beside the alternatives.
        return hasProperties ? `${objectType(schema, path)} & (${options})` : options;
    }
    if (hasProperties)
        return objectType(schema, path);
    if ('prefixItems' in schema) {
        if (!Array.isArray(schema.prefixItems))
            fail(path, 'prefixItems must be an array');
        return `readonly [${schema.prefixItems.map((item, index) => typeOf(item, `${path}/prefixItems/${index}`)).join(', ')}]`;
    }
    if ('items' in schema)
        return `readonly (${typeOf(schema.items, `${path}/items`)})[]`;
    if ('type' in schema) {
        const types = Array.isArray(schema.type) ? schema.type : [schema.type];
        if (types.length === 0)
            fail(path, 'type must not be empty');
        return union(types.map(type => primitive(type, path)));
    }
    return 'unknown';
}
/** Format TypeScript text exactly as scripts/format.mjs does. */
export function format(text) {
    const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed, removeComments: false });
    const print = source => printer.printFile(ts.createSourceFile(OUTPUT_PATH, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS));
    let clean = print(text);
    for (let pass = 0; pass < 3; pass++) {
        const next = print(clean);
        if (next === clean)
            break;
        clean = next;
    }
    return clean.replace(/[\t ]+$/gm, '').replace(/\n*$/, '\n');
}
/** The generated module for a parsed schema document. */
export function generate(document) {
    if (typeof document !== 'object' || document === null)
        fail('#', 'the schema document must be an object');
    const meta = document['x-hatchling'];
    const defs = document.$defs;
    if (typeof meta !== 'object' || meta === null)
        fail('#', 'missing x-hatchling');
    if (typeof defs !== 'object' || defs === null)
        fail('#', 'missing $defs');
    const { ui_schema_version: uiVersion, session_protocol: sessionVersion, incompatible } = meta;
    if (!Number.isInteger(uiVersion) || !Number.isInteger(sessionVersion))
        fail('#/x-hatchling', 'the versions must be integers');
    if (typeof incompatible?.frontend_newer !== 'string' || typeof incompatible?.backend_newer !== 'string')
        fail('#/x-hatchling/incompatible', 'both mismatch templates are required');
    const transitions = meta.control?.transitions;
    if (!Array.isArray(transitions) || transitions.some(row => [row?.from, row?.handoff, row?.to].some(value => typeof value !== 'string')))
        fail('#/x-hatchling/control', 'the control transition table is required, as { from, handoff, to } strings');
    const parts = [
        '// Generated by scripts/generate-protocol.mjs from protocol/schema.json, which is',
        '// generated from hatchling-core by `cargo run -p hatchling-dev -- protocol-schema --write`.',
        '// Do not edit: regenerate with `npm run protocol:generate`. `npm run protocol:check` fails on drift.',
        '/** The frontend protocol version this build speaks. */',
        `export const UI_SCHEMA_VERSION = ${uiVersion};`,
        '/** The broker session protocol version the schema was generated at. */',
        `export const SESSION_PROTOCOL_VERSION = ${sessionVersion};`,
        '/** The mismatch sentence when this frontend is the newer side. */',
        `export const INCOMPATIBLE_FRONTEND_NEWER = ${quote(incompatible.frontend_newer)};`,
        '/** The mismatch sentence when the backend is the newer side. */',
        `export const INCOMPATIBLE_BACKEND_NEWER = ${quote(incompatible.backend_newer)};`,
        '/** Every allowed control handoff, from hatchling-core: nothing else moves control. */',
        `export const CONTROL_TRANSITIONS: readonly { readonly from: Control; readonly handoff: Handoff; readonly to: Control }[] = [${transitions.map(row => `{ from: ${quote(row.from)}, handoff: ${quote(row.handoff)}, to: ${quote(row.to)} }`).join(', ')}];`,
    ];
    for (const name of sorted(defs)) {
        if (!identifier.test(name))
            fail(`#/$defs/${name}`, 'a definition name must be an identifier');
        const schema = defs[name];
        parts.push(`${doc(schema.description)}export type ${name} = ${typeOf(schema, `#/$defs/${name}`)};`);
    }
    return format(`${parts.join('\n')}\n`);
}
/** One schema node as wire-table data: prose dropped, keys sorted, and
 * anything the runtime check does not interpret refused. */
function checked(schema, path, reached) {
    if (typeof schema === 'boolean')
        return schema;
    if (typeof schema !== 'object' || schema === null || Array.isArray(schema))
        return fail(path, 'a schema must be an object or a boolean');
    const out = {};
    for (const key of sorted(schema)) {
        const value = schema[key];
        if (PROSE.has(key))
            continue;
        if (!CHECKED.has(key))
            fail(path, `the runtime check does not interpret ${JSON.stringify(key)}`);
        if (key === '$ref') {
            const match = /^#\/\$defs\/([A-Za-z_][A-Za-z0-9_]*)$/.exec(value);
            if (!match)
                fail(path, `unsupported reference ${JSON.stringify(value)}`);
            reached.push(match[1]);
            out[key] = value;
        }
        else if (key === 'properties')
            out[key] = Object.fromEntries(sorted(value).map(name => [name, checked(value[name], `${path}/properties/${name}`, reached)]));
        else if (key === 'oneOf' || key === 'anyOf')
            out[key] = value.map((option, index) => checked(option, `${path}/${key}/${index}`, reached));
        else if (key === 'items' || key === 'additionalProperties')
            out[key] = checked(value, `${path}/${key}`, reached);
        else
            out[key] = value;
    }
    return out;
}
/** The wire table: every definition the UI frames reach, as checkable data. */
export function wire(document) {
    const defs = document?.$defs;
    if (typeof defs !== 'object' || defs === null)
        fail('#', 'missing $defs');
    const table = {};
    const queue = [...WIRE_ROOTS];
    while (queue.length > 0) {
        const name = queue.shift();
        if (Object.hasOwn(table, name))
            continue;
        if (!Object.hasOwn(defs, name))
            fail(`#/$defs/${name}`, 'a referenced definition is missing');
        table[name] = checked(defs[name], `#/$defs/${name}`, queue);
    }
    const ordered = Object.fromEntries(sorted(table).map(name => [name, table[name]]));
    return format([
        '// Generated by scripts/generate-protocol.mjs from protocol/schema.json: the shapes',
        `// the UI frames (${WIRE_ROOTS.join(', ')}) reach, for the runtime check in conform.ts.`,
        '// Do not edit: regenerate with `npm run protocol:generate`. `npm run protocol:check` fails on drift.',
        "import type { SchemaNode } from './conform.js';",
        '/** Every definition the UI frames reach, keyed by name. */',
        `export const WIRE_DEFINITIONS: Readonly<Record<string, SchemaNode>> = ${JSON.stringify(ordered, null, 4)};`,
        '',
    ].join('\n'));
}
function main() {
    const document = JSON.parse(readFileSync(SCHEMA_PATH, 'utf8'));
    const outputs = [[OUTPUT_PATH, generate(document)], [WIRE_PATH, wire(document)]];
    if (process.argv.includes('--check')) {
        for (const [path, text] of outputs) {
            let current = '';
            try {
                current = readFileSync(path, 'utf8');
            }
            catch {
                current = '';
            }
            if (current !== text) {
                console.error(`${path} differs from ${SCHEMA_PATH}: run npm run protocol:generate (never edit it by hand).`);
                process.exitCode = 1;
                return;
            }
            console.log(`${path} matches ${SCHEMA_PATH}.`);
        }
        return;
    }
    for (const [path, text] of outputs) {
        writeFileSync(path, text);
        console.log(`Wrote ${path} from ${SCHEMA_PATH}.`);
    }
}
if (process.argv[1] === fileURLToPath(import.meta.url))
    main();
