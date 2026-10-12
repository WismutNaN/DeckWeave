// SPDX-License-Identifier: GPL-3.0-or-later
import { readFileSync } from "node:fs";
import ts from "typescript";
import { compileModule } from "svelte/compiler";
import type { EditorSession as Session } from "../src/editor/session.svelte";
const source = new URL("../src/editor/session.svelte.ts", import.meta.url);
const js = ts.transpileModule(readFileSync(source, "utf8"), {
  compilerOptions: {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.ESNext,
  },
}).outputText;
const compiled = compileModule(js, {
  filename: source.pathname,
  generate: "server",
}).js.code.replace(
  /from (["'])([^"']+)\1/g,
  (_, quote, specifier) =>
    `from ${quote}${specifier.startsWith(".") ? new URL(specifier + ".ts", source).href : import.meta.resolve(specifier)}${quote}`,
);
export const EditorSession = (
  await import(
    "data:text/javascript;base64," + Buffer.from(compiled).toString("base64")
  )
).EditorSession as typeof Session;
