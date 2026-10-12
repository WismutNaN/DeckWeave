// SPDX-License-Identifier: GPL-3.0-or-later
// Mutation workflow; EditorSession owns reactive selection and draft state.
import { copy } from "./api";
import type { EditorSession } from "./session.svelte";
import type { Draft, Plan, Operation } from "./types";
export async function saveDraft(s: EditorSession) {
  if (s.pending && s.pending.command.type !== "patch_draft")
    throw new Error(
      s.t(
        "Сначала завершите запрос применения.",
        "Resolve the pending apply request first.",
      ),
    );
  if (!s.draft || !s.document || (!s.dirty && !s.pending)) return;
  if (s.conflict && !s.pending)
    throw new Error(
      s.t(
        "Сначала объедините изменения с новой версией.",
        "Merge the current draft revision before saving.",
      ),
    );
  const locks = [...s.draft.packageLock];
  const required = new Set(
    s.document.graph.nodes.flatMap((n) =>
      n.kind.endpoint ? [n.kind.endpoint.packageId] : [],
    ),
  );
  for (const packageId of required) {
    if (locks.some((p) => p.packageId === packageId)) continue;
    const selected = s.availableCatalogs.filter(
      (c) => c.package.packageId === packageId,
    );
    if (selected.length !== 1)
      throw new Error(
        s.t(
          "Нужна однозначная версия плагина.",
          "Select an unambiguous plugin version.",
        ),
      );
    locks.push({ ...selected[0].package, source: null });
  }
  s.pending ??= s.envelope({
    type: "patch_draft",
    draft_id: s.draft.draftId,
    expected_version: s.draft.version,
    edit: {
      type: "replace_configuration",
      document: copy(s.document),
      package_lock: locks,
    },
  });
  const result = await s.sendPending<{ draft: Draft }>();
  s.draft = result.draft;
  s.document = copy(result.draft.document);
  s.pending = null;
  s.attempted = false;
  s.conflict = false;
  s.persist();
  s.notice = s.t("Черновик сохранён", "Draft saved");
}
export async function applyDraft(s: EditorSession) {
  await s.run(async () => {
    if (s.pending?.command.type === "apply_draft") {
      await s.finishApply();
      return;
    }
    if (s.pending) {
      await s.save();
      return;
    }
    await s.save();
    if (!s.draft) return;
    s.plan = await s.client.query<Plan>({
      type: "plan_apply",
      draft_id: s.draft.draftId,
      expected_draft_version: s.draft.version,
      expected_active: s.snapshot?.active ?? null,
      allow_disruption: false,
    });
    s.pending = s.envelope({
      type: "apply_draft",
      draft_id: s.draft.draftId,
      expected_draft_version: s.draft.version,
      expected_active: s.snapshot?.active ?? null,
      allow_disruption: false,
      plan_hash: s.plan.planHash,
    });
    s.attempted = false;
    await s.finishApply();
  });
}
export async function finishApply(s: EditorSession) {
  if (!s.pending) return;
  const receipt = await s.sendPending<{ operationId: string }>();
  s.operationId = receipt.operationId;
  s.persist();
  s.operation = await s.client.query<Operation>({
    type: "operation",
    operation_id: receipt.operationId,
  });
  await s.refresh();
  s.pending = null;
  s.attempted = false;
  s.persist();
  s.notice = s.preview
    ? s.t(
        "Пример сохранён. Оборудование не используется.",
        "Example saved. No hardware is used.",
      )
    : s.operation.state === "succeeded" &&
        s.operation.runtimeHealth === "applied"
      ? s.t("Действия применены", "Actions applied")
      : s.t(
          `Состояние применения: ${s.operation.state}`,
          `Apply status: ${s.operation.state}`,
        );
}
