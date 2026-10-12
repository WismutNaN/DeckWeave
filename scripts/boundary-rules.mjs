// SPDX-License-Identifier: GPL-3.0-or-later

export function normalizePath(value) {
  return value.replaceAll("\\", "/");
}

export function countLines(content) {
  if (content.length === 0) return 0;
  const normalized = content.replaceAll("\r\n", "\n");
  const lines = normalized.split("\n").length;
  return normalized.endsWith("\n") ? lines - 1 : lines;
}

export function validateLocalDependencies(packages, policies) {
  const errors = [];
  const localNames = new Set(packages.map((item) => item.name));

  for (const packageName of Object.keys(policies)) {
    if (!localNames.has(packageName)) {
      errors.push(
        `${packageName}: architecture policy не соответствует workspace package`,
      );
    }
  }

  for (const pkg of packages) {
    const policy = policies[pkg.name];
    if (!policy) {
      errors.push(
        `${pkg.name}: добавьте context ownership и разрешённые зависимости`,
      );
      continue;
    }
    if (!policy.context?.trim()) {
      errors.push(`${pkg.name}: context ownership не задан`);
    }
    if (!Array.isArray(policy.allowedDependencies)) {
      errors.push(`${pkg.name}: allowedDependencies должен быть массивом`);
      continue;
    }
    for (const dependency of policy.allowedDependencies) {
      if (!localNames.has(dependency)) {
        errors.push(
          `${pkg.name}: policy ссылается на неизвестный package ${dependency}`,
        );
      }
    }
    for (const dependency of pkg.dependencies) {
      if (dependency.kind === "dev" || !localNames.has(dependency.name))
        continue;
      if (!policy.allowedDependencies.includes(dependency.name)) {
        const targetContext =
          policies[dependency.name]?.context ?? "неизвестный context";
        errors.push(
          `${pkg.name} (${policy.context}) -> ${dependency.name} (${targetContext}): ` +
            "нарушена граница зависимости",
        );
      }
    }
  }

  return errors;
}

export function validateSizeRatchet({ files, crateRoots, limits, exceptions }) {
  const errors = [];
  const roots = new Set([...crateRoots].map(normalizePath));
  const normalizedFiles = new Map(
    [...files].map(([file, content]) => [normalizePath(file), content]),
  );

  for (const [file, exception] of Object.entries(exceptions)) {
    if (!normalizedFiles.has(file)) {
      errors.push(
        `${file}: size exception ссылается на отсутствующий production source`,
      );
    }
    if (!Number.isInteger(exception.maxLines) || exception.maxLines < 1) {
      errors.push(`${file}: size exception требует положительный maxLines`);
    }
    for (const field of ["owner", "reason", "reviewMilestone"]) {
      if (!exception[field]?.trim()) {
        errors.push(`${file}: size exception требует ${field}`);
      }
    }
  }

  for (const [file, content] of normalizedFiles) {
    const lineCount = countLines(content);
    const limit = roots.has(file)
      ? limits.crateRootLines
      : limits.productionModuleLines;
    const exception = exceptions[file];

    if (lineCount <= limit) {
      if (exception) {
        errors.push(
          `${file}: ${lineCount} строк уже укладываются в limit ${limit}; удалите size exception`,
        );
      }
      continue;
    }
    if (!exception) {
      errors.push(
        `${file}: ${lineCount} строк превышают limit ${limit}; ` +
          "разделите ответственность или добавьте reviewed ratchet exception",
      );
      continue;
    }
    if (lineCount > exception.maxLines) {
      errors.push(
        `${file}: size ratchet вырос ${exception.maxLines} -> ${lineCount} строк ` +
          `(owner: ${exception.owner}, review: ${exception.reviewMilestone})`,
      );
    }
  }

  return errors;
}
