<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    native,
    getSnapshot,
    getAutostart,
    setAutostart,
    exitApp,
    onHostChanged,
    type HostSnapshot,
  } from "./bridge";
  import { desktopClient } from "./editor/api";
  import { EditorSession } from "./editor/session.svelte";
  import { browserJournal } from "./editor/indexed-journal";
  import Library from "./components/Library.svelte";
  import { LiveControls } from "./editor/live.svelte";
  import WorkspaceCanvas from "./components/WorkspaceCanvas.svelte";
  import Inspector from "./components/Inspector.svelte";
  import DevicePicker from "./components/DevicePicker.svelte";
  import InstanceManager from "./components/InstanceManager.svelte";
  import ApplyReview from "./components/ApplyReview.svelte";
  import LiveEditingControls from "./components/LiveEditingControls.svelte";
  import ThemeSettings from "./components/ThemeSettings.svelte";
  import PermissionSettings from "./components/PermissionSettings.svelte";
  import Icon from "./components/Icon.svelte";
  import RuntimeStatus from "./components/RuntimeStatus.svelte";
  let session = $state(new EditorSession(desktopClient));
  let live = $derived(new LiveControls(session));
  let host = $state<HostSnapshot | null>(null);
  let ready = $state(!native),
    adding = $state(false),
    settings = $state(false),
    connections = $state(false),
    permissions = $state(false),
    startup = $state(false);
  const t = (ru: string, en: string) => session.t(ru, en);
  async function connect() {
    session.liveEditing.dispose();
    session = new EditorSession(desktopClient);
    session.journal = browserJournal(localStorage);
    await session.load();
    session.liveEditing.start();
    try {
      host = await getSnapshot();
      if (host.mode === "installed") startup = await getAutostart();
    } catch (cause) {
      session.error = String(cause);
    }
    ready = true;
  }
  async function example() {
    const { previewClient } = await import("./editor/preview");
    session.liveEditing.dispose();
    session = new EditorSession(previewClient(), true);
    await session.load();
    session.liveEditing.start();
    adding = false;
  }
  async function toggleStartup() {
    await session.run(async () => {
      await setAutostart(!startup);
      startup = await getAutostart();
    });
  }
  function keyboard(e: KeyboardEvent) {
    const input = ["INPUT", "TEXTAREA", "SELECT"].includes(
      (e.target as HTMLElement)?.tagName,
    );
    if (
      (e.ctrlKey || e.metaKey) &&
      (e.code === "KeyS" || e.key.toLowerCase() === "s")
    ) {
      e.preventDefault();
      void session.run(() => session.save());
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      if (session.liveEnabled) session.liveEditing.resume();
      else void session.apply();
    }
    if (
      (e.ctrlKey || e.metaKey) &&
      (e.code === "KeyZ" || e.key.toLowerCase() === "z") &&
      !input
    ) {
      e.preventDefault();
      session.history(e.shiftKey ? "redo" : "undo");
    }
    if (e.key === "/" && !input) {
      e.preventDefault();
      document.querySelector<HTMLInputElement>("#action-search")?.focus();
    }
    if (e.key === "Escape") settings = false;
  }
  onMount(() => {
    if (!native) return;
    let disposed = false,
      off: (() => void) | undefined;
    void connect()
      .then(async () => {
        const unsubscribe = await onHostChanged(() => {
          void getSnapshot().then((value) => (host = value));
        });
        if (disposed) unsubscribe();
        else off = unsubscribe;
      })
      .catch((cause) => {
        session.error = String(cause);
        ready = true;
      });
    return () => {
      disposed = true;
      session.liveEditing.dispose();
      off?.();
    };
  });
  $effect(() => {
    document.documentElement.lang = session.lang;
  });
</script>

<svelte:head
  ><title>DeckWeave — {t("Рабочее пространство", "Workspace")}</title
  ></svelte:head
>
<svelte:window
  onkeydown={keyboard}
  onbeforeunload={(e) => {
    if ((session.dirty || session.pending) && !session.journal) {
      e.preventDefault();
      e.returnValue = "";
    }
  }}
