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

test("Node binding generates package files and decodes runtime bytes", async () => {
  const result = await generate({
    target: "node",
    identity: { name: "orders", version: "1" },
    package_fields: { holon: "holon-generated-apps-v1" },
    runtime: {
      "protocol.d.ts": "export interface ToolClient<A,B,C> {}\n",
      "runtime.mjs": "export const runtime = true;\n",
      "THIRD-PARTY-NOTICES.txt": "notice\n",
    },
  });
  assert.match(result.files["index.js"], /createApps/);
  assert.equal(result.files["runtime.mjs"], "export const runtime = true;\n");
  assert.match(result.files["package.json"], /holon-generated-apps-v1/);
});
