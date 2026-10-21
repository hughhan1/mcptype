import json
from pathlib import Path

from mcptype.compiler import canonical, changes, generate

ROOT = Path(__file__).parents[2]
VECTORS = json.loads((ROOT / "tests/vectors/contract-core.json").read_text())


def test_canonical_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["canonical"]:
        assert canonical(vector["input"]) == vector["output"]


def test_change_outputs_match_shared_vectors() -> None:
    for vector in VECTORS["changes"]:
        assert changes(vector["input"]) == vector["output"]


def test_python_binding_generates_runtime_files() -> None:
    result = generate(
        {
            "target": "python",
            "identity": {"name": "orders", "version": "1"},
            "package_fields": {"holon": "holon-generated-apps-v1"},
            "runtime": {
                "__init__.py": "# package\n",
                "binding.py": "# binding\n",
                "client.py": "# client\n",
                "context.py": "# context\n",
                "identity.py": "# identity\n",
                "context.schema.json": "{}\n",
                "identity.schema.json": "{}\n",
                "py.typed": "",
            },
        }
    )
    assert "create_apps" in result["files"]["_apps.py"]
    assert result["files"]["client.py"] == "# client\n"
