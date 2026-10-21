import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";

execFileSync("cargo", ["build", "--locked", "--release", "-p", "mcptype-node"], { stdio: "inherit" });
mkdirSync("native", { recursive: true });
const extension = process.platform === "darwin" ? "dylib" : "so";
copyFileSync(`target/release/libmcptype_node.${extension}`, `native/${process.platform}-${process.arch}.node`);
