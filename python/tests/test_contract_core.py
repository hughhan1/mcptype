import json
from pathlib import Path

from mcptype.compiler import canonical, changes


ROOT = Path(__file__).parents[2]
VECTORS = json.loads((ROOT / "tests/vectors/contract-core.json").read_text())


def test_canonical_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["canonical"]:
        assert canonical(vector["input"]) == vector["output"]


def test_change_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["changes"]:
        assert changes(vector["input"]) == vector["output"]
