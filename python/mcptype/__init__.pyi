from contextlib import AbstractAsyncContextManager
from typing import Never

from .client import ToolCallError as ToolCallError

def mcp(name: Never) -> AbstractAsyncContextManager[object]: ...
