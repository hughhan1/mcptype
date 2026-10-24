import shutil
import subprocess
import sys
from pathlib import Path

output = Path(sys.argv[1]).resolve()
target = sys.argv[2]
subprocess.run(["cargo", "build", "--locked", "--release", "-p", "mcptype-ffi", "--target", target], check=True)
(output / "include").mkdir(parents=True, exist_ok=True)
(output / "lib/pkgconfig").mkdir(parents=True, exist_ok=True)
shutil.copyfile("ffi/mcptype.h", output / "include/mcptype.h")
shutil.copyfile(f"target/{target}/release/libmcptype.a", output / "lib/libmcptype.a")
libs = "-lSystem -lc -lm" if "apple" in target else "-ldl -lpthread -lm"
(output / "lib/pkgconfig/mcptype.pc").write_text(f"""prefix=${{pcfiledir}}/../..
Name: mcptype
Description: MCP contract compiler
Version: 0.0.6
Libs: -L${{prefix}}/lib -lmcptype {libs}
Cflags: -I${{prefix}}/include
""")
