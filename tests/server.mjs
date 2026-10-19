import { createServer } from "node:http";
import { McpServer, createMcpHandler } from "@modelcontextprotocol/server";
import { StdioServerTransport } from "@modelcontextprotocol/server/stdio";
import { toNodeHandler } from "@modelcontextprotocol/node";
import { z } from "zod";

function server() {
  const mcp = new McpServer({ name: "mcptype-fixture", version: "1" });
  const numeric = process.argv.includes("--number");
  mcp.registerTool("read_text_file", { inputSchema: z.strictObject({ path: z.string() }), outputSchema: z.object({ content: numeric ? z.number() : z.string() }) }, async ({path}) => ({ content: [{type:"text", text:path}], structuredContent: {content: numeric ? 42 : path} }));
  mcp.registerTool("fail", { inputSchema: z.strictObject({}) }, async () => ({isError:true, content:[{type:"text",text:"failed"}]}));
  mcp.registerTool("ask", { inputSchema: z.strictObject({}) }, async () => ({resultType:"input_required",requestState:"resume"}));
  return mcp;
}

if (process.argv.includes("--http")) {
  const legacy = process.argv.includes("--legacy-sse");
  const handler = createMcpHandler(server, { legacy: legacy ? "stateless" : "reject" });
  const dispatch = toNodeHandler({fetch: async (request, options) => {
    if (legacy && request.method === "POST") {
      const message = await request.clone().json();
      if (message.method === "server/discover") {
        return new Response(`event: message\ndata: ${JSON.stringify({jsonrpc:"2.0",id:message.id,error:{code:-32601,message:"Method not found"}})}\n\n`, {headers:{"content-type":"text/event-stream"}});
      }
    }
    return handler.fetch(request, options);
  }});
  const http = createServer((req, res) => { void dispatch(req, res).catch(() => res.destroy()); });
  http.listen(0, "127.0.0.1", () => console.log(`http://127.0.0.1:${http.address().port}/mcp`));
  process.on("SIGTERM", async () => { await handler.close(); http.closeAllConnections(); http.close(); });
} else {
  await server().connect(new StdioServerTransport());
}
