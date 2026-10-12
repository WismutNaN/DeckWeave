// SPDX-License-Identifier: GPL-3.0-or-later
import { previewData } from "../src/editor/preview";
import { layout } from "../src/editor/appearance";
import { appendAction } from "../src/editor/bindings";
export function editorFixture() {
  const { draft, catalog } = previewData();
  const device = layout(draft.document).devices[0];
  const actions = catalog.endpoints
    .filter((e) => e.operation)
    .map((e) => ({ ...e, catalog, title: e.operation!.operationId }));
  const first = appendAction(
    draft.document,
    [catalog],
    device,
    "key-1",
    actions[0],
    "first",
  );
  const second = appendAction(
    draft.document,
    [catalog],
    device,
    "key-1",
    actions[1],
    "second",
  );
  draft.document.graph.actionArguments![first].fields.text = {
    type: "constant",
    value: "Hello from the editor",
  };
  draft.document.graph.actionArguments![second].fields.text = {
    type: "run_result",
    nodeId: first,
    pointer: "",
  };
  draft.packageLock = [{ ...catalog.package, source: null }];
  return { draft, catalog };
}
