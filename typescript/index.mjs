import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import { validator } from "./validate.mjs";

export class ToolCallError extends Error {
  constructor(app, tool, result) {
    const code = result.resultType === "input_required" ? "input_required" : "tool_error";
    super(`${app}.${tool}: ${code}`);
    Object.assign(this, { name: "ToolCallError", app, tool, code, result });
  }
}

function binding(name) {
  let root = resolve(process.cwd());
  while (true) {
    try {
      const lock = JSON.parse(readFileSync(join(root, "mcptype.lock.json"), "utf8"));
      if (lock.version !== 1) throw new Error("unsupported mcptype.lock.json version");
      if (!Object.hasOwn(lock.bindings, name)) throw new Error(`Unknown binding ${name}; run mcptype add first.`);
      return { ...lock.bindings[name], root };
    } catch (error) {
      if (error.code !== "ENOENT" || dirname(root) === root) throw error;
      root = dirname(root);
    }
  }
}

export async function mcp(name, { url, headers, signal } = {}) {
  const { endpoint: saved, contract, root } = binding(name);
  const endpoint = url ? { type: "http", url } : saved;
  const transport = endpoint.type === "http"
    ? new StreamableHTTPClientTransport(new URL(endpoint.url), { requestInit: { headers } })
    : new StdioClientTransport({ command: endpoint.command, args: endpoint.args, cwd: root });
  const definitions = new Map(contract.tools.map(tool => [tool.name, { definition: tool, input: validator(tool.inputSchema), output: tool.outputSchema ? validator(tool.outputSchema) : undefined }]));
  const client = new Client({ name: "mcptype", version: "0.0.1" }, { versionNegotiation: { mode: "auto" } });
  try { await client.connect(transport, { signal }); }
  catch (error) { await client.close(); throw error; }
  const callTool = async (request, options) => {
    const tool = definitions.get(request.name);
    if (!tool) throw new Error(`Unknown tool ${name}.${request.name}`);
    if (!tool.input(request.arguments)) throw new Error(`Invalid arguments for ${name}.${request.name}: ${JSON.stringify(tool.input.errors)}`);
    const result = await client.callTool(request, { ...options, allowInputRequired: true, toolDefinition: { ...tool.definition, outputSchema: undefined } });
    if (result.resultType === "input_required") return result;
    if (!result.isError && tool.output && !tool.output(result.structuredContent)) throw new Error(`Invalid structured output from ${name}.${request.name}: ${JSON.stringify(tool.output.errors)}`);
    return { ...result, resultType: "complete" };
  };
  const tools = Object.create(null);
  for (const tool of contract.tools) {
    if (["then", "toJSON", "toString", "valueOf", "toLocaleString"].includes(tool.name)) continue;
    tools[tool.name] = async (arguments_, options) => {
      const result = await callTool({ name: tool.name, arguments: arguments_ }, options);
      if (result.resultType === "input_required" || result.isError) throw new ToolCallError(name, tool.name, result);
      return result.structuredContent;
    };
  }
  return Object.freeze({ tools: Object.freeze(tools), callTool, close: () => client.close() });
}
