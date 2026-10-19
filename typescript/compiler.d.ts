export interface CompileRequest {
  language: "typescript" | "python";
  contracts: Record<string, { tools: readonly { name: string; inputSchema: Record<string, unknown>; outputSchema?: Record<string, unknown> }[] }>;
}
export function compile(request: CompileRequest): Promise<{ declarations: string }>;
