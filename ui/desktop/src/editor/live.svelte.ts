// SPDX-License-Identifier: GPL-3.0-or-later
import { equal, sourceFor } from "./bindings";
import { copy, message } from "./api";
import type { EditorSession } from "./session.svelte";
import type { EditorDevice, Endpoint, GraphDocument } from "./types";
type Surface = {
  activationEpoch: number | null;
  displays: { endpoint: Endpoint; text: string }[];
};
export class LiveControls {
  enabled = $state(false);
  pending = $state(false);
  surface = $state<Surface>({ activationEpoch: null, displays: [] });
  document = $state<GraphDocument | null>(null);
  notice = $state("");
  error = $state("");
  private operationId: string | null = null;
  constructor(public session: EditorSession) {}
  get ready() {
    return (
      this.surface.activationEpoch !== null &&
      this.document !== null &&
      !this.session.dirty &&
      equal(this.session.document, this.document) &&
      !this.session.pending &&
      !this.session.busy &&
      !this.session.preview
    );
  }
  async refresh() {
    try {
      const surface = await this.session.client.query<Surface>({
        type: "virtual_surface",
      });
      // Layout-only commits retain their execution epoch but change the document.
      if (
        !this.document ||
        surface.activationEpoch !== this.surface.activationEpoch ||
        this.operationId !== this.session.operationId
      ) {
        await this.session.refresh();
        const active = await this.session.client.query<{
          document: GraphDocument | null;
        }>({ type: "active_document" });
        this.document = active.document;
        this.operationId = this.session.operationId;
      }
      this.surface = surface;
    } catch (error) {
      this.document = null;
      this.surface = { activationEpoch: null, displays: [] };
      throw error;
    }
  }
  async toggle() {
    this.enabled = !this.enabled;
    this.error = "";
    this.notice = "";
    if (this.enabled)
      try {
        await this.session.refresh();
        await this.refresh();
      } catch (e) {
        this.enabled = false;
        this.error = message(e, this.session.lang);
      }
  }
  text(device: EditorDevice, control: string) {
    return this.surface.displays.find(
      (d) =>
        d.endpoint.packageId === device.package.packageId &&
        d.endpoint.nodeId === device.nodeId &&
        d.endpoint.target.kind === "device" &&
        d.endpoint.target.providerId === device.descriptor.providerId &&
        d.endpoint.target.deviceIdentity === device.descriptor.identity &&
        d.endpoint.port.elementId === control,
    )?.text;
  }
  async click(device: EditorDevice, control: string) {
    const s = this.session;
    if (this.pending || !this.ready || device.descriptor.kind !== "virtual")
      return;
    const endpoint = sourceFor(s.availableCatalogs, device, control)?.endpoint;
    const generation = s.snapshot?.nodes.find(
      (n) =>
        n.packageId === device.package.packageId &&
        n.descriptor.nodeId === device.nodeId,
    )?.generation;
    if (
      !endpoint ||
      !generation ||
      !this.surface.activationEpoch ||
      !s.snapshot?.active
    ) {
      this.error = s.t(
        "Сначала назначьте действие и примените конфигурацию.",
        "Assign an action and apply the configuration first.",
      );
      return;
    }
    this.pending = true;
    this.error = "";
    this.notice = "";
    try {
      const result = await s.client.command<{
        accepted: boolean;
        replayed: boolean;
      }>(
        s.envelope({
          type: "virtual_click",
          request: {
            endpoint: copy(endpoint),
            expectedActive: copy(s.snapshot.active),
            activationEpoch: this.surface.activationEpoch,
            sessionGeneration: generation,
          },
        }),
      );
      this.notice = result.accepted
        ? s.t(
            "Нажатие передано. Результат появится после выполнения.",
            "Click sent. The result will appear after execution.",
          )
        : s.t(
            "Исход предыдущего нажатия неизвестен. Оно не повторено.",
            "The previous click outcome is unknown. It was not repeated.",
          );
      await this.refresh();
    } catch (e) {
      this.error =
        s.t(
          "Нажатие не подтверждено; автоматического повтора нет. ",
          "Click was not confirmed; it was not retried. ",
        ) + message(e, s.lang);
    } finally {
      this.pending = false;
    }
  }
}
