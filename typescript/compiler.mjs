import { createRequire } from "node:module";

const native = createRequire(import.meta.url)(`../native/${process.platform}-${process.arch}.node`);

export async function compile(request) {
  return JSON.parse(native.compile(JSON.stringify(request)));
}

export async function canonical(request) {
  return JSON.parse(native.canonical(JSON.stringify(request)));
}

export async function changes(request) {
  return JSON.parse(native.changes(JSON.stringify(request)));
}

export async function generate(request) {
  const runtime = Object.fromEntries(Object.entries(request.runtime ?? {}).map(([path, data]) => [path, Buffer.from(data).toString("base64")]));
  const { files } = JSON.parse(native.generate(JSON.stringify({ ...request, runtime })));
  return { files: Object.fromEntries(Object.entries(files).map(([path, data]) => [path, Buffer.from(data, "base64")])) };
}
