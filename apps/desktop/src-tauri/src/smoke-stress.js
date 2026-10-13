// SPDX-License-Identifier: GPL-3.0-or-later
// A draft-only stress fixture. It is never applied to the provider and is restored after rendering.
window.deckweaveSmokeStress = async () => {
  const invoke = (name, args = {}) =>
    window.__TAURI_INTERNALS__.invoke(name, args);
  const assert = (v, m) => {
    if (!v) throw new Error(m);
  };
  const pause = () => new Promise((r) => setTimeout(r, 30));
  const key = "deckweave-smoke-stress";
  const save = async (document, label) => {
    const current = await invoke("open_workspace"),
      window = await invoke("mutation_window");
    await invoke("app_command", {
      envelope: {
        requestId: label,
        idempotencyKey: label,
        dedupEpoch: window.dedupEpoch,
        dryRun: false,
        command: {
          type: "patch_draft",
          draft_id: current.draftId,
          expected_version: current.version,
          edit: {
            type: "replace_configuration",
            document,
            package_lock: current.packageLock,
          },
        },
      },
    });
  };
  if (!sessionStorage.getItem(key)) {
    const draft = await invoke("open_workspace"),
      document = structuredClone(draft.document);
    sessionStorage.setItem(key + "-original", JSON.stringify(draft.document));
    const device = document.extensions["org.deckweave.editor"].devices[0];
    const element = { capabilities: [] }; // Rendering fixture; hardware contracts are tested by the separate native E2E.
    delete device.descriptor["org.deckweave.controlLayout"];
    device.descriptor.elements = Array.from({ length: 4096 }, (_, i) => ({
      ...element,
      elementId: `stress-${i}`,
      label: `Control ${i}`,
    }));
    device.controls = {};
    const action = document.graph.nodes.find((n) => n.kind.type === "action");
    document.graph.nodes = Array.from({ length: 4096 }, (_, i) => ({
      ...action,
      id: `stress-action-${i}`,
      extensions: {
        "org.deckweave.editorBinding": JSON.stringify([
          device.id,
          `stress-${i}`,
        ]),
      },
    }));
    document.graph.edges = [];
    document.graph.actionArguments = {};
    document.graph.nodeInstances = {};
    await save(document, "smoke-stress-seed");
    sessionStorage.setItem(key, "render");
    location.reload();
    return null;
  }
  if (sessionStorage.getItem(key) === "render") {
    const liveToggle = document.querySelector(".live-edit-toggle");
    if (liveToggle?.getAttribute("aria-pressed") === "true") {
      liveToggle.click();
      await pause();
    }
    const started = performance.now();
    for (
      let i = 0;
      i < 160 &&
      !document.querySelector('.device-key[data-control-id^="stress-"]');
      i++
    )
      await pause();
    const keys = () => [...document.querySelectorAll(".device-key")];
    assert(
      keys().length > 0 && keys().length < 256,
      `unbounded controls: ${keys().length}`,
    );
    const before = keys()[0].dataset.controlId;
    document
      .querySelector(".workspace-stage")
      .dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: 10000 }));
    await pause();
    assert(
      keys().length > 0 &&
        keys().length < 256 &&
        keys()[0].dataset.controlId !== before,
      "viewport did not remount after scroll",
    );
    keys()[0].focus();
    keys()[0].dispatchEvent(
      new KeyboardEvent("keydown", { bubbles: true, key: "ArrowRight" }),
    );
    await pause();
    assert(
      document.activeElement.classList.contains("device-key"),
      "selection lost during virtualization",
    );
    const latency = Math.round(performance.now() - started);
    const input = document.querySelector(".inspector-panel input");
    assert(input, "large draft inspector");
    const original = input.value;
    input.value = "Large draft edit";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await pause();
    const recovery = async () => {
      const db = await new Promise((resolve, reject) => {
        const r = indexedDB.open("deckweave-editor", 1);
        r.onsuccess = () => resolve(r.result);
        r.onerror = () => reject(r.error);
      });
      try {
        return await new Promise((resolve, reject) => {
          const r = db
            .transaction("recovery")
            .objectStore("recovery")
            .get("deckweave.editor.recovery.v1");
          r.onsuccess = () => resolve(r.result);
          r.onerror = () => reject(r.error);
        });
      } finally {
        db.close();
      }
    };
    let journal;
    for (let i = 0; i < 160; i++) {
      journal = await recovery();
      if (
        JSON.stringify(journal?.document.extensions).includes(
          "Large draft edit",
        )
      )
        break;
      await pause();
    }
    assert(
      JSON.stringify(journal?.document.extensions).includes("Large draft edit"),
      "large edit was not durably journaled",
    );
    window.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "я",
        code: "KeyZ",
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    await pause();
    assert(
      document.querySelector(".inspector-panel input").value === original,
      "large undo failed",
    );
    for (let i = 0; i < 160; i++) {
      journal = await recovery();
      if (
        !JSON.stringify(journal?.document.extensions).includes(
          "Large draft edit",
        )
      )
        break;
      await pause();
    }
    assert(
      !JSON.stringify(journal?.document.extensions).includes(
        "Large draft edit",
      ),
      "large undo was not durably journaled",
    );
    assert(latency < 5000, `viewport response ${latency}ms`);
    sessionStorage.setItem(
      key + "-detail",
      `4096 controls + 4096 graph nodes: ${keys().length} mounted, scroll/selection ${latency}ms, durable edit/undo`,
    );
    await save(
      JSON.parse(sessionStorage.getItem(key + "-original")),
      "smoke-stress-restore",
    );
    sessionStorage.setItem(key, "restored");
    location.reload();
    return null;
  }
  const original = JSON.parse(sessionStorage.getItem(key + "-original"));
  assert(
    JSON.stringify((await invoke("open_workspace")).document) ===
      JSON.stringify(original),
    "stress fixture not restored",
  );
  return sessionStorage.getItem(key + "-detail");
};
