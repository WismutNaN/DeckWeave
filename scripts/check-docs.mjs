// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import GithubSlugger from "github-slugger";

export function scanMarkdown(text) {
  const slugger = new GithubSlugger();
  const anchors = new Set();
  const json = [];
  const prose = [];
  let fence = null;
  let content = [];
  for (const line of text.split(/\r?\n/)) {
    const marker = line.match(/^\s*(`{3,}|~{3,})(.*)$/);
    if (marker) {
      if (!fence) {
        fence = {
          char: marker[1][0],
          size: marker[1].length,
          lang: marker[2].trim(),
        };
        content = [];
      } else if (
        marker[1][0] === fence.char &&
        marker[1].length >= fence.size &&
        !marker[2].trim()
      ) {
        if (fence.lang === "json") json.push(content.join("\n"));
        fence = null;
      }
      continue;
    }
    if (fence) {
      content.push(line);
      continue;
    }
    prose.push(line);
    for (const match of line.matchAll(/<a\s+id="([^"]+)"/g))
      anchors.add(match[1]);
    const heading = line.match(/^#{1,6}\s+(.+?)\s*#*$/);
    if (heading)
      anchors.add(
        slugger.slug(heading[1].replace(/<[^>]*>/g, "").replaceAll("`", "")),
      );
  }
  return {
    anchors,
    json,
    prose: prose.join("\n"),
    unclosedFence: Boolean(fence),
  };
}

function walk(dir) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((item) =>
      item.isDirectory()
        ? walk(path.join(dir, item.name))
        : [path.join(dir, item.name)],
    );
}

export function checkDocs(root) {
  const docs = [
    path.join(root, "README.md"),
    path.join(root, "THIRD_PARTY.md"),
    ...walk(path.join(root, "docs")),
  ].filter((name) => name.endsWith(".md"));
  const parsed = new Map(
    docs.map((name) => [
      path.resolve(name),
      scanMarkdown(fs.readFileSync(name, "utf8")),
    ]),
  );
  const issues = [];
  const requirements = fs.readFileSync(
    path.join(root, "docs/product/requirements.md"),
    "utf8",
  );
  const ids = [...requirements.matchAll(/^\| (DW-[A-Z]+-\d{3}) \|/gm)].map(
    (m) => m[1],
  );
  const known = new Set(ids);
  if (known.size !== ids.length) issues.push("Повторяющиеся requirement IDs");
  for (const [file, doc] of parsed) {
    const label = path.relative(root, file);
    const archived = label.split(path.sep).includes("archive");
    if (doc.unclosedFence) issues.push(label + ": незакрытый code fence");
    for (const example of doc.json) {
      try {
        JSON.parse(example);
      } catch (error) {
        issues.push(label + ": JSON: " + error.message);
      }
    }
    for (const match of doc.prose.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
      const target = match[1].replace(/^<|>$/g, "");
      if (/^[a-z][a-z\d+.-]*:/i.test(target)) continue;
      const [relative, fragment] = target.split("#");
      const destination = path.resolve(
        path.dirname(file),
        decodeURIComponent(relative || path.basename(file)),
      );
      if (!fs.existsSync(destination)) {
        issues.push(label + ": отсутствует " + target);
        continue;
      }
      if (
        fragment &&
        parsed.has(destination) &&
        !parsed.get(destination).anchors.has(decodeURIComponent(fragment))
      ) {
        issues.push(label + ": отсутствует anchor " + target);
      }
    }
    if (!archived) {
      for (const match of doc.prose.matchAll(/DW-[A-Z]+-\d{3}/g)) {
        if (!known.has(match[0]))
          issues.push(label + ": неизвестный ID " + match[0]);
      }
    }
  }
  const planFile = path.join(root, "docs/implementation/roadmap.md");
  const plan = fs.readFileSync(planFile, "utf8");
  let tasks = false;
  for (const line of plan.split(/\r?\n/)) {
    if (/^### /.test(line)) tasks = line === "### Задачи";
    if (/^## /.test(line)) tasks = false;
    if (tasks && /^- \[[ x~]\]/.test(line) && !/\]\([^)]+\)/.test(line)) {
      issues.push("roadmap: задача без ссылки: " + line);
    }
  }
  for (const module of fs.readdirSync(
    path.join(root, "docs/architecture/modules"),
  )) {
    if (module === "README.md" || !module.endsWith(".md")) continue;
    if (!plan.includes("modules/" + module))
      issues.push("roadmap: отсутствует модуль " + module);
  }
  return { issues, files: docs.length, requirements: known.size };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const result = checkDocs(process.cwd());
  if (result.issues.length) {
    console.error(result.issues.join("\n"));
    process.exitCode = 1;
  } else
    console.log(
      `Документация: ${result.files} файлов, ${result.requirements} требований; ссылки, anchors и JSON корректны.`,
    );
}
