<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { message } from "../editor/api";
  type Selection = {
    packageId: string;
    manifestSha256: string;
    permissionIndex: number;
  };
  type Offer = {
    selection: Selection;
    runtimeGroupIds: string[];
    request: {
      scope: string;
      reason: string;
      accessMode: string;
      resourceSelector: unknown;
    };
  };
  type Snapshot = {
    preferences: { revision: number; selected: Selection[] };
    offers: Offer[];
  };
  let {
    open,
    lang,
    onclose,
    onchanged,
  }: {
    open: boolean;
    lang: string;
    onclose: () => void;
    onchanged: () => void;
  } = $props();
  let dialog = $state<HTMLDialogElement>(),
    snapshot = $state<Snapshot>(),
    selected = $state<string[]>([]),
    busy = $state(false),
    error = $state("");
  const t = (ru: string, en: string) => (lang === "ru" ? ru : en);
  const key = (s: Selection) =>
    JSON.stringify([s.packageId, s.manifestSha256, s.permissionIndex]);
  const adopt = (value: Snapshot) => {
    snapshot = value;
    selected = value.preferences.selected.map(key);
  };
  async function load() {
    busy = true;
    error = "";
    try {
      adopt(await invoke<Snapshot>("permissions_get"));
    } catch (cause) {
      error = message(cause, lang);
      snapshot = undefined;
    } finally {
      busy = false;
    }
  }
  async function save() {
    if (!snapshot || busy) return;
    busy = true;
    error = "";
    try {
      adopt(
        await invoke<Snapshot>("permissions_change", {
          expected: snapshot.preferences.revision,
          selected: snapshot.offers
            .filter((o) => selected.includes(key(o.selection)))
            .map((o) => o.selection),
        }),
      );
      onchanged();
      onclose();
    } catch (cause) {
      // Never retry a stale selection automatically after a conflict or uncertain response.
      const failure = message(cause, lang);
      try {
        adopt(await invoke<Snapshot>("permissions_get"));
      } catch {
        snapshot = undefined;
      }
      error = failure;
    } finally {
      busy = false;
    }
  }
  let groups = $derived([
    ...new Set(snapshot?.offers.map((o) => o.selection.packageId) ?? []),
  ]);
  const scopes: Record<string, [string, string]> = {
    "windows.audio.observe": [
      "Читать громкость и состояние звука",
      "Read volume and audio state",
    ],
    "windows.audio.control": [
      "Менять громкость и отключать звук",
      "Change volume and mute",
    ],
    "windows.window.observe": [
      "Читать список окон и их заголовки",
      "Read windows and their titles",
    ],
    "windows.window.activate": [
      "Переключать активное окно",
      "Switch the active window",
    ],
    "windows.process.launch": [
      "Запускать программы с аргументами",
      "Launch programs with arguments",
    ],
    "windows.shell.open": ["Открывать файлы и ссылки", "Open files and links"],
    "windows.media.control": [
      "Управлять воспроизведением",
      "Control media playback",
    ],
    "input.observe": [
      "Наблюдать нажатия клавиш во всех приложениях",
      "Observe key presses across applications",
    ],
  };
  function label(offer: Offer) {
    const scope = offer.request.scope.replace(/^org\.deckweave\./, "");
    return scopes[scope]?.[lang === "ru" ? 0 : 1] ?? offer.request.reason;
  }
  $effect(() => {
    if (open && !dialog?.open) {
      dialog?.showModal();
      void load();
    } else if (!open && dialog?.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  {onclose}
  oncancel={(e) => {
    if (busy) e.preventDefault();
  }}
  aria-labelledby="permissions-title"
>
  <div class="dialog-heading">
    <h2 id="permissions-title">
      {t("Разрешения плагинов", "Plugin permissions")}
    </h2>
    <button
      class="icon-button"
      disabled={busy}
      aria-label={t("Закрыть", "Close")}
      onclick={onclose}>×</button
    >
  </div>
  <div class="content">
    <p class="help">
      {t(
        "Разрешения действуют для указанной версии пакета. Установка плагина сама по себе не даёт доступа к системе.",
        "Permissions apply to the listed package version. Installing a plugin does not grant system access.",
      )}
    </p>
    <p class="help">
      {t(
        "Функции одной группы запускаются вместе и требуют всех перечисленных разрешений этой группы. Наблюдение за клавиатурой включается отдельно.",
        "Functions in a runtime group start together and require all permissions listed for that group. Keyboard observation is enabled separately.",
      )}
    </p>
    {#each groups as packageId}
      <fieldset disabled={busy}>
        <legend>{packageId}</legend>
        {#each snapshot?.offers.filter((o) => o.selection.packageId === packageId) ?? [] as offer}
          <label class="permission">
            <input
              type="checkbox"
              value={key(offer.selection)}
              bind:group={selected}
            />
            <span
              ><strong>{label(offer)}</strong><small
                >{t("Группа", "Group")}: {offer.runtimeGroupIds.join(", ") ||
                  t("Все", "All")}</small
              ></span
            >
          </label>
          <details>
            <summary>{t("Область доступа", "Access scope")}</summary><code
              >{offer.request.scope} · {offer.request.accessMode}</code
            >
            <pre>{JSON.stringify(offer.request.resourceSelector, null, 2)}</pre>
          </details>
        {/each}
      </fieldset>
    {/each}
    {#if snapshot && !groups.length}<p>
        {t(
          "Установленные плагины не запрашивают разрешений.",
          "Installed plugins do not request permissions.",
        )}
      </p>{/if}
    {#if error}<p role="alert" class="invalid">{error}</p>{/if}
    {#if busy}<p role="status">{t("Подождите…", "Working…")}</p>{/if}
    <p class="help">
      {t(
        "После сохранения заново примените действия. Отзыв доступа останавливает связанные вызовы; уже выполненные действия не отменяются.",
        "Apply your actions again after saving. Revoking access stops related calls; completed effects are not undone.",
      )}
    </p>
  </div>
  <footer>
    <button class="secondary" disabled={busy} onclick={load}
      >{t("Обновить", "Refresh")}</button
    ><button class="primary" disabled={busy || !snapshot} onclick={save}
      >{t("Сохранить разрешения", "Save permissions")}</button
    >
  </footer>
</dialog>

<style>
  dialog {
    width: min(620px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    padding: 0;
  }
  .dialog-heading,
  footer {
    padding: 18px 22px;
  }
  .content {
    padding: 0 22px 16px;
    max-height: calc(100dvh - 200px);
    overflow: auto;
  }
  fieldset {
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 12px 16px;
    margin: 20px 0;
    min-width: 0;
  }
  legend {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .permission {
    display: flex;
    gap: 12px;
    align-items: start;
    padding-top: 12px;
    cursor: pointer;
  }
  .permission input {
    margin-top: 3px;
    flex: 0 0 auto;
  }
  strong {
    display: block;
    font-size: 13px;
    font-weight: 500;
  }
  small {
    display: block;
    margin-top: 4px;
    opacity: 0.7;
  }
  details {
    margin: 6px 0 10px 26px;
    font-size: 11px;
  }
  summary {
    cursor: pointer;
    opacity: 0.7;
  }
  pre,
  code {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  footer {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    border-top: 1px solid var(--line);
  }
</style>
