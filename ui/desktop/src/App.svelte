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

  let host = $state<HostSnapshot | null>(null);
  let error = $state("");
  let startupError = $state("");
  let startup = $state(false);
  let busy = $state(false);
  let lang = $state<"ru" | "en">("ru");
  const t = (ru: string, en: string) => (lang === "ru" ? ru : en);

  async function refresh() {
    try {
      host = await getSnapshot();
      error = "";
    } catch (cause) {
      error = String(cause);
    }
  }
  async function toggleStartup() {
    busy = true;
    try {
      await setAutostart(!startup);
      startup = await getAutostart();
      startupError = "";
    } catch (cause) {
      startupError = String(cause);
    } finally {
      busy = false;
    }
  }
  async function exit() {
    try {
      await exitApp();
    } catch (cause) {
      error = String(cause);
    }
  }
  onMount(() => {
    if (!native) return;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void (async () => {
      await refresh();
      if (host?.mode === "installed") {
        try {
          startup = await getAutostart();
        } catch (cause) {
          startupError = String(cause);
        }
      }
      const off = await onHostChanged(() => {
        void refresh();
      });
      if (disposed) off();
      else unsubscribe = off;
    })().catch((cause) => {
      error = String(cause);
    });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  });
</script>

<svelte:head><title>DeckWeave</title></svelte:head>

<div class="shell">
  <header>
    <div class="brand">
      <img src="/deckweave.svg" alt="" /><strong>DeckWeave</strong><span
        class="version">0.1 · foundation</span
      >
    </div>
    <div class="header-tools">
      <span class="status" class:online={host?.lifecycle === "running"}
        ><i></i>{host
          ? t("Host работает", "Host running")
          : native
            ? t("Подключение…", "Connecting…")
            : t("Предпросмотр UI", "UI preview")}</span
      >
      <button
        class="language"
        onclick={() => {
          lang = lang === "ru" ? "en" : "ru";
          document.documentElement.lang = lang;
        }}>{lang === "ru" ? "EN" : "RU"}</button
      >
    </div>
  </header>
  <div class="body">
    <aside class="navigation">
      <div class="section-label">{t("РАБОЧЕЕ ПРОСТРАНСТВО", "WORKSPACE")}</div>
      <div class="nav-current">
        <span class="grid-icon">▦</span>{t("Обзор", "Overview")}<span
          class="count">0</span
        >
      </div>
      <div class="nav-future">
        {t("Устройства", "Devices")}<span>{t("Позже", "Later")}</span>
      </div>
      <div class="nav-future">
        {t("Автоматизации", "Automations")}<span>{t("Позже", "Later")}</span>
      </div>
      <div class="nav-future">
        {t("Плагины", "Plugins")}<span>{t("Позже", "Later")}</span>
      </div>
      <div class="sidebar-note">
        <span class="tiny-square"></span>{t("Фаза 1", "Phase 1")}
        <p>
          {t(
            "Оболочка и жизненный цикл приложения.",
            "Application shell and lifecycle.",
          )}
        </p>
      </div>
    </aside>
    <main>
      <div class="page-heading">
        <div>
          <div class="eyebrow">DECKWEAVE / 01</div>
          <h1>{t("Рабочее пространство", "Workspace")}</h1>
        </div>
        <span class="outline-tag"
          >{t("Скелет приложения", "Application scaffold")}</span
        >
      </div>
      {#if !native}<div class="notice">
          {t(
            "В браузере доступен только интерфейс. Запустите desktop-приложение для связи с host.",
            "This browser shows the interface only. Launch the desktop app to connect to the host.",
          )}
        </div>{/if}
      {#if error}<div class="notice error" role="alert">{error}</div>{/if}
      {#if host?.notice}<div class="notice" role="alert">
          {host.notice}
        </div>{/if}
      <section
        class="canvas"
        aria-label={t(
          "Будущее рабочее пространство устройств",
          "Future device workspace",
        )}
      >
        <div class="canvas-coordinate">0, 0</div>
        <div class="empty-state">
          <div class="device-outline" aria-hidden="true">
            <div class="keys">
              {#each Array(10) as _}<span></span>{/each}
            </div>
            <div class="touch-strip"></div>
            <div class="knobs">
              {#each Array(4) as _}<span></span>{/each}
            </div>
          </div>
          <h2>{t("Место для ваших устройств", "A place for your devices")}</h2>
          <p>
            {t(
              "Здесь появятся панели, клавиатуры и другие устройства. Первый этап готовит приложение к их подключению.",
              "Panels, keyboards and other devices will appear here. This first stage prepares the application for them.",
            )}
          </p>
          <div class="phase-tag">
            <span></span>{t(
              "Подключение оборудования — следующие фазы",
              "Hardware support comes in later phases",
            )}
          </div>
        </div>
        <div class="canvas-caption">
          {t("Оборудование пока не используется", "No hardware is in use")}
        </div>
      </section>
    </main>
    <aside class="inspector">
      <div class="section-label">{t("ПРИЛОЖЕНИЕ", "APPLICATION")}</div>
      <h2>{t("Состояние host", "Host status")}</h2>
      <dl>
        <dt>{t("Режим", "Mode")}</dt>
        <dd>
          {host?.mode === "portable" ? "Portable" : host ? "Installed" : "—"}
        </dd>
        <dt>{t("Версия", "Version")}</dt>
        <dd>{host?.version ?? "—"}</dd>
      </dl>
      <div class="divider"></div>
      <label class="field-label" for="data-root"
        >{t("Каталог данных", "Data directory")}</label
      >
      <output id="data-root" class="root-path">{host?.dataRoot ?? "—"}</output>
      <p class="help">
        {t(
          "Каталог защищён от одновременной записи другим экземпляром.",
          "The directory is protected against concurrent writes by another instance.",
        )}
      </p>
      <div class="divider"></div>
      <div class="startup-row">
        <span>{t("Запуск при входе", "Start at login")}</span><button
          class="toggle"
          class:enabled={startup}
          role="switch"
          aria-checked={startup}
          aria-label={t("Запуск при входе", "Start at login")}
          disabled={!host || host.mode === "portable" || busy}
          onclick={toggleStartup}><span></span></button
        >
      </div>
      <p class="help">
        {host?.mode === "portable"
          ? t(
              "В portable-режиме автозапуск недоступен.",
              "Autostart is unavailable in portable mode.",
            )
          : t(
              "Включается только по вашему выбору.",
              "Enabled only when you choose to.",
            )}
      </p>
      {#if startupError}<p class="inline-error" role="alert">
          {startupError}
        </p>{/if}
      <div class="tray-note">
        <strong
          >{t(
            "Приложение остаётся в трее",
            "The app stays in the tray",
          )}</strong
        >
        <p>
          {t(
            "Закрытие окна сохраняет host работающим. Для завершения используйте «Выход».",
            "Closing this window keeps the host running. Use Exit to shut it down.",
          )}
        </p>
      </div>
      <button class="exit" disabled={!host} onclick={exit}
        >{t("Выход из DeckWeave", "Exit DeckWeave")}</button
      >
    </aside>
  </div>
  <footer>
    <span
      ><i class="footer-dot"></i>{t(
        "0 устройств · 0 плагинов",
        "0 devices · 0 plugins",
      )}</span
    ><span>{t("Rust + Tauri + Svelte", "Rust + Tauri + Svelte")}</span>
  </footer>
</div>
