// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { accentTokens, contrast, readTheme } from "../src/editor/theme";
import { visibleControls, canPan } from "../src/editor/viewport";
import { layout } from "../src/editor/appearance";
import { previewData } from "../src/editor/preview";
test("custom accents remain readable in both themes, including extreme user colors", () => {
  for (const dark of [false, true])
    for (const color of [
      "#ffffff",
      "#000000",
      "#ffff00",
      "#0000ff",
      "#267567",
      "#b04873",
    ]) {
      const value = accentTokens(color, dark);
      assert.ok(contrast(value.color, dark ? "#20272d" : "#ffffff") >= 4.5);
      assert.ok(contrast(value.color, value.foreground) >= 4.5);
    }
  assert.equal(readTheme({ getItem: () => "invalid" }).mode, "system");
  assert.equal(
    readTheme({ getItem: () => '{"mode":"dark","accent":"#123456"}' }).accent,
    "#123456",
  );
});
test("blank canvas pans without space but controls and device bodies keep their input", () => {
  assert.equal(canPan(0, false, false, false), true);
  assert.equal(canPan(0, false, true, false), false);
  assert.equal(canPan(0, false, false, true), false);
  assert.equal(canPan(1, false, true, true), true);
  assert.equal(canPan(2, true, false, false), false);
});
test("4096 controls stay bounded in the viewport; keyboard selection remains mountable", () => {
  const device = layout(previewData().draft.document).devices[0];
  delete device.descriptor["org.deckweave.controlLayout"];
  device.descriptor.elements = Array.from({ length: 4096 }, (_, i) => ({
    ...device.descriptor.elements[0],
    elementId: `large-${i}`,
  }));
  const view = { width: 900, height: 700, pan: { x: 0, y: 0 }, zoom: 1 };
  const visible = visibleControls(device, view, "large-4095");
  assert.ok(visible.length < 50);
  assert.ok(visible.some((x) => x.element.elementId === "large-4095"));
  const scrolled = visibleControls(
    device,
    { ...view, pan: { x: 0, y: -10000 } },
    "",
  );
  assert.ok(scrolled.length > 0 && scrolled.length < 50);
  assert.ok(scrolled.every((x) => x.index > 400));
});
