// The runtime check for frames off the wire . A frame is untrusted
// until it conforms to the schema its types were generated from, so a decoded
// value is exactly what the generated type says, never a cast. This interprets
// the JSON Schema keywords scripts/generate-protocol.mjs lets into the wire
// table, and nothing else: the generator refuses any other keyword.
/** One schema node, as the wire table holds it. */
export type SchemaNode = boolean | {
    readonly $ref?: string;
    readonly type?: string | readonly string[];
    readonly const?: string | number | boolean | null;
    readonly properties?: Readonly<Record<string, SchemaNode>>;
    readonly required?: readonly string[];
    readonly additionalProperties?: SchemaNode;
    readonly oneOf?: readonly SchemaNode[];
    readonly anyOf?: readonly SchemaNode[];
    readonly items?: SchemaNode;
    readonly minimum?: number;
    readonly maximum?: number;
};
const DEFINITION = '#/$defs/';
function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function hasType(value: unknown, type: string): boolean {
    switch (type) {
        case 'string':
            return typeof value === 'string';
        case 'boolean':
            return typeof value === 'boolean';
        case 'null':
            return value === null;
        case 'integer':
            return Number.isInteger(value);
        case 'number':
            return Number.isFinite(value);
        case 'object':
            return isRecord(value);
        case 'array':
            return Array.isArray(value);
        default:
            return false;
    }
}
function fieldsConform(value: Readonly<Record<string, unknown>>, node: Exclude<SchemaNode, boolean>, definitions: Readonly<Record<string, SchemaNode>>): boolean {
    const properties = node.properties ?? {};
    if ((node.required ?? []).some(name => !Object.hasOwn(value, name)))
        return false;
    return Object.entries(value).every(([name, field]) => {
        const schema = Object.hasOwn(properties, name) ? properties[name] : node.additionalProperties;
        return schema === undefined || conforms(field, schema, definitions);
    });
}
/**
 * Whether a parsed JSON value conforms to a schema node. Every keyword on a
 * node applies, as in draft 2020-12, so a `$ref` with sibling properties
 * checks both. An unresolvable reference conforms to nothing.
 */
export function conforms(value: unknown, node: SchemaNode, definitions: Readonly<Record<string, SchemaNode>>): boolean {
    if (typeof node === 'boolean')
        return node;
    if (node.$ref !== undefined) {
        const target = node.$ref.startsWith(DEFINITION) && Object.hasOwn(definitions, node.$ref.slice(DEFINITION.length)) ? definitions[node.$ref.slice(DEFINITION.length)] : undefined;
        if (target === undefined || !conforms(value, target, definitions))
            return false;
    }
    if (node.type !== undefined && ![node.type].flat().some(type => hasType(value, type)))
        return false;
    if (node.const !== undefined && value !== node.const)
        return false;
    if (typeof value === 'number' && ((node.minimum !== undefined && value < node.minimum) || (node.maximum !== undefined && value > node.maximum)))
        return false;
    if (node.oneOf !== undefined && node.oneOf.filter(option => conforms(value, option, definitions)).length !== 1)
        return false;
    if (node.anyOf !== undefined && !node.anyOf.some(option => conforms(value, option, definitions)))
        return false;
    const items = node.items;
    if (items !== undefined && Array.isArray(value) && !value.every((item: unknown) => conforms(item, items, definitions)))
        return false;
    return !isRecord(value) || fieldsConform(value, node, definitions);
}