/>
<div class="shell">
  <header class="app-header">
    <div class="brand">
      <img src="/deckweave.svg" alt="" /><strong>DeckWeave</strong>
    </div>
    <span class="header-divider"></span><span class="workspace-name"
      >{t("Основное", "Main")}</span
    >
    <span class="draft-state" class:modified={session.dirty}
      ><i></i>{!session.draft
        ? t("Нет черновика", "No draft")
        : session.liveUpdating
          ? t("Применение…", "Applying…")
          : session.pending
            ? t("Результат неизвестен", "Outcome unknown")
            : session.liveStatus === "error"
              ? t("Не применено", "Not applied")
              : session.dirty || session.liveStatus === "queued"
                ? t("Есть изменения", "Changes pending")
                : session.liveEnabled &&
                    session.operation?.state === "succeeded" &&
                    session.operation.runtimeHealth === "applied"
                  ? t("Применено", "Applied")
                  : t("Черновик сохранён", "Draft saved")}</span
    >
    <div class="header-tools">
      <button
        class="icon-button"
        aria-label={t("Подключения плагинов", "Plugin connections")}
        disabled={!session.document}
        onclick={() => (connections = true)}><Icon name="layers" /></button
      >
      <button
        class="icon-button"
        title={t("Отменить · Ctrl+Z", "Undo · Ctrl+Z")}
        aria-label={t("Отменить", "Undo")}
        disabled={!session.undoStack.length || session.editLocked}
        onclick={() => session.history("undo")}><Icon name="undo" /></button
      >
      <button
        class="icon-button"
        title={t("Повторить · Ctrl+Shift+Z", "Redo · Ctrl+Shift+Z")}
        aria-label={t("Повторить", "Redo")}
        disabled={!session.redoStack.length || session.editLocked}
        onclick={() => session.history("redo")}><Icon name="redo" /></button
      >
      <LiveEditingControls {session} />
      <button
        class="icon-button"
        aria-label={t("Настройки приложения", "Application settings")}
        aria-expanded={settings}
        onclick={() => (settings = !settings)}><Icon name="settings" /></button
      >
    </div>
  </header>
  {#if session.preview}<div class="preview-bar">
      <span
        >{t(
          "Учебный пример · без оборудования · изменения хранятся до закрытия",
          "Practice workspace · no hardware · changes last until you close it",
        )}</span
      >{#if native}<button class="text-button" onclick={connect}
          >{t("Вернуться к устройствам", "Return to devices")}</button
        >{/if}
    </div>{/if}
  {#if session.error || (session.pending && !session.busy) || session.conflict}<div
      class="error-bar"
      role="alert"
    >
      <Icon name="alert" size={17} /><span
        >{session.error ||
          session.notice ||
          t(
            "Черновик изменён вне редактора.",
            "The draft changed outside this editor.",
          )}</span
      >{#if session.conflict && !session.pending}<button
          class="secondary"
          disabled={session.busy}
          onclick={() => session.mergeLatest()}
          >{t("Обновить и объединить", "Refresh and merge")}</button
        >{/if}{#if session.pending}<button
          class="secondary"
          disabled={session.busy}
          onclick={() => session.retry()}
          >{t("Повторить тот же запрос", "Retry same request")}</button
        >{/if}{#if native && !session.preview && !session.document && !session.pending}<button
          class="secondary"
          disabled={session.busy}
          onclick={connect}>{t("Подключиться повторно", "Reconnect")}</button
        >{/if}
    </div>{/if}
  <div class="workspace-body">
    <Library {session} onadd={() => (adding = true)} />
    <main class="canvas-container">
      <WorkspaceCanvas {session} {live} onadd={() => (adding = true)} />
      {#if !native && !session.preview}<div class="browser-welcome">
          <div class="welcome-mark"><Icon name="grid" size={34} /></div>
          <h1>
            {t("Ваши устройства. Ваш порядок.", "Your devices. Your way.")}
          </h1>
          <p>
            {t(
              "Настраивайте кнопки, собирайте действия и задавайте общий стиль. Подключение оборудования доступно в desktop-приложении.",
              "Configure controls, connect actions and create a shared style. Hardware connections are available in the desktop app.",
            )}
          </p>
          <button class="primary" onclick={example}
            >{t("Попробовать на примере", "Explore an example")}<Icon
              name="arrow"
              size={16}
            /></button
          ><small
            >{t(
              "Можно попробовать без подключения устройств",
              "No device needed",
            )}</small
          >
        </div>{/if}
      {#if !ready}<div class="loading-overlay">
          {t("Открываем рабочее пространство…", "Opening workspace…")}
        </div>{/if}
    </main>
    <Inspector {session} />
  </div>
  <footer class="status-bar">
    <span class="host-state"
      ><i class:online={host?.lifecycle === "running" || session.preview}
      ></i>{session.preview
        ? t("Учебный пример", "Practice mode")
        : host?.lifecycle === "running"
          ? t("Host работает", "Host running")
          : t("Нет подключения", "Disconnected")}</span
    ><span
      >{t("Устройства", "Devices")}: {session.devices.length} · {t(
        "Плагины",
        "Plugins",
      )}: {session.catalogs.length}</span
    ><RuntimeStatus {session} /><span class="status-notice" role="status"
      >{session.notice}</span
    ><button
      class="text-button"
      title={t("Обновить состояние устройств", "Refresh device status")}
      disabled={!session.document || session.busy}
      onclick={() => session.run(() => session.refresh())}
      ><Icon name="refresh" size={13} /></button
    ><button
      class="text-button language"
      disabled={!session.document || session.busy}
      onclick={() => session.language(session.lang === "ru" ? "en" : "ru")}
      >{session.lang.toUpperCase()}<Icon name="chevron" size={12} /></button
    >
  </footer>
  {#if settings}<section
      class="settings-popover"
      aria-label={t("Настройки приложения", "Application settings")}
    >
      <div class="section-heading">
        <h2>{t("Приложение", "Application")}</h2>
        <button
          class="icon-button"
          aria-label={t("Закрыть", "Close")}
          onclick={() => (settings = false)}><Icon name="close" /></button
        >
      </div>
      <ThemeSettings lang={session.lang} />
      {#if native && !session.preview}<button
          class="secondary full-width"
          onclick={() => {
            settings = false;
            permissions = true;
          }}>{t("Разрешения плагинов", "Plugin permissions")}</button
        >{/if}
      <div class="section-divider"></div>
      <label class="check-row"
        ><input
          type="checkbox"
          checked={startup}
          disabled={!host || host.mode === "portable" || session.busy}
          onchange={toggleStartup}
        />{t("Запускать при входе", "Start at login")}</label
      >{#if host?.mode === "portable"}<p class="help">
          {t(
            "В portable-режиме автозапуск недоступен.",
            "Autostart is unavailable in portable mode.",
          )}
        </p>{/if}
      <div class="section-divider"></div>
      <label class="field-label" for="data-root"
        >{t("Каталог данных", "Data directory")}</label
      ><output id="data-root" class="root-path">{host?.dataRoot ?? "—"}</output>
      <p class="help">
        {t(
          "Закрытие окна оставляет автоматизации работающими в трее.",
          "Closing the window keeps automations running in the tray.",
        )}
      </p>
      <button
        class="secondary full-width"
        disabled={!host || session.dirty || Boolean(session.pending)}
        onclick={() => session.run(exitApp)}
        >{t("Выход из DeckWeave", "Exit DeckWeave")}</button
      >{#if session.dirty}<p class="help">
          {t(
            "Перед выходом сохраните черновик.",
            "Save your draft before exiting.",
          )}
        </p>{/if}
    </section>{/if}
</div>
<DevicePicker
  {session}
  open={adding}
  onclose={() => (adding = false)}
  onexample={example}
/>

<InstanceManager
  {session}
  open={connections}
  onclose={() => (connections = false)}
/>
<ApplyReview {session} />
<PermissionSettings
  open={permissions}
  lang={session.lang}
  onclose={() => (permissions = false)}
  onchanged={() => {
    session.notice = t(
      "Разрешения сохранены. Примените действия заново.",
      "Permissions saved. Apply your actions again.",
    );
    void session.run(() => session.refresh());
  }}
/>
