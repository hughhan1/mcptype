import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { mcp, ToolCallError } from "mcptype";
import { compile } from "mcptype/compiler";

const binary = fileURLToPath(new URL("../../target/release/mcptype", import.meta.url));
const server = fileURLToPath(new URL("../../tests/server.mjs", import.meta.url));
const root = fileURLToPath(new URL("..", import.meta.resolve("mcptype")));
function cli(cwd, args) {
  const result = spawnSync(binary, args, {cwd,encoding:"utf8",timeout:30000});
  assert.equal(result.status, 0, result.stderr);
}
function project(t) {
  const directory = mkdtempSync(join(tmpdir(), "mcptype-"));
  t.after(() => rmSync(directory, {recursive:true,force:true}));
  return directory;
}
async function http(t, ...flags) {
  const child = spawn(process.execPath, [server,"--http",...flags], {stdio:["ignore","pipe","inherit"]});
  t.after(async () => { child.kill("SIGTERM"); await once(child,"exit"); });
  const [line] = await once(child.stdout,"data");
  return String(line).trim();
}

test("stdio and HTTP discovery produce clients with pinned validation and raw MCP results", {timeout:60000}, async t => {
  const directory = project(t);
  cli(directory, ["add","files","stdio",process.execPath,server]);
  cli(directory, ["add","remote","http",await http(t)]);
  const previous = process.cwd();
  process.chdir(directory);
  t.after(() => process.chdir(previous));
  for (const name of ["files","remote"]) {
    const client = await mcp(name);
    try {
      assert.deepEqual(await client.tools.read_text_file({path:"hello"}), {content:"hello"});
      const raw = await client.callTool({name:"read_text_file",arguments:{path:"hello"}});
      assert.equal(raw.content[0].text,"hello");
      await assert.rejects(client.tools.read_text_file({path:1}), /Invalid arguments/);
      await assert.rejects(client.tools.fail({}), error => error instanceof ToolCallError && error.result.isError);
      if (name === "remote") assert.equal((await client.callTool({name:"ask",arguments:{}})).resultType,"input_required");
    } finally { await client.close(); }
  }
  const lock = JSON.parse(readFileSync("mcptype.lock.json","utf8"));
  lock.bindings.files.endpoint.args.push("--number");
  writeFileSync("mcptype.lock.json",JSON.stringify(lock));
  const drifted = await mcp("files");
  try { await assert.rejects(drifted.tools.read_text_file({path:"hello"}), /Invalid structured output/); }
  finally { await drifted.close(); }
  cli(directory,["check"]);
  writeFileSync("mcptype-env.d.ts", "stale");
  const stale = spawnSync(binary,["check"],{cwd:directory,encoding:"utf8"});
  assert.notEqual(stale.status,0);
  assert.match(stale.stderr,/stale declarations/);
});

test("generated declarations, from the CLI or the native compiler, enforce inputs and outputs", async t => {
  const directory = project(t);
  cli(directory,["add","files","stdio",process.execPath,server]);
  const { bindings } = JSON.parse(readFileSync(join(directory,"mcptype.lock.json"),"utf8"));
  const contracts = Object.fromEntries(Object.entries(bindings).map(([name, binding]) => [name, binding.contract]));
  assert.equal((await compile({ language: "typescript", contracts })).declarations, readFileSync(join(directory,"mcptype-env.d.ts"),"utf8"));
  mkdirSync(join(directory,"node_modules"),{recursive:true});
  symlinkSync(root,join(directory,"node_modules/mcptype"));
  writeFileSync(join(directory,"package.json"),'{"type":"module"}');
  writeFileSync(join(directory,"app.ts"), `import {mcp} from "mcptype";
const files = await mcp("files");
(await files.tools.read_text_file({path:"README.md"})).content.toUpperCase();
// @ts-expect-error
await mcp("missing");
// @ts-expect-error
await files.tools.read_text_file({path:1});
// @ts-expect-error
await files.tools.read_text_file({});
// @ts-expect-error
(await files.tools.read_text_file({path:"README.md"})).content.toFixed();
// @ts-expect-error
(await files.tools.fail({})).content;
await files.close();
`);
  const program = ts.createProgram([join(directory,"app.ts"),join(directory,"mcptype-env.d.ts")], {noEmit:true,strict:true,skipLibCheck:true,target:ts.ScriptTarget.ES2023,module:ts.ModuleKind.NodeNext,moduleResolution:ts.ModuleResolutionKind.NodeNext});
  const errors = ts.getPreEmitDiagnostics(program);
  assert.equal(errors.length,0,ts.formatDiagnostics(errors,{getCurrentDirectory:()=>directory,getCanonicalFileName:p=>p,getNewLine:()=>"\n"}));
});

test("HTTP registration supports legacy servers returning discovery errors over SSE", async t => {
  const directory = project(t);
  cli(directory,["add","files","http",await http(t,"--legacy-sse")]);
  const lock = JSON.parse(readFileSync(join(directory,"mcptype.lock.json"),"utf8"));
  assert.equal(lock.bindings.files.contract.tools.find(tool => tool.name === "read_text_file").inputSchema.properties.path.type,"string");
});
