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
  const runtime = Object.fromEntries(Object.entries(request.runtime ?? {}).map(([path, contents]) => [
    path,
    Buffer.from(contents).toString("base64"),
  ]));
  const result = JSON.parse(native.generate(JSON.stringify({ ...request, runtime })));
  return { files: Object.fromEntries(Object.entries(result.files).map(([path, contents]) => [
    path,
    Buffer.from(contents, "base64").toString("utf8"),
  ])) };
}
