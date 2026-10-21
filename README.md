# mcptype

`mcptype` is the language-independent core for typed MCP contracts. It validates and serializes the v1 contract document, canonicalizes JSON Schemas, reports compatibility changes, and compiles tool schemas to TypeScript, Python, or Go declarations. It has no transport client and does not discover servers.

## Install

Build the crate and native bindings from this repository. The C ABI is packaged as a static library by `ffi/package-sdk.py` for hosts such as Go.

## CLI

```sh
mcptype compile < request.json
mcptype diff expected.json published.json
mcptype generate request.json output.json
mcptype check check.json [--verify-only]
mcptype schema contract
```

`compile` reads a JSON compile request from stdin and writes declarations as JSON. `generate` reads a package request and runtime file paths from JSON, then writes the generated package as JSON. `check` verifies source fingerprints and runs `tsc` or `mypy`; `--verify-only` skips the type checker. `schema contract` prints the JSON Schema for the v1 metadata document. `diff` prints one breaking-change message per line and exits with status 1 when the caller's expectation is no longer satisfied.

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

## Node and Python bindings

The Node binding exports `compile`, `canonical`, `changes`, and `generate` from `mcptype/compiler`:

```js
import { canonical, changes, compile, generate } from "mcptype/compiler";

const generated = await compile({ language: "typescript", contracts });
const fingerprint = await canonical({ contract });
const report = await changes({ expected, published });
const packageFiles = await generate({ target: "browser", identity, contracts, runtime });
```

The Python binding exposes the same functions from `mcptype.compiler`:

```python
from mcptype.compiler import canonical, changes, compile, generate

result = compile({"language": "python", "contracts": contracts})
fingerprint = canonical({"contract": contract})
report = changes({"expected": expected, "published": published})
package_files = generate({"target": "python", "identity": identity, "contracts": contracts, "runtime": runtime})
```

The bindings accept JSON-compatible objects and return JSON-compatible objects. `generate` accepts runtime files as byte strings at the language boundary and returns generated files as byte strings. The bindings do not connect to MCP servers or provide runtime clients.

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
| `src/` | Contract model, schema type tree, canonicalization, comparison, package generation, and declaration emitters |
| `ffi/` | Static C library and packaging script |
| `typescript/` | Node-API binding and JavaScript wrapper |
| `python/` | PyO3 binding and Python wrapper |
| `schema/` | Published contract JSON Schema |
| `tests/vectors/` | Canonicalization and compatibility vectors shared across bindings |

`schema/contract.schema.json` documents the wire format. Rust, Node, and Python run the same vectors, including the digests.
