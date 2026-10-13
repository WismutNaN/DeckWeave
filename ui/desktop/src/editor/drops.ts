// SPDX-License-Identifier: GPL-3.0-or-later
import type { TransferDrop } from "./pointer-drag";
import type { EditorSession } from "./session.svelte";
import { readActionDrag, readSelectionDrag } from "./drag";
import { copyControlBinding, reorderAction } from "./binding-edits";
import { equal } from "./bindings";
import { layout, putLayout } from "./appearance";
import { copy } from "./api";
export function dropOnControl(
  session: EditorSession,
  event: TransferDrop,
  deviceId: string,
  controlId: string,
  beforeStep?: string,
) {
  const endpoint = readActionDrag(event.dataTransfer),
    selection = readSelectionDrag(event.dataTransfer);
  if (!endpoint && !selection) return;
  event.preventDefault();
  event.stopPropagation();
  if (session.busy || session.pending) return;
  session.select(deviceId, controlId);
  if (endpoint) {
    const action = session.actions.find((a) => equal(a.endpoint, endpoint));
    if (action) session.assign(action);
  } else if (
    selection?.kind === "step" &&
    selection.deviceId === deviceId &&
    selection.controlId === controlId &&
    beforeStep
  ) {
    session.edit((doc) =>
      reorderAction(
        doc,
        session.availableCatalogs,
        deviceId,
        controlId,
        selection.stepId!,
        beforeStep,
      ),
    );
  } else if (selection?.kind === "control") {
    session.edit((doc) => {
      const state = layout(doc),
        source = state.devices.find((d) => d.id === selection.deviceId),
        target = state.devices.find((d) => d.id === deviceId);
      if (
        !source ||
        !target ||
        !source.descriptor.elements.some(
          (e) => e.elementId === selection.controlId,
        ) ||
        !target.descriptor.elements.some((e) => e.elementId === controlId)
      )
        return;
      if (source.id === target.id && selection.controlId === controlId) return;
      const original = source.controls[selection.controlId];
      if (original)
        target.controls[controlId] = {
          ...copy(original),
          trigger: target.controls[controlId]?.trigger,
        };
      copyControlBinding(
        doc,
        session.availableCatalogs,
        source,
        selection.controlId,
        target,
        controlId,
        event.shiftKey,
      );
      if (event.shiftKey) delete source.controls[selection.controlId];
      putLayout(doc, state);
    });
  }
}
