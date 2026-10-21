import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { canonical, changes, generate } from "mcptype/compiler";

const root = fileURLToPath(new URL("../..", import.meta.url));
const vectors = JSON.parse(readFileSync(`${root}/tests/vectors/contract-core.json`, "utf8"));
test("Rust and Node canonical outputs match the shared vectors", async () => {
  for (const vector of vectors.canonical) {
    assert.deepStrictEqual(await canonical(vector.input), vector.output);
  }
});

test("Rust and Node change outputs match the shared vectors", async () => {
  for (const vector of vectors.changes) {
    assert.deepStrictEqual(await changes(vector.input), vector.output);
  }
});

test("Node generate encodes runtime files and returns package bytes", async () => {
  const { files } = await generate({
    target: "python",
    identity: { name: "checkout", version: "1" },
    contracts: { orders: { tools: [{ name: "get", inputSchema: { type: "object" } }] } },
    runtime: { "client.py": "pass\n", "py.typed": new Uint8Array() },
    package_fields: { holon: "marker" },
  });
  assert.deepStrictEqual(Object.keys(files).sort(), ["_apps.py", "client.py", "package.json", "py.typed"]);
  assert.equal(files["client.py"].toString(), "pass\n");
  assert.equal(files["package.json"].toString(), '{"holon": "marker"}');
  await assert.rejects(generate({ target: "go", identity: { name: "a", version: "1" } }), /go_module is required/);
});
