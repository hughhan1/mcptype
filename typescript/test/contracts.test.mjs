import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { canonical, changes } from "mcptype/compiler";

const root = fileURLToPath(new URL("../..", import.meta.url));
const vectors = JSON.parse(readFileSync(`${root}/tests/vectors/contract-core.json`, "utf8"));
const stable = value => Array.isArray(value)
  ? `[${value.map(stable).join(",")}]`
  : value && typeof value === "object"
    ? `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${stable(value[key])}`).join(",")}}`
    : JSON.stringify(value);

test("Rust and Node canonical outputs match the shared vectors", async () => {
  for (const vector of vectors.canonical) {
    assert.equal(stable(await canonical(vector.input)), stable(vector.output));
  }
});

test("Rust and Node change outputs match the shared vectors", async () => {
  for (const vector of vectors.changes) {
    assert.equal(stable(await changes(vector.input)), stable(vector.output));
  }
});
