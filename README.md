# mcptype

`mcptype` is the language-independent core for typed MCP contracts. It validates and serializes the v1 contract document, canonicalizes JSON Schemas, reports compatibility changes, compiles tool schemas to TypeScript, Python, or Go declarations, and lays those declarations out as typed client packages. It has no transport client and does not discover servers.

## Install

Build the crate and native bindings from this repository. The C ABI is packaged as a static library by `ffi/package-sdk.py` for hosts such as Go.

## CLI

```sh
mcptype compile < request.json
mcptype diff expected.json published.json
mcptype generate request.json out/
```

`compile` reads a JSON compile request from stdin and writes declarations as JSON. `generate` writes one target's package into a directory; in its request file, `runtime` values are paths of files to copy in. `diff` prints one breaking-change message per line and exits with status 1 when the caller's expectation is no longer satisfied.

## Rust API

```rust
use mcptype::{Language, Request, compile};
use serde_json::json;
use std::collections::BTreeMap;

let generated = compile(Request {
    language: Language::TypeScript,
    contracts: BTreeMap::from([(
        "orders".into(),
        json!({"tools": [{
            "name": "get_order",
            "inputSchema": {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}
        }]}),
    )]),
})?;
println!("{}", generated.declarations);
```

`Contract` stores tools by name and resources by URI and keeps the v1 wire format (arrays), written in sorted order. Deserializing a contract or metadata document validates it; there is no separate validation call. `canonical` removes schema annotations and sorts order-insensitive JSON Schema keywords; the binding entry point accepts `{"contract"}`, `{"schema"}` or `{"metadata"}`, so a published metadata document can be validated and digested in one call (`canonical` and `digest` are `null` when it has no contract). `digest` returns the SHA-256 of the canonical contract encoded as compact JSON with sorted keys. `changes` returns typed change records; an empty list means the expectation is compatible with published metadata.

## Generated packages

`generate` turns a contract set into the files of one target's client package. The caller supplies the runtime the package imports, so mcptype never ships a client of its own:

```json
{
  "target": "node",
  "identity": {"name": "checkout", "version": "1.0.0"},
  "requires": {"orders": {"name": "orders", "version": "1.0.0"}},
  "contracts": {"orders": {"tools": [{"name": "get_order", "inputSchema": {"type": "object"}}]}},
  "runtime": {"runtime.mjs": "<base64>"},
  "protocol": "<declarations spliced into each .d.ts entry point>",
  "package_fields": {"private-marker": "value"},
  "go_module": "example.com/runtime"
}
```

| Target | Files besides `runtime` |
| --- | --- |
| `node` | `index.js`, `index.d.ts`, `resources.d.ts`, `package.json` |
| `browser` | `browser.js`, `browser.d.ts`, `browser-resources.d.ts`, `package.json` (the same `package.json` as `node`, so both merge into one package) |
| `python` | `_apps.py`, `package.json` |
| `go` | `types.go`, `clients.go` (gofmt-clean; imports `go_module` as `holon`), `package.json` |

`requires` lists every dependency the caller declares; `contracts` holds the typed subset. Generated `createApps(bindings, options)` accepts `options.validator`, a function from a JSON Schema to a predicate, for hosts that precompile validators. Over JSON, `runtime` and the returned `files` values are base64; the Node and Python wrappers accept strings or bytes and return bytes.

## Node and Python bindings

The Node binding exports `compile`, `canonical`, `changes`, and `generate` from `mcptype/compiler`:

```js
import { canonical, changes, compile } from "mcptype/compiler";

const generated = await compile({ language: "typescript", contracts });
const fingerprint = await canonical({ contract });
const report = await changes({ expected, published });
const { files } = await generate({ target: "browser", identity, contracts, runtime });
```

The Python binding exposes the same functions from `mcptype.compiler`:

```python
from mcptype.compiler import canonical, changes, compile, generate

result = compile({"language": "python", "contracts": contracts})
fingerprint = canonical({"contract": contract})
report = changes({"expected": expected, "published": published})
files = generate({"target": "python", "identity": identity, "contracts": contracts})["files"]
```

The bindings accept JSON-compatible objects and return JSON-compatible objects. They do not connect to MCP servers, write files, or provide runtime clients; `generate` returns file contents for the caller to write.

## Contract wire format

A metadata document has `version: 1`, an `identity`, an optional app `contract`, and optional dependency expectations in `requires`. A contract contains tools and resources:

```json
{
  "version": 1,
  "identity": {"name": "example/orders", "version": "1.0.0"},
  "contract": {
    "tools": [{
      "name": "get_order",
      "inputSchema": {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}
    }],
    "resources": [{
      "uri": "orders://summary",
      "outputSchema": {"type": "object", "properties": {"total": {"type": "number"}}}
    }]
  }
}
```

Descriptions, titles, `$comment`, and examples do not affect compatibility. Additive tools and resources do not break callers. Changes to an expected tool's input or output, or to an expected resource's output, are reported in deterministic name and URI order. The exact display text is stable for CLI and host diagnostics.

## Go declarations

Compiling with `language: "go"` returns Go declarations and a `bindings` map. The map includes each binding's generated client name and each tool's generated method, input, and output names. This lets hosts build transport-specific clients without reimplementing name allocation.

Closed objects produce structs, arrays produce slices, and homogeneous maps retain their value type. JSON numbers use `json.Number` to preserve precision. Optional fields retain presence; nullable and unsupported shapes use `json.RawMessage`. Runtime schema validation remains the caller's responsibility.

## Layout

| Path | Contents |
| --- | --- |
| `src/` | Contract model, schema type tree, canonicalization, comparison, and declaration emitters |
| `src/generate/` | Package layouts per target |
| `ffi/` | Static C library and packaging script |
| `typescript/` | Node-API binding and JavaScript wrapper |
| `python/` | PyO3 binding and Python wrapper |
| `schema/` | Published contract JSON Schema |
| `tests/vectors/` | Canonicalization and compatibility vectors shared across bindings |
| `tests/generate.rs` | Generated packages checked with `tsc`, `mypy --strict`, `go vet`, and `gofmt` |

`schema/contract.schema.json` documents the wire format. Rust, Node, and Python run the same vectors, including the digests.
