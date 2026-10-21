import json
from pathlib import Path

import pytest

from mcptype.compiler import canonical, changes, generate


ROOT = Path(__file__).parents[2]
VECTORS = json.loads((ROOT / "tests/vectors/contract-core.json").read_text())


def test_canonical_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["canonical"]:
        assert canonical(vector["input"]) == vector["output"]


def test_change_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["changes"]:
        assert changes(vector["input"]) == vector["output"]


def test_generate_encodes_runtime_files_and_returns_bytes() -> None:
    files = generate(
        {
            "target": "go",
            "identity": {"name": "checkout", "version": "1"},
            "contracts": {"orders": {"tools": [{"name": "get", "inputSchema": {"type": "object"}}]}},
            "runtime": {"README": "runtime\n", "blob": b"\x00"},
            "go_module": "example.com/runtime",
        }
    )["files"]
    assert sorted(files) == ["README", "blob", "clients.go", "package.json", "types.go"]
    assert files["blob"] == b"\x00"
    assert b'holon "example.com/runtime"' in files["clients.go"]
    with pytest.raises(ValueError, match="must be relative"):
        generate({"target": "node", "identity": {"name": "a", "version": "1"}, "runtime": {"/x": ""}})
