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
