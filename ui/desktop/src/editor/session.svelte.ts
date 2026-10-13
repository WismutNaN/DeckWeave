// SPDX-License-Identifier: GPL-3.0-or-later
import { pushHistory } from "./history";
import { LiveEditing } from "./live-editing";
import { removeEditorFeedback, syncEditorFeedback } from "./feedback";
import { copy, message, type EditorClient } from "./api";
import {
  saveDraft,
  applyDraft,
  finishApply,
  confirmApply,
  planReview,
} from "./operations";
import { defaultBackground, layout, putLayout } from "./appearance";
import { appendAction, equal, sourceFor } from "./bindings";
import { mergeDocuments, type RecoveryJournal } from "./recovery";
import type {
  Action,
  ApplyReview,
  DraftDiagnostic,
  Catalog,
  Device,
  Draft,
  EditorDevice,
  GraphDocument,
  Locale,
  MutationEnvelope,
  Operation,
  PackageRef,
  Plan,
  Snapshot,
} from "./types";
export class EditorSession {
  draft = $state<Draft | null>(null);
  document = $state<GraphDocument | null>(null);
  snapshot = $state<Snapshot | null>(null);
  catalogs = $state<Catalog[]>([]);
  locale = $state<Locale>({
    revision: 1,
    requestedLocale: "ru",
    timeZone: "UTC",
  });
  review = $state<ApplyReview | null>(null);
  diagnostics = $state<DraftDiagnostic[]>([]);
  busy = $state(false);
  liveEnabled = $state(false);
  liveUpdating = $state(false);
  liveStatus = $state<"idle" | "queued" | "updating" | "error" | "paused">(
    "paused",
  );
  readonly liveEditing = new LiveEditing(this);
  get editLocked() {
    return !this.liveUpdating && (this.busy || Boolean(this.pending));
  }
  error = $state("");
  notice = $state("");
  selectedDevice = $state("");
  selectedControl = $state("");
  selectedStep = $state("");
  undoStack = $state<GraphDocument[]>([]);
  redoStack = $state<GraphDocument[]>([]);
  pending = $state<MutationEnvelope | null>(null);
  plan = $state<Plan | null>(null);
  operation = $state<Operation | null>(null);
  epoch = "";
  attempted = false;
  operationId = $state<string | null>(null);
  conflict = $state(false);
  localePending = $state(false);
  journal?: RecoveryJournal;
  private journalWrites: Promise<void> = Promise.resolve();
  async persist() {
    if (this.draft && this.document && this.journal) {
      const journal = this.journal;
      const value = {
        version: 1 as const,
        base: copy(this.draft),
        document: copy(this.document),
        pending: copy(this.pending),
        attempted: this.attempted,
        operationId: this.operationId,
      };
      this.journalWrites = this.journalWrites
        .catch(() => {})
        .then(() => journal.write(value));
      await this.journalWrites;
    }
  }
  persistSoon() {
    void this.persist().catch((cause) => {
      this.error =
        this.t(
          "Не удалось сохранить восстановление черновика: ",
          "Draft recovery could not be saved: ",
        ) + message(cause, this.lang);
    });
  }
  async sendPending<T>(): Promise<T> {
    if (!this.pending) throw new Error("No pending command");
    const replay = this.attempted;
    this.attempted = true;
    await this.persist(); // Publish the retry identity before any possible effect.
    try {
      return await this.client.command<T>(copy(this.pending));
    } catch (cause) {
      const code = (cause as { code?: string })?.code;
      if (
        !replay &&
        code &&
        [
          "invalid_command",
          "permission_denied",
          "plan_conflict",
          "revision_conflict",
          "capacity_exceeded",
          "not_found",
        ].includes(code)
      ) {
        this.pending = null;
        this.attempted = false;
        this.conflict = code === "revision_conflict";
        await this.persist();
      }
      throw cause;
    }
  }
  constructor(
    public client: EditorClient,
    public preview = false,
  ) {}
  get lang(): "ru" | "en" {
    return this.locale.requestedLocale.startsWith("ru") ? "ru" : "en";
  }
  t(ru: string, en: string) {
    return this.lang === "ru" ? ru : en;
  }
  get devices(): EditorDevice[] {
    return this.document ? layout(this.document).devices : [];
  }
  get device(): EditorDevice | undefined {
    return this.devices.find((d) => d.id === this.selectedDevice);
  }
  get dirty(): boolean {
    return (
      this.document !== null && !equal(this.document, this.draft?.document)
    );
  }
  get actions(): Action[] {
    return this.availableCatalogs.flatMap((c) =>
      c.endpoints
        .filter((e) => e.operation)
        .map((e) => ({
          ...e,
          catalog: c,
          title: this.label(c, e.operation!.operationId),
        })),
    );
  }
  get availableCatalogs(): Catalog[] {
    return this.catalogs.filter((c) => {
      const pin = this.draft?.packageLock.find(
        (p) => p.packageId === c.package.packageId,
      );
      return pin
        ? pin.version === c.package.version &&
            pin.artifactSha256 === c.package.artifactSha256
        : this.catalogs.filter(
            (p) => p.package.packageId === c.package.packageId,
          ).length === 1;
    });
  }
  get ambiguousPackages(): string[] {
    return [
      ...new Set(
        this.catalogs
          .filter(
            (c) =>
              !this.draft?.packageLock.some(
                (p) => p.packageId === c.package.packageId,
              ) &&
              this.catalogs.filter(
                (p) => p.package.packageId === c.package.packageId,
              ).length > 1,
          )
          .map((c) => c.package.packageId),
      ),
    ];
  }
  label(catalog: Catalog, key: string): string {
    const locales = [
      this.locale.requestedLocale,
      this.lang,
      catalog.translations?.defaultLocale ?? "en",
    ];
    for (const locale of locales) {
      const value = catalog.translations?.messages[locale]?.[key];
      if (value) return value;
    }
    return key;
  }
  async run(task: () => Promise<void>) {
    if (this.busy) return;
    this.busy = true;
    this.error = "";
    try {
      await task();
    } catch (cause) {
      this.diagnostics =
        (cause as { details?: { diagnostics?: DraftDiagnostic[] } })?.details
          ?.diagnostics ?? [];
      this.error = message(cause, this.lang);
    } finally {
      this.busy = false;
      this.liveEditing.wake();
    }
  }
  async load() {
    await this.run(async () => {
      this.draft = await this.client.open();
      layout(this.draft.document);
      this.document = copy(this.draft.document);
      const recovery = await this.journal?.read();
      if (recovery && recovery.base.draftId === this.draft.draftId) {
        layout(recovery.document);
        if (
          recovery.pending ||
          !equal(recovery.document, recovery.base.document)
        ) {
          this.conflict = recovery.base.version !== this.draft.version;
          this.draft = recovery.base;
          this.document = recovery.document;
          this.pending = recovery.pending;
          this.attempted = recovery.attempted ?? Boolean(recovery.pending);
        }
        this.operationId = recovery.operationId;
      }
      this.locale = await this.client.locale();
      this.epoch = (await this.client.window()).dedupEpoch;
      await this.refresh();
      this.selectedDevice = this.devices[0]?.id ?? "";
      if (this.pending) {
        this.notice = this.t(
          "Восстановлен незавершённый запрос. Повторите его для проверки результата.",
          "An unfinished request was restored. Retry it to resolve its outcome.",
        );
      }
      await this.persist();
    });
  }
  async refresh() {
    const snapshot = await this.client.query<Snapshot>({
      type: "editor_snapshot",
    });
    const catalogs = await Promise.all(
      snapshot.installed.map((p) =>
        this.client.query<Catalog>({
          type: "plugin_catalog",
          package: p.package,
        }),
      ),
    );
    this.snapshot = snapshot;
    this.catalogs = catalogs;
    if (this.operationId)
      this.operation = await this.client.query<Operation>({
        type: "operation",
        operation_id: this.operationId,
      });
    if (this.localePending) {
      const result = await this.client.reconcileLocale();
      this.locale = result.context;
      this.localePending = result.deliveryPending;
    }
  }
  edit(change: (document: GraphDocument) => void) {
    if (!this.document || this.editLocked) return;
    try {
      const before = copy(this.document),
        next = copy(this.document);
      removeEditorFeedback(next);
      change(next);
      syncEditorFeedback(next, this.availableCatalogs);
      if (equal(before, next)) return;
      this.undoStack = pushHistory(this.undoStack, before);
      this.redoStack = [];
      this.document = next;
      this.persistSoon();
      this.plan = null;
      this.review = null;
      this.diagnostics = [];
      this.notice = "";
      this.error = "";
      this.liveEditing.changed();
    } catch (cause) {
      this.error = message(cause, this.lang);
    }
  }
  history(direction: "undo" | "redo") {
    if (!this.document || this.editLocked) return;
    const source = direction === "undo" ? this.undoStack : this.redoStack;
    const next = source.at(-1);
    if (!next) return;
    const current = copy(this.document);
    if (direction === "undo") {
      this.undoStack = source.slice(0, -1);
      this.redoStack = pushHistory(this.redoStack, current);
    } else {
      this.redoStack = source.slice(0, -1);
      this.undoStack = pushHistory(this.undoStack, current);
    }
    this.document = copy(next);
    this.persistSoon();
    this.plan = null;
    this.review = null;
    this.diagnostics = [];
    this.notice = "";
    this.liveEditing.changed();
  }
  select(deviceId: string, controlId = "") {
    this.selectedDevice = deviceId;
    this.selectedControl = controlId;
    this.selectedStep = "";
  }
  addDevice(packageRef: PackageRef, nodeId: string, descriptor: Device) {
    const id = JSON.stringify([
      packageRef.packageId,
      nodeId,
      descriptor.providerId,
      descriptor.identity,
    ]);
    this.edit((document) => {
      const state = layout(document);
      if (!state.devices.some((d) => d.id === id))
        state.devices.push({
          id,
          package: packageRef,
          nodeId,
          descriptor,
          x: state.devices.length * 520,
          y: 0,
          appearance: defaultBackground(),
          controls: {},
        });
      putLayout(document, state);
    });
    this.select(id);
  }
  updateDevice(change: (device: EditorDevice) => void) {
    this.edit((document) => {
      const state = layout(document),
        device = state.devices.find((d) => d.id === this.selectedDevice);
      if (device) change(device);
      putLayout(document, state);
    });
  }
  assign(action: Action) {
    const device = this.device;
    if (!device || !this.selectedControl) return;
    this.edit((document) => {
      this.selectedStep = appendAction(
        document,
        this.availableCatalogs,
        device,
        this.selectedControl,
        action,
        crypto.randomUUID(),
      );
    });
  }
  get hasSource() {
    return Boolean(
      this.device &&
      sourceFor(this.catalogs, this.device, this.selectedControl),
    );
  }
  envelope(command: Record<string, unknown>): MutationEnvelope {
    return {
      requestId: crypto.randomUUID(),
      idempotencyKey: crypto.randomUUID(),
      dedupEpoch: this.epoch,
      dryRun: false,
      command,
    };
  }
  async save() {
    return saveDraft(this);
  }
  async apply() {
    return applyDraft(this);
  }
  async confirmApply() {
    return confirmApply(this);
  }
  async planReview() {
    return this.run(() => planReview(this));
  }
  async retry() {
    await this.run(async () => {
      if (this.pending?.command.type === "patch_draft") await this.save();
      else if (this.pending?.command.type === "apply_draft")
        await this.finishApply();
    });
  }
  async finishApply() {
    return finishApply(this);
  }
  async mergeLatest() {
    await this.run(async () => {
      if (!this.draft || !this.document || this.pending) return;
      const remote = await this.client.query<Draft>({
        type: "draft",
        draft_id: this.draft.draftId,
      });
      const merged = mergeDocuments(
        this.draft.document,
        this.document,
        remote.document,
      );
      this.draft = remote;
      this.document = merged;
      this.conflict = false;
      this.plan = null;
      this.review = null;
      this.diagnostics = [];
      await this.persist();
      this.notice = this.t(
        "Изменения объединены. Проверьте и сохраните черновик.",
        "Changes merged. Review and save your draft.",
      );
    });
  }
  async language(lang: "ru" | "en") {
    await this.run(async () => {
      const next = {
        ...this.locale,
        requestedLocale: lang,
        revision: this.locale.revision + 1,
      };
      const result = await this.client.changeLocale(this.locale.revision, next);
      this.locale = result.context;
      this.localePending = result.deliveryPending;
      if (this.localePending) {
        const delivery = await this.client.reconcileLocale();
        this.locale = delivery.context;
        this.localePending = delivery.deliveryPending;
      }
      if (this.localePending)
        this.notice = this.t(
          "Язык сохранён; плагины ожидают обновления.",
          "Language saved; plugins await delivery.",
        );
    });
  }
}
