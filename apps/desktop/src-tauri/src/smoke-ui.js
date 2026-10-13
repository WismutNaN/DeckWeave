// SPDX-License-Identifier: GPL-3.0-or-later
// Runs only in a smoke-test WebView: actual components and HTML drag/drop events.
window.deckweaveSmokeUi = async () => {
  const invoke = (name, args = {}) =>
    window.__TAURI_INTERNALS__.invoke(name, args).catch(error => {
      throw Object.assign(new Error(`UI smoke: ${name} ${args.query?.type ?? args.envelope?.command?.type ?? ""}: ${error.messageKey ?? error}`), error);
    });
  const assert = (value, message) => {
    if (!value) throw new Error(message);
  };
  const sleep = () => new Promise((r) => setTimeout(r, 30));
  const wait = async (fn, description) => {
    for (let i = 0; i < 160; i++) {
      const value = await fn();
      if (value) return value;
      await sleep();
    }
    throw new Error(`${description}: ${document.body.innerText.slice(-2400)}`);
  };
  const button = (text, root = document) =>
    [...root.querySelectorAll("button")].find(
      (e) =>
        e.textContent.trim() === text || e.getAttribute("aria-label") === text,
    );
  const click = async (text, root = document) => {
    const b = await wait(() => button(text, root), `button ${text}`);
    assert(!b.disabled, `disabled ${text}`);
    b.click();
    await sleep();
  };
  const field = (label, root = document) =>
    [...root.querySelectorAll("label")]
      .find((e) => e.querySelector("span")?.textContent === label)
      ?.querySelector("input,select,textarea");
  const set = async (element, value, type = "change") => {
    assert(element, "field missing");
    element.value = value;
    element.dispatchEvent(new Event(type, { bubbles: true }));
    await sleep();
  };
  const key = (id) =>
    document.querySelector(`.device-key[data-control-id="${id}"]`);
  const drag = async (source, target, shiftKey = false) => {
    assert(source && target, "drag endpoints");
    const dataTransfer = new DataTransfer();
    source.dispatchEvent(
      new DragEvent("dragstart", { bubbles: true, dataTransfer }),
    );
    target.dispatchEvent(
      new DragEvent("dragover", {
        bubbles: true,
        cancelable: true,
        dataTransfer,
        shiftKey,
      }),
    );
    target.dispatchEvent(
      new DragEvent("drop", {
        bubbles: true,
        cancelable: true,
        dataTransfer,
        shiftKey,
      }),
    );
    source.dispatchEvent(
      new DragEvent("dragend", { bubbles: true, dataTransfer }),
    );
    await sleep();
  };
  assert(button("Применять сразу")?.getAttribute("aria-pressed") === "true", "live editing defaults on");
  // Keep coverage of explicit review while a user assembles an unfinished flow.
  await click("Применять сразу");
  await click("Настройки приложения");
  await set(field("Тема"), "dark");
  await click("Акцент #8661c5");
  assert(document.documentElement.dataset.theme === "dark", "dark theme");
  assert(
    JSON.parse(localStorage.getItem("deckweave.appearance.v1")).accent ===
      "#8661c5",
    "accent persistence",
  );
  await click("Закрыть", document.querySelector(".settings-popover"));
  await click("Добавить устройство", document.querySelector(".library-panel"));
  await wait(
    () => document.querySelector(".picker-device"),
    "bundled virtual devices",
  );
  const available = [...document.querySelectorAll(".picker-device")];
  assert(
    available.some((e) => e.textContent.includes("Virtual Deck A")) &&
      available.some((e) => e.textContent.includes("Virtual Deck B")),
    "distinct virtual templates",
  );
  available.find((e) => e.textContent.includes("Virtual Deck A")).click();
  await sleep();
  key("key-1").focus();
  key("key-1").dispatchEvent(
    new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
  );
  await sleep();
  assert(
    document.activeElement.dataset.controlId === "key-2",
    "spatial keyboard navigation",
  );
  // Bundled Windows packages also expose actions; select this fixture explicitly.
  await set(document.querySelector("#action-search"), "org.deckweave.virtual", "input");
  await drag(document.querySelector(".library-action"), key("key-1"));
  assert(
    document.querySelectorAll(".action-chain li").length === 1,
    "library to control drag",
  );
  await set(
    document.querySelector(".argument-field textarea") ??
      document.querySelector(".inspector-panel textarea"),
    "Hello",
    "input",
  );
  window.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "ы",
      code: "KeyS",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  await wait(
    () => !document.querySelector(".draft-state.modified"),
    "save shortcut with Russian keyboard layout",
  );
  // Two independently configured instances sharing the same action endpoint.
  await click("Подключения плагинов");
  const manager = document.querySelector(".instance-manager");
  const template = field("Плагин и конфигурация", manager);
  await set(template, template.options[1].value);
  for (const [name, prefix] of [
    ["Work", "W:"],
    ["Home", "H:"],
  ]) {
    await set(field("Имя нового подключения", manager), name, "input");
    await click("Создать подключение", manager);
    await set(manager.querySelector("textarea"), prefix, "input");
  }
  await click("Закрыть", manager);
  const choose = async (name) => {
    const select = field("Подключение");
    await set(
      select,
      [...select.options].find((o) => o.textContent === name).value,
    );
  };
  await choose("Work");
  await drag(
    document.querySelector(".library-action"),
    document.querySelector(".binding-drop-zone"),
  );
  assert(
    document.querySelectorAll(".action-chain li").length === 2,
    "library to inspector drag",
  );
  await choose("Home");
  const source = document.querySelector(".argument-source select");
  const result = [...source.options].find((o) =>
    o.value.includes("run_result"),
  );
  assert(result, "typed previous result option");
  await set(source, result.value);
  const check = [...document.querySelectorAll("label")]
    .find((l) => l.textContent.includes("Показать результат на этой кнопке"))
    .querySelector("input");
  check.click();
  await sleep();
  // Copy then move uses actual component drop targets, retaining typed result references.
  await drag(key("key-1"), key("key-2"));
  assert(key("key-2").classList.contains("assigned"), "control copy");
  await drag(key("key-2"), key("key-3"), true);
  assert(
    !key("key-2").classList.contains("assigned") &&
      key("key-3").classList.contains("assigned"),
    "control move",
  );
  key("key-1").click();
  await sleep();
  await click("Применить действия");
  const review = await wait(
    () => document.querySelector(".apply-review[open]"),
    "review",
  );
  await wait(
    () => button("Применить", review) && !button("Применить", review).disabled,
    "validated plan",
  );
  const bounds = review.getBoundingClientRect(),
    applyBounds = button("Применить", review).getBoundingClientRect();
  assert(
    bounds.left >= 0 &&
      bounds.right <= innerWidth &&
      bounds.top >= 0 &&
      bounds.bottom <= innerHeight &&
      applyBounds.bottom <= innerHeight,
    "review/primary action overflows at 760x520",
  );
  assert(
    review.scrollWidth <= review.clientWidth + 1,
    "review horizontal overflow",
  );
  const before = await invoke("app_query", {
    query: { type: "active_document" },
  });
  assert(
    before.document.graph.nodes.length === 0,
    "review must not execute apply",
  );
  await click("Применить", review);
  await wait(
    () => !document.querySelector(".apply-review[open]"),
    "apply completed",
  );
  await click("Проверить кнопки");
  await wait(() => !key("key-1").disabled, "live mode ready");
  const snapshot = await invoke("app_query", {
      query: { type: "editor_snapshot" },
    }),
    surface = await invoke("app_query", { query: { type: "virtual_surface" } }),
    active = await invoke("app_query", { query: { type: "active_document" } });
  const endpoint = active.document.graph.nodes.find(
    (n) =>
      n.kind.type === "source" &&
      n.kind.endpoint.port.capabilityId === "button",
  ).kind.endpoint;
  const epoch = await invoke("mutation_window");
  const base = {
    endpoint,
    expectedActive: snapshot.active,
    activationEpoch: surface.activationEpoch,
    sessionGeneration: snapshot.nodes[0].generation,
  };
  for (const [name, change, code] of [
    [
      "generation",
      { sessionGeneration: base.sessionGeneration + 100 },
      "revision_conflict",
    ],
    [
      "epoch",
      { activationEpoch: base.activationEpoch + 100 },
      "revision_conflict",
    ],
    [
      "endpoint",
      {
        endpoint: {
          ...endpoint,
          target: { ...endpoint.target, deviceIdentity: "missing" },
        },
      },
      "invalid_command",
    ],
  ]) {
    let denied = false;
    try {
      await invoke("app_command", {
        envelope: {
          requestId: "stale-" + name,
          idempotencyKey: "stale-" + name,
          dedupEpoch: epoch.dedupEpoch,
          dryRun: false,
          command: { type: "virtual_click", request: { ...base, ...change } },
        },
      });
    } catch (e) {
      denied = e.code === code;
    }
    assert(denied, "virtual fence " + name);
  }
  key("key-1").click();
  await wait(
    () => key("key-1").textContent.includes("H:W:Hello"),
    "actual plugin result and display readback",
  );
  const trace = await invoke("app_query", { query: { type: "runtime_trace" } });
  assert(
    !JSON.stringify(trace).includes("Hello"),
    "private result stays out of trace",
  );
  assert(
    trace.entries.some((e) => e.kind === "intent_feedback"),
    "feedback dispatch trace",
  );
  const draft = await invoke("open_workspace");
  assert(
    Object.keys(draft.document.pluginInstances).length === 2,
    "both configured instances persisted",
  );
  assert(
    Object.values(draft.document.graph.nodeInstances).length === 4,
    "copied instance ownership persisted",
  );
  await click("Вернуться к настройке");
  const oldTrace = await invoke("app_query", {query: {type: "runtime_trace"}});
  await click("Применять сразу");
  await set(field("Подпись на кнопке"), "Live label", "input");
  await wait(async () => {
    const value = await invoke("app_query", {query: {type: "active_document"}});
    return value.document.extensions["org.deckweave.editor"]?.devices[0]?.controls["key-1"]?.label === "Live label";
  }, "layout applied automatically");
  const retained = await invoke("app_query", {query: {type: "virtual_surface"}});
  assert(retained.activationEpoch === surface.activationEpoch, "layout retains live execution epoch");
  const retainedTrace = await invoke("app_query", {query: {type: "runtime_trace"}});
  assert(retainedTrace.entries.filter(e => e.kind === "intent_invoke").length === oldTrace.entries.filter(e => e.kind === "intent_invoke").length, "editing never invokes an action");
  assert(!document.querySelector(".apply-review[open]"), "live edit has no mandatory review");
  await click("Проверить кнопки");
  await wait(() => !key("key-1").disabled, "live controls follow retained-epoch revision");
  key("key-1").click();
  await wait(async () => {
    const current = await invoke("app_query", {query: {type: "runtime_trace"}});
    return current.entries.filter(e => e.kind === "intent_invoke").length === retainedTrace.entries.filter(e => e.kind === "intent_invoke").length + 2;
  }, "explicit click after live edit executes once");
  await click("Вернуться к настройке");
  // Subsequent stress setup mutates the repository independently of this UI.
  await click("Применять сразу");
  // UI stays usable after real execution; theme and accent survive a component reload.
  await click("Настройки приложения");
  assert(
    field("Тема").value === "dark" &&
      field("Акцентный цвет").value === "#8661c5",
    "appearance restored",
  );
  await set(field("Тема"), "light");
  await click("Закрыть", document.querySelector(".settings-popover"));
  return "native component DnD, two configured instances, typed result chain, review, live editing without runtime restart or action replay, virtual click, isolated plugin and display readback";
};
