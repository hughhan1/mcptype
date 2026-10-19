import asyncio
import json
import subprocess
import sys
from pathlib import Path

import jsonschema
import pytest
from mcptype import mcp
from mcptype.client import ToolCallError
from mcptype.compiler import compile

ROOT = Path(__file__).parents[2]
BINARY = ROOT / "target/release/mcptype"
SERVER = ROOT / "tests/server.mjs"


def cli(root, *args):
    subprocess.run(
        [str(BINARY), *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )


@pytest.fixture
def http_server():
    server = subprocess.Popen(
        ["node", str(SERVER), "--http"], stdout=subprocess.PIPE, text=True
    )
    yield server.stdout.readline().strip()
    server.terminate()
    server.wait(timeout=10)


def test_clients_validate_saved_contracts_and_preserve_raw_results(
    tmp_path, monkeypatch, http_server
):
    cli(tmp_path, "add", "files", "stdio", "node", str(SERVER))
    cli(tmp_path, "add", "remote", "http", http_server)
    monkeypatch.chdir(tmp_path)

    async def run():
        for name in ["files", "remote"]:
            async with mcp(name) as client:
                assert await client.tools.read_text_file({"path": "hello"}) == {
                    "content": "hello"
                }
                raw = await client.call_tool("read_text_file", {"path": "hello"})
                assert raw.content[0].text == "hello"
                with pytest.raises(jsonschema.ValidationError):
                    await client.tools.read_text_file({"path": 1})
                with pytest.raises(ToolCallError) as error:
                    await client.tools.fail({})
                assert error.value.result.is_error
                if name == "remote":
                    asked = await client.call_tool("ask", {})
                    assert asked.result_type == "input_required"
        lock = json.loads(Path("mcptype.lock.json").read_text())
        lock["bindings"]["files"]["endpoint"]["args"].append("--number")
        Path("mcptype.lock.json").write_text(json.dumps(lock))
        async with mcp("files") as drifted:
            with pytest.raises(jsonschema.ValidationError):
                await drifted.tools.read_text_file({"path": "hello"})

    asyncio.run(run())


def test_generated_stubs_from_the_cli_or_native_compiler_type_check(tmp_path):
    cli(tmp_path, "add", "files", "stdio", "node", str(SERVER))
    lock = json.loads((tmp_path / "mcptype.lock.json").read_text())
    contracts = {name: b["contract"] for name, b in lock["bindings"].items()}
    declarations = compile({"language": "python", "contracts": contracts})["declarations"]
    assert declarations == (tmp_path / "typings/mcptype/__init__.pyi").read_text()
    (tmp_path / "pyproject.toml").write_text("""[tool.mypy]
strict = true
mypy_path = "$MYPY_CONFIG_FILE_DIR/typings"
[tool.pyright]
typeCheckingMode = "strict"
stubPath = "typings"
reportUnnecessaryTypeIgnoreComment = "error"
""")
    # Each ignored line must be an error: both checkers reject unused ignores.
    (tmp_path / "app.py").write_text("""from mcptype import mcp

async def run() -> None:
    async with mcp("files") as files:
        result = await files.tools.read_text_file({"path": "README.md"})
        print(result["content"].upper())
        print(result["content"] + 1)  # type: ignore
        await files.tools.read_text_file({"path": 1})  # type: ignore
        untyped = await files.tools.fail({})
        print(untyped["content"])  # type: ignore
    async with mcp("missing"):  # type: ignore
        pass
""")
    for checker in [["mypy"], ["pyright", "--pythonpath", sys.executable]]:
        result = subprocess.run(
            [sys.executable, "-m", *checker, "app.py"],
            check=False,
            cwd=tmp_path,
            capture_output=True,
            text=True,
            timeout=60,
        )
        assert result.returncode == 0, result.stdout + result.stderr
