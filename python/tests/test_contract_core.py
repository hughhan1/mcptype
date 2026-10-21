import json
from pathlib import Path

from mcptype.compiler import canonical, changes


ROOT = Path(__file__).parents[2]
VECTORS = json.loads((ROOT / "tests/vectors/contract-core.json").read_text())


def test_canonical_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["canonical"]:
        assert json.dumps(canonical(vector["input"]), sort_keys=True, separators=(",", ":")) == json.dumps(
            vector["output"], sort_keys=True, separators=(",", ":")
        )


def test_change_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["changes"]:
        assert json.dumps(changes(vector["input"]), sort_keys=True, separators=(",", ":")) == json.dumps(
            vector["output"], sort_keys=True, separators=(",", ":")
        )
