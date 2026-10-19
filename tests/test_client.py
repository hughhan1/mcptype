import asyncio
import json
import os
import subprocess
import sys
from pathlib import Path

import jsonschema
import pytest
from mcptype import mcp
from mcptype.client import ToolCallError

ROOT = Path(__file__).parents[1]
BINARY = Path(os.environ.get("MCPTYPE_BINARY", ROOT / "target/release/mcptype"))
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


@pytest.mark.parametrize("transport", ["stdio", "http"])
def test_clients_validate_saved_contracts_and_preserve_raw_results(
    tmp_path, monkeypatch, transport
):
    http = None
    try:
        if transport == "http":
            http = subprocess.Popen(
                ["node", str(SERVER), "--http"], stdout=subprocess.PIPE, text=True
            )
            cli(tmp_path, "add", "files", "http", http.stdout.readline().strip())
        else:
            cli(tmp_path, "add", "files", "stdio", "node", str(SERVER))
        monkeypatch.chdir(tmp_path)

        async def run():
            async with mcp("files") as files:
                assert await files.tools.read_text_file({"path": "hello"}) == {
                    "content": "hello"
                }
                raw = await files.call_tool("read_text_file", {"path": "hello"})
                assert raw.content[0].text == "hello"
                with pytest.raises(jsonschema.ValidationError):
                    await files.tools.read_text_file({"path": 1})
                with pytest.raises(ToolCallError) as error:
                    await files.tools.fail({})
                assert error.value.result.is_error
                asked = await files.call_tool("ask", {})
                if transport == "http":
                    assert asked.result_type == "input_required"
                else:
                    assert asked.is_error

        asyncio.run(run())
        if transport == "stdio":
            lock = json.loads(Path("mcptype.lock.json").read_text())
            lock["bindings"]["files"]["endpoint"]["args"].append("--number")
            Path("mcptype.lock.json").write_text(json.dumps(lock))

            async def drift():
                async with mcp("files") as files:
                    with pytest.raises(jsonschema.ValidationError):
                        await files.tools.read_text_file({"path": "hello"})

            asyncio.run(drift())
    finally:
        if http is not None:
            http.terminate()
            http.wait(timeout=10)


def test_both_python_checkers_use_saved_types_with_an_ordinary_import(tmp_path):
    cli(tmp_path, "add", "files", "stdio", "node", str(SERVER))
    (tmp_path / "pyproject.toml").write_text("""[tool.mypy]
strict = true
mypy_path = "$MYPY_CONFIG_FILE_DIR/typings"
[tool.pyright]
typeCheckingMode = "strict"
stubPath = "typings"
""")
    cases = {
        "valid": (
            True,
            """async with mcp("files") as files:
        result = await files.tools.read_text_file({"path": "README.md"})
        print(result["content"].upper())""",
        ),
        "input": (
            False,
            """async with mcp("files") as files:
        await files.tools.read_text_file({"path": 1})""",
        ),
        "output": (
            False,
            """async with mcp("files") as files:
        result = await files.tools.read_text_file({"path": "README.md"})
        print(result["content"] + 1)""",
        ),
        "unknown": (
            False,
            """async with mcp("missing"):
        pass""",
        ),
        "missing_schema": (
            False,
            """async with mcp("files") as files:
        result = await files.tools.fail({})
        print(result["content"])""",
        ),
    }
    for name, (success, body) in cases.items():
        path = tmp_path / f"{name}.py"
        path.write_text(
            "from mcptype import mcp\n\nasync def run() -> None:\n    " + body + "\n"
        )
        for checker in ["mypy", "pyright"]:
            command = [sys.executable, "-m", checker]
            if checker == "pyright":
                command += ["--pythonpath", sys.executable]
            result = subprocess.run(
                [*command, path.name],
                check=False,
                cwd=tmp_path,
                capture_output=True,
                text=True,
                timeout=30,
            )
            assert (result.returncode == 0) == success, result.stdout + result.stderr


def test_native_compiler_without_cli(tmp_path, monkeypatch):
    from mcptype.compiler import compile

    monkeypatch.setenv("PATH", "")
    with pytest.raises(ValueError, match="inputSchema must have type object"):
        compile({"language": "python", "contracts": {"files": {"tools": [{"name": "read", "inputSchema": {"type": "string"}}]}}})
    result = compile({"language": "python", "contracts": {"files": {"tools": [{"name": "read", "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}]}}})
    source = tmp_path / "contract.py"
    source.write_text("from __future__ import annotations\nfrom mcp.types import InputResponses as _InputResponses, RequestParamsMeta as _RequestParamsMeta\nfrom mcptype.client import ToolResult as _ToolResult\n" + result["declarations"] + '\nvalid: Inputs.files.read = {"path": "README.md"}\ninvalid: Inputs.files.read = {"path": 1}  # type: ignore[typeddict-item]\n')
    checked = subprocess.run([sys.executable, "-m", "mypy", "--strict", str(source)], capture_output=True, text=True)
    assert checked.returncode == 0, checked.stdout + checked.stderr
