from __future__ import annotations

import json
from typing import Any, cast

from . import _native


def compile(request: dict[str, Any]) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_native.compile(json.dumps(request))))
