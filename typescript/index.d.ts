import type { CallToolRequest, CallToolResult, InputRequiredResult, InputResponses, RequestOptions } from "@modelcontextprotocol/client";

export type CallOptions = Omit<RequestOptions, "allowInputRequired">;
export type CallMetadata = Omit<CallToolRequest["params"], "name" | "arguments"> & { inputResponses?: InputResponses; requestState?: string };
type Fields<T> = { [K in keyof T as string extends K ? never : number extends K ? never : K]: T[K] };
export type ToolResult<Output> =
  | (Omit<Fields<CallToolResult>, "structuredContent" | "isError"> & { resultType: "complete"; isError?: false; structuredContent: Output })
  | (Omit<Fields<CallToolResult>, "structuredContent" | "isError"> & { resultType: "complete"; isError: true; structuredContent?: unknown })
  | InputRequiredResult;

export declare class ToolCallError extends Error {
  readonly code: "tool_error" | "input_required";
  readonly app: string;
  readonly tool: string;
  readonly result: Extract<ToolResult<unknown>, { isError: true } | { resultType: "input_required" }>;
  constructor(app: string, tool: string, result: ToolCallError["result"]);
}

export interface ToolClient<I, O> {
  readonly tools: { readonly [K in Exclude<keyof I, "then" | "toJSON" | "toString" | "valueOf" | "toLocaleString">]: (input: I[K], options?: CallOptions) => Promise<K extends keyof O ? O[K] : unknown> };
  readonly callTool: <K extends keyof I & string>(request: CallMetadata & { name: K; arguments: I[K] }, options?: CallOptions) => Promise<ToolResult<K extends keyof O ? O[K] : unknown>>;
}

export interface Inputs {}
export interface Outputs {}
export interface InputTypes {}
export interface OutputTypes {}

export declare function mcp<K extends keyof Inputs & string>(name: K, options?: { url?: string; headers?: Record<string, string>; signal?: AbortSignal }): Promise<ToolClient<Inputs[K], K extends keyof Outputs ? Outputs[K] : {}> & { close(): Promise<void> }>;
