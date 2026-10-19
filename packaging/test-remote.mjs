import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";

function run(command, args) {
  const result = spawnSync(command, args, {encoding:"utf8", timeout:120000});
  assert.equal(result.status, 0, result.stdout + result.stderr);
  return result.stdout;
}

for (const [name, url] of [["deepwiki","https://mcp.deepwiki.com/mcp"],["learn","https://learn.microsoft.com/api/mcp"]]) {
  run(process.env.MCPTYPE_BINARY,["add",name,"http",url]);
}
run(process.env.MCPTYPE_BINARY,["check"]);

const source = `import {mcp} from "mcptype";
const wiki = await mcp("deepwiki");
try {
  const result: string = (await wiki.tools.read_wiki_structure({repoName:"modelcontextprotocol/typescript-sdk"})).result;
  if (!result.length) throw new Error("empty wiki response");
} finally { await wiki.close(); }
const learn = await mcp("learn");
try {
  const result = await learn.tools.microsoft_docs_search({query:"Python virtual environments"});
  if (!result.results?.length) throw new Error("empty docs response");
} finally { await learn.close(); }
`;
writeFileSync("remote.ts",source);
writeFileSync("tsconfig.json",JSON.stringify({compilerOptions:{strict:true,skipLibCheck:true,target:"ES2023",module:"NodeNext",moduleResolution:"NodeNext",outDir:"out"},include:["remote.ts","mcptype-env.d.ts"]}));
run(process.execPath,["node_modules/typescript/bin/tsc"]);
run(process.execPath,["out/remote.js"]);
writeFileSync("remote.ts",source+'\n(await mcp("deepwiki")).tools.read_wiki_structure({repoName:42});\n');
const invalid = spawnSync(process.execPath,["node_modules/typescript/bin/tsc","--noEmit"],{encoding:"utf8"});
assert.notEqual(invalid.status,0);
assert.match(invalid.stdout,/number.*string/);
writeFileSync("remote.ts",source);

writeFileSync("pyproject.toml",'[tool.mypy]\nstrict = true\nmypy_path = "$MYPY_CONFIG_FILE_DIR/typings"\n[tool.pyright]\ntypeCheckingMode = "strict"\nstubPath = "typings"\n');
const python = process.env.MCPTYPE_TEST_PYTHON ?? ".venv/bin/python";
const py = `import asyncio
from mcptype import mcp

async def main() -> None:
    async with mcp("deepwiki") as wiki:
        text: str = (await wiki.tools.read_wiki_structure({"repoName":"modelcontextprotocol/typescript-sdk"}))["result"]
        assert len(text) > 0
    async with mcp("learn") as learn:
        result = await learn.tools.microsoft_docs_search({"query":"Python virtual environments"})
        assert len(result.get("results", [])) > 0

asyncio.run(main())
`;
writeFileSync("remote.py",py);
for (const checker of ["mypy","pyright"]) {
  const command = ["-m",checker,...(checker === "pyright" ? ["--pythonpath",python] : [])];
  run(python,[...command,"remote.py"]);
  writeFileSync("invalid.py",'from mcptype import mcp\nasync def run() -> None:\n    async with mcp("deepwiki") as wiki:\n        await wiki.tools.read_wiki_structure({"repoName":42})\n');
  const invalid = spawnSync(python,[...command,"invalid.py"],{encoding:"utf8"});
  assert.notEqual(invalid.status,0);
  assert.match(invalid.stdout,/int.*str|str.*int/);
}
run(python,["remote.py"]);
console.log("Remote packages passed: TypeScript + Python, DeepWiki + Microsoft Learn, valid + invalid static types, no Rust or mcptype CLI on PATH.");
