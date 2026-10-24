export type JsonSchema = Record<string, unknown>;
export interface Identity { name: string; version: string }
export interface Tool { name: string; inputSchema: JsonSchema; outputSchema?: JsonSchema }
export interface Resource { uri: string; outputSchema: JsonSchema }
export interface Contract { tools: Tool[]; resources?: Resource[] }
export interface Expectation { identity: Identity; contract: Contract }
export interface Metadata {
  version: 1;
  identity: Identity;
  contract?: Contract;
  requires?: Record<string, Expectation>;
}
export interface CompileRequest {
  language: "typescript" | "python" | "go";
  contracts: Record<string, { tools: readonly { name: string; inputSchema: JsonSchema; outputSchema?: JsonSchema }[] }>;
}
export type Change =
  | { kind: "no_contract"; text: string }
  | { kind: "identity"; expected: Identity; published: Identity; text: string }
  | { kind: "tool_removed" | "tool_input_changed" | "tool_output_changed"; name: string; text: string }
  | { kind: "resource_removed" | "resource_output_changed"; uri: string; text: string };
export interface GenerateRequest {
  target: "node" | "browser" | "python" | "go";
  identity: Identity;
  requires?: Record<string, Identity>;
  contracts?: Record<string, Contract>;
  /** Files copied into the package verbatim; strings are written as UTF-8. */
  runtime?: Record<string, string | Uint8Array>;
  protocol?: string;
  runtime_specifier?: string;
  browser_runtime_specifier?: string;
  protocol_specifier?: string;
  python_module?: string;
  python_runtime_module?: string;
  banner?: string;
  package_fields?: Record<string, unknown>;
  go_module?: string;
}
export type CanonicalRequest = { contract: Contract } | { schema: JsonSchema } | { metadata: Metadata };
export function compile(request: CompileRequest): Promise<{ declarations: string; bindings?: Record<string, unknown> }>;
export function canonical(request: { contract: Contract }): Promise<{ canonical: Contract; digest: string }>;
export function canonical(request: { schema: JsonSchema }): Promise<{ canonical: JsonSchema; digest: string }>;
export function canonical(request: { metadata: Metadata }): Promise<{ canonical: Contract | null; digest: string | null }>;
export function changes(request: { expected: Expectation; published: Metadata | null }): Promise<{ changes: Change[] }>;
export function generate(request: GenerateRequest): Promise<{ files: Record<string, Buffer> }>;
