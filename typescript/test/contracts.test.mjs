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


test("Node generate returns only slot sources with external runtime and protocol", async () => {
  const { files } = await generate({
    target: "browser",
    identity: { name: "checkout", version: "1" },
    browser_runtime_specifier: "../browser-runtime.mjs",
    protocol_specifier: "../protocol.js",
    banner: "holon-generated-apps-v1 app=checkout",
  });
  assert.deepStrictEqual(Object.keys(files).sort(), ["browser-resources.d.ts", "browser.d.ts", "browser.js"]);
  assert.ok(files["browser.js"].toString().startsWith("// holon-generated-apps-v1 app=checkout\n"));
  assert.match(files["browser.js"].toString(), /from "\.\.\/browser-runtime\.mjs"/);
  assert.match(files["browser.d.ts"].toString(), /from "\.\.\/protocol\.js"/);
});
