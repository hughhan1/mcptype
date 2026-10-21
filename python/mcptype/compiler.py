from __future__ import annotations

import base64
import json
from typing import Any, cast

from . import _native


def compile(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.compile(json.dumps(request))))


def canonical(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.canonical(json.dumps(request))))


def changes(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.changes(json.dumps(request))))


def generate(request: dict[str, Any]) -> dict[str, dict[str, bytes]]:
    """Runtime values may be ``str`` (written as UTF-8) or ``bytes``; files come back as ``bytes``."""
    runtime = {
        path: base64.b64encode(data.encode() if isinstance(data, str) else data).decode()
        for path, data in request.get("runtime", {}).items()
    }
    result = json.loads(_native.generate(json.dumps({**request, "runtime": runtime})))
    return {"files": {path: base64.b64decode(data) for path, data in result["files"].items()}}
