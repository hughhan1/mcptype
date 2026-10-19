from __future__ import annotations

import json
from collections.abc import AsyncIterator, Mapping
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any, Generic, Literal, Protocol, TypeAlias, TypeVar

import httpx2
import jsonschema
from mcp import Client
from mcp.client.stdio import StdioServerParameters, stdio_client
from mcp.client.streamable_http import streamable_http_client
from mcp.types import (
    CallToolRequest,
    CallToolRequestParams,
    CallToolResult,
    ContentBlock,
    Implementation,
    InputRequiredResult,
    InputResponses,
    RequestParamsMeta,
)
from pydantic import TypeAdapter
from referencing import Registry

Output = TypeVar("Output")


class CompleteResult(Protocol, Generic[Output]):
    result_type: Literal["complete"]
    is_error: Literal[False]
    structured_content: Output
    content: list[ContentBlock]
    meta: dict[str, object] | None


class ErrorResult(Protocol):
    result_type: Literal["complete"]
    is_error: Literal[True]
    structured_content: object
    content: list[ContentBlock]
    meta: dict[str, object] | None


ToolResult: TypeAlias = CompleteResult[Output] | ErrorResult | InputRequiredResult
_result = TypeAdapter(CallToolResult | InputRequiredResult)


class ToolCallError(RuntimeError):
    def __init__(
        self, app: str, tool: str, result: CallToolResult | InputRequiredResult
    ) -> None:
        self.app, self.tool, self.result = app, tool, result
        self.code = (
            "input_required"
            if isinstance(result, InputRequiredResult)
            else "tool_error"
        )
        super().__init__(f"{app}.{tool}: {self.code}")


def _validate(instance: object, schema: dict[str, Any]) -> None:
    jsonschema.validate(
        instance, schema, registry=Registry(), format_checker=jsonschema.FormatChecker()
    )


class Connection:
    def __init__(self, name: str, client: Client, contract: dict[str, Any]) -> None:
        self.name = name
        self.client = client
        self.definitions = {tool["name"]: tool for tool in contract["tools"]}
        self.tools = Tools(self)

    async def call_tool(
        self,
        name: str,
        arguments: Mapping[str, object],
        *,
        timeout: float | None = None,
        input_responses: InputResponses | None = None,
        request_state: str | None = None,
        meta: RequestParamsMeta | None = None,
    ) -> CallToolResult | InputRequiredResult:
        if name not in self.definitions:
            raise ValueError(f"Unknown tool {self.name}.{name}")
        definition = self.definitions[name]
        _validate(arguments, definition["inputSchema"])
        result = await self.client.session.send_request(
            CallToolRequest(
                params=CallToolRequestParams(
                    name=name,
                    arguments=dict(arguments),
                    input_responses=input_responses,
                    request_state=request_state,
                    _meta=meta,
                )
            ),
            _result,
            request_read_timeout_seconds=timeout,
        )
        if (
            isinstance(result, CallToolResult)
            and not result.is_error
            and "outputSchema" in definition
        ):
            _validate(result.structured_content, definition["outputSchema"])
        return result


class Tools:
    def __init__(self, connection: Connection) -> None:
        self._connection = connection

    def __getattr__(self, name: str) -> Any:
        if name.startswith("_") or name not in self._connection.definitions:
            raise AttributeError(name)

        async def call(
            arguments: Mapping[str, object], *, timeout: float | None = None
        ) -> object:
            result = await self._connection.call_tool(name, arguments, timeout=timeout)
            if isinstance(result, InputRequiredResult) or result.is_error:
                raise ToolCallError(self._connection.name, name, result)
            return result.structured_content

        return call


def binding(name: str) -> tuple[Path, dict[str, Any]]:
    cwd = Path.cwd()
    for root in [cwd, *cwd.parents]:
        path = root / "mcptype.lock.json"
        if path.is_file():
            lock = json.loads(path.read_text())
            if lock["version"] != 1:
                raise ValueError("unsupported mcptype.lock.json version")
            if name not in lock["bindings"]:
                raise ValueError(f"Unknown binding {name}; run mcptype add first.")
            return root, lock["bindings"][name]
    raise FileNotFoundError("mcptype.lock.json; run mcptype add first")


@asynccontextmanager
async def mcp(
    name: str, *, url: str | None = None, headers: dict[str, str] | None = None
) -> AsyncIterator[Connection]:
    root, saved = binding(name)
    endpoint: dict[str, Any] = {"type": "http", "url": url} if url else saved["endpoint"]
    async with httpx2.AsyncClient(headers=headers, trust_env=False) as http:
        transport = (
            streamable_http_client(endpoint["url"], http_client=http)
            if endpoint["type"] == "http"
            else stdio_client(
                StdioServerParameters(
                    command=endpoint["command"], args=endpoint["args"], cwd=root
                )
            )
        )
        async with Client(
            transport,
            client_info=Implementation(name="mcptype", version="0.0.1"),
            mode="auto",
        ) as client:
            yield Connection(name, client, saved["contract"])
