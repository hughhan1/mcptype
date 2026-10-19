import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import ts from "typescript";
const { mcp, ToolCallError } = await import(process.env.MCPTYPE_PACKAGE_TEST ? "mcptype" : "../typescript/index.mjs");

const binary = process.env.MCPTYPE_BINARY ?? resolve("target/release/mcptype");
const server = resolve("tests/server.mjs");
const root = process.env.MCPTYPE_PACKAGE_TEST ? fileURLToPath(new URL("..", import.meta.resolve("mcptype"))) : resolve(".");
function cli(cwd, args) {
  const result = spawnSync(binary, args, {cwd,encoding:"utf8",timeout:30000});
  assert.equal(result.status, 0, result.stderr);
}

test("stdio and HTTP discovery produce clients with pinned validation and raw MCP results", {timeout:60000}, async t => {
  const directory = mkdtempSync(join(tmpdir(), "mcptype-client-"));
  t.after(() => rmSync(directory, {recursive:true,force:true}));
  cli(directory, ["add","files","stdio",process.execPath,server]);
  const http = spawn(process.execPath, [server,"--http"], {stdio:["ignore","pipe","inherit"]});
  t.after(async () => { http.kill("SIGTERM"); await once(http,"exit"); });
  const [line] = await once(http.stdout,"data");
  cli(directory, ["add","remote","http",String(line).trim()]);
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
      const asked = await client.callTool({name:"ask",arguments:{}});
      if (name === "remote") assert.equal(asked.resultType,"input_required");
      else assert.equal(asked.isError,true);
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

test("ordinary TypeScript imports enforce generated inputs and outputs", async t => {
  const directory = mkdtempSync(join(tmpdir(), "mcptype-types-"));
  t.after(() => rmSync(directory, {recursive:true,force:true}));
  cli(directory,["add","files","stdio",process.execPath,server]);
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
  const directory = mkdtempSync(join(tmpdir(), "mcptype-legacy-"));
  t.after(() => rmSync(directory, {recursive:true,force:true}));
  const http = spawn(process.execPath, [server,"--http","--legacy-sse"], {stdio:["ignore","pipe","inherit"]});
  t.after(async () => { http.kill("SIGTERM"); await once(http,"exit"); });
  const [line] = await once(http.stdout,"data");
  cli(directory,["add","files","http",String(line).trim()]);
  const lock = JSON.parse(readFileSync(join(directory,"mcptype.lock.json"),"utf8"));
  assert.equal(lock.bindings.files.contract.tools.find(tool => tool.name === "read_text_file").inputSchema.properties.path.type,"string");
});

test("native compiler produces checked declarations without a CLI on PATH", async t => {
  const { compile } = await import(process.env.MCPTYPE_PACKAGE_TEST ? "mcptype/compiler" : "../typescript/compiler.mjs");
  const path = process.env.PATH;
  process.env.PATH = "";
  t.after(() => { process.env.PATH = path; });
  await assert.rejects(compile({ language: "typescript", contracts: { files: { tools: [{ name: "read", inputSchema: { type: "string" } }] } } }), /inputSchema must have type object/);
  const { declarations } = await compile({ language: "typescript", contracts: { files: { tools: [{ name: "read", inputSchema: { type: "object", properties: { path: { type: "string" } }, required: ["path"] } }] } } });
  const directory = mkdtempSync(join(tmpdir(), "mcptype-native-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const source = join(directory, "types.ts");
  writeFileSync(source, declarations + '\nconst valid: Inputs["files"]["read"] = {path: "README.md"};\n// @ts-expect-error\nconst invalid: Inputs["files"]["read"] = {path: 1};\n');
  const program = ts.createProgram([source], { noEmit: true, strict: true, skipLibCheck: true });
  assert.deepEqual(ts.getPreEmitDiagnostics(program).map(d => ts.flattenDiagnosticMessageText(d.messageText, "\n")), []);
});
