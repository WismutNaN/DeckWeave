// SPDX-License-Identifier: GPL-3.0-or-later
// Mutation workflow; EditorSession owns reactive selection and draft state.
import { copy } from "./api";
import { equal } from "./bindings";
import type { EditorSession } from "./session.svelte";
import type {
  Draft,
  Plan,
  Operation,
  ActiveSelection,
  GraphDocument,
} from "./types";
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
  const required = new Set([
    ...s.document.graph.nodes.flatMap((n) =>
      n.kind.endpoint ? [n.kind.endpoint.packageId] : [],
    ),
    ...Object.values(s.document.pluginInstances ?? {}).map((i) => i.packageId),
  ]);
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
  const submitted = copy(
    (s.pending.command.edit as { document: GraphDocument }).document,
  );
  const result = await s.sendPending<{ draft: Draft }>();
  s.draft = result.draft;
  if (equal(s.document, submitted)) s.document = copy(result.draft.document);
  s.pending = null;
  s.attempted = false;
  s.conflict = false;
  await s.persist();
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
    const active = await s.client.query<{
      selection: ActiveSelection | null;
      document: GraphDocument | null;
    }>({ type: "active_document" });
    s.review = {
      draftVersion: s.draft.version,
      expectedActive: active.selection,
      before: active.document,
      document: copy(s.document!),
      requiresDisruption: false,
      consent: false,
    };
    await planReview(s);
  });
}
export async function finishApply(s: EditorSession) {
  if (!s.pending) return;
  const receipt = await s.sendPending<{ operationId: string }>();
  s.operationId = receipt.operationId;
  await s.persist();
  s.operation = await s.client.query<Operation>({
    type: "operation",
    operation_id: receipt.operationId,
  });
  await s.refresh();
  s.pending = null;
  s.attempted = false;
  await s.persist();
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

export async function planReview(s: EditorSession) {
  if (
    !s.review ||
    !s.draft ||
    s.review.draftVersion !== s.draft.version ||
    s.dirty
  )
    throw new Error(
      s.t(
        "Черновик изменился. Проверьте изменения заново.",
        "Draft changed. Review it again.",
      ),
    );
  s.plan = null;
  s.diagnostics = [];
  try {
    s.plan = await s.client.query<Plan>({
      type: "plan_apply",
      draft_id: s.draft.draftId,
      expected_draft_version: s.review.draftVersion,
      expected_active: s.review.expectedActive,
      allow_disruption: s.review.consent,
    });
    s.review.requiresDisruption = s.plan.requiresDisruption;
  } catch (error) {
    if (
      (error as { details?: { requiresDisruption?: boolean } }).details
        ?.requiresDisruption
    )
      s.review.requiresDisruption = true;
    else throw error;
  }
}
export async function confirmApply(s: EditorSession) {
  await s.run(async () => {
    if (
      !s.review ||
      !s.draft ||
      s.dirty ||
      s.draft.version !== s.review.draftVersion
    )
      throw new Error(
        s.t(
          "Сначала обновите проверку изменений.",
          "Refresh the review first.",
        ),
      );
    if (s.review.requiresDisruption && !s.review.consent)
      throw new Error(
        s.t(
          "Подтвердите прерывание работающих действий.",
          "Confirm interruption of running actions.",
        ),
      );
    if (!s.plan) {
      await planReview(s);
      return;
    }
    s.pending = s.envelope({
      type: "apply_draft",
      draft_id: s.draft.draftId,
      expected_draft_version: s.review.draftVersion,
      expected_active: s.review.expectedActive,
      allow_disruption: s.plan.disruptionAllowed,
      plan_hash: s.plan.planHash,
    });
    s.attempted = false;
    await s.finishApply();
    s.review = null;
  });
}
