// SPDX-License-Identifier: GPL-3.0-or-later
import { copy, message } from "./api";
import { saveDraft, finishApply } from "./operations";
import type { EditorSession } from "./session.svelte";
import type { ActiveSelection, DraftDiagnostic, Plan } from "./types";

// One writer owns save → plan → commit. Later edits replace queued work, never
// the identity of a command that may already have crossed the process boundary.
export class LiveEditing {
  private timer?: ReturnType<typeof setTimeout>;
  private revision = 0;
  private handled = 0;
  private running: Promise<void> | null = null;
  constructor(
    private session: EditorSession,
    private delay = 180,
  ) {}

  start() {
    // Loading a recovery journal is not permission to replay uncertain work.
    this.session.liveEnabled = true;
    this.session.liveStatus = "idle";
  }
  pause() {
    this.session.liveEnabled = false;
    this.session.liveStatus = "paused";
    clearTimeout(this.timer);
  }
  dispose() {
    this.pause();
  }
  changed() {
    this.revision++;
    this.wake();
  }
  resume() {
    this.start();
    this.changed();
  }
  wake() {
    if (
      !this.session.liveEnabled ||
      this.running ||
      this.revision === this.handled
    )
      return;
    clearTimeout(this.timer);
    this.session.liveStatus = "queued";
    this.timer = setTimeout(() => {
      void this.flush();
    }, this.delay);
  }
  async flush(): Promise<void> {
    clearTimeout(this.timer);
    if (this.running) return this.running;
    const s = this.session;
    if (!s.liveEnabled || this.revision === this.handled) return;
    if (s.busy) return; // A manual operation wakes this writer when it finishes.
    if (s.pending || s.conflict || !s.document || !s.draft) {
      this.handled = this.revision;
      s.liveStatus = "error";
      return;
    }
    this.running = this.drain();
    return this.running;
  }
  private async drain() {
    const s = this.session;
    s.busy = true;
    s.liveUpdating = true;
    s.liveStatus = "updating";
    s.error = "";
    s.diagnostics = [];
    try {
      while (s.liveEnabled && this.handled !== this.revision) {
        const revision = this.revision;
        this.handled = revision;
        await saveDraft(s);
        if (!s.liveEnabled) break;
        if (revision !== this.revision || s.dirty) continue;
        const draft = copy(s.draft!);
        let active: { selection: ActiveSelection | null }, plan: Plan;
        try {
          active = await s.client.query<{ selection: ActiveSelection | null }>({
            type: "active_document",
          });
          if (!s.liveEnabled || revision !== this.revision) continue;
          plan = await s.client.query<Plan>({
            type: "plan_apply",
            draft_id: draft.draftId,
            expected_draft_version: draft.version,
            expected_active: active.selection,
            allow_disruption: false,
          });
        } catch (cause) {
          // A read-only validation failure for an obsolete form must not hide
          // a newer edit. Mutations with uncertain outcomes never take this path.
          if (s.liveEnabled && revision !== this.revision) continue;
          throw cause;
        }
        if (
          !s.liveEnabled ||
          revision !== this.revision ||
          s.dirty ||
          s.draft?.version !== draft.version
        )
          continue;
        if (plan.requiresDisruption)
          throw { details: { requiresDisruption: true } };
        s.pending = s.envelope({
          type: "apply_draft",
          draft_id: draft.draftId,
          expected_draft_version: draft.version,
          expected_active: active.selection,
          allow_disruption: false,
          plan_hash: plan.planHash,
        });
        s.attempted = false;
        await finishApply(s);
        if (
          !s.preview &&
          (s.operation?.state !== "succeeded" ||
            s.operation.runtimeHealth !== "applied")
        )
          throw new Error(
            s.t(
              "Изменения сохранены, но не работают. Откройте состояние выполнения.",
              "Changes were saved but are not running. Open runtime status.",
            ),
          );
      }
      s.liveStatus = s.liveEnabled ? "idle" : "paused";
    } catch (cause) {
      // No automatic retry after an uncertain command or a failed activation.
      this.handled = this.revision;
      const details = (
        cause as {
          details?: {
            diagnostics?: DraftDiagnostic[];
            requiresDisruption?: boolean;
          };
        }
      )?.details;
      s.diagnostics = details?.diagnostics ?? [];
      s.error = details?.requiresDisruption
        ? s.t(
            "Требуется прерывание устройства. Приостановите автоприменение и проверьте изменения.",
            "Device interruption is required. Pause live editing and review the changes.",
          )
        : message(cause, s.lang);
      s.liveStatus = "error";
    } finally {
      s.liveUpdating = false;
      s.busy = false;
      this.running = null;
      this.wake();
    }
  }
}
