// SPDX-License-Identifier: GPL-3.0-or-later
import { copy, message, type EditorClient } from "./api";
import { defaultBackground, layout, putLayout } from "./appearance";
import { appendAction, equal, sourceFor } from "./bindings";
import type {
  Action,
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
  busy = $state(false);
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
    return this.catalogs.flatMap((c) =>
      c.endpoints
        .filter((e) => e.operation)
        .map((e) => ({
          ...e,
          catalog: c,
          title: this.label(c, e.operation!.operationId),
        })),
    );
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
      // Only explicit API rejections prove a mutation did not take effect.
      const code = (cause as { code?: string })?.code;
      if (
        code &&
        [
          "invalid_command",
          "permission_denied",
          "plan_conflict",
          "revision_conflict",
          "capacity_exceeded",
          "not_found",
        ].includes(code)
      )
        this.pending = null;
      this.error = message(cause, this.lang);
    } finally {
      this.busy = false;
    }
  }
  async load() {
    await this.run(async () => {
      this.draft = await this.client.open();
      this.document = copy(this.draft.document);
      this.locale = await this.client.locale();
      this.epoch = (await this.client.window()).dedupEpoch;
      await this.refresh();
      this.selectedDevice = this.devices[0]?.id ?? "";
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
  }
  edit(change: (document: GraphDocument) => void) {
    if (!this.document || this.busy || this.pending) return;
    try {
      const before = copy(this.document),
        next = copy(this.document);
      change(next);
      if (equal(before, next)) return;
      this.undoStack = [...this.undoStack.slice(-63), before];
      this.redoStack = [];
      this.document = next;
      this.plan = null;
      this.notice = "";
      this.error = "";
    } catch (cause) {
      this.error = message(cause, this.lang);
    }
  }
  history(direction: "undo" | "redo") {
    if (!this.document || this.busy || this.pending) return;
    const source = direction === "undo" ? this.undoStack : this.redoStack;
    const next = source.at(-1);
    if (!next) return;
    const current = copy(this.document);
    if (direction === "undo") {
      this.undoStack = source.slice(0, -1);
      this.redoStack = [...this.redoStack, current];
    } else {
      this.redoStack = source.slice(0, -1);
      this.undoStack = [...this.undoStack, current];
    }
    this.document = copy(next);
    this.plan = null;
    this.notice = "";
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
        this.catalogs,
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
    if (this.pending && this.pending.command.type !== "patch_draft")
      throw new Error(
        this.t(
          "Сначала завершите запрос применения.",
          "Resolve the pending apply request first.",
        ),
      );
    if (!this.draft || !this.document || (!this.dirty && !this.pending)) return;
    const locks = [...this.draft.packageLock];
    const required = new Set(
      this.document.graph.nodes.flatMap((n) =>
        n.kind.endpoint ? [n.kind.endpoint.packageId] : [],
      ),
    );
    for (const packageId of required) {
      if (locks.some((p) => p.packageId === packageId)) continue;
      const selected = this.catalogs.filter(
        (c) => c.package.packageId === packageId,
      );
      if (selected.length !== 1)
        throw new Error(
          this.t(
            "Нужна однозначная версия плагина.",
            "Select an unambiguous plugin version.",
          ),
        );
      locks.push({ ...selected[0].package, source: null });
    }
    this.pending ??= this.envelope({
      type: "patch_draft",
      draft_id: this.draft.draftId,
      expected_version: this.draft.version,
      edit: {
        type: "replace_configuration",
        document: copy(this.document),
        package_lock: locks,
      },
    });
    const result = await this.client.command<{ draft: Draft }>(
      copy(this.pending),
    );
    this.draft = result.draft;
    this.document = copy(result.draft.document);
    this.pending = null;
    this.notice = this.t("Черновик сохранён", "Draft saved");
  }
  async apply() {
    await this.run(async () => {
      if (this.pending?.command.type === "apply_draft") {
        await this.finishApply();
        return;
      }
      await this.save();
      if (!this.draft) return;
      this.plan = await this.client.query<Plan>({
        type: "plan_apply",
        draft_id: this.draft.draftId,
        expected_draft_version: this.draft.version,
        expected_active: this.snapshot?.active ?? null,
        allow_disruption: false,
      });
      this.pending = this.envelope({
        type: "apply_draft",
        draft_id: this.draft.draftId,
        expected_draft_version: this.draft.version,
        expected_active: this.snapshot?.active ?? null,
        allow_disruption: false,
        plan_hash: this.plan.planHash,
      });
      await this.finishApply();
    });
  }
  async retry() {
    await this.run(async () => {
      if (this.pending?.command.type === "patch_draft") await this.save();
      else if (this.pending?.command.type === "apply_draft")
        await this.finishApply();
    });
  }
  async finishApply() {
    if (!this.pending) return;
    const receipt = await this.client.command<{ operationId: string }>(
      copy(this.pending),
    );
    this.operation = await this.client.query<Operation>({
      type: "operation",
      operation_id: receipt.operationId,
    });
    await this.refresh();
    this.pending = null;
    this.notice = this.preview
      ? this.t(
          "Пример сохранён. Оборудование не используется.",
          "Example saved. No hardware is used.",
        )
      : this.operation.state === "succeeded"
        ? this.t("Действия применены", "Actions applied")
        : this.t(
            `Состояние применения: ${this.operation.state}`,
            `Apply status: ${this.operation.state}`,
          );
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
      if (result.deliveryPending)
        this.notice = this.t(
          "Язык сохранён; плагины ожидают обновления.",
          "Language saved; plugins await delivery.",
        );
    });
  }
}
