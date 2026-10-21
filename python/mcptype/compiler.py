from __future__ import annotations

import json
import base64
from typing import Any, cast

from . import _native


def compile(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.compile(json.dumps(request))))


def canonical(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.canonical(json.dumps(request))))


def changes(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.changes(json.dumps(request))))


def generate(request: dict[str, Any]) -> dict[str, Any]:
    encoded = dict(request)
    encoded["runtime"] = {
        path: base64.b64encode(contents.encode() if isinstance(contents, str) else contents).decode()
        for path, contents in request.get("runtime", {}).items()
    }
    result = cast(dict[str, Any], json.loads(_native.generate(json.dumps(encoded))))
    result["files"] = {path: base64.b64decode(contents).decode() for path, contents in result["files"].items()}
    return result
