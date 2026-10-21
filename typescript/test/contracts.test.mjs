import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { canonical, changes } from "mcptype/compiler";

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
