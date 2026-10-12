<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import { actionShape } from "../editor/bindings";
  import { writeActionDrag, writeSelectionDrag } from "../editor/drag";
  import type { EditorSession } from "../editor/session.svelte";
  let { session, onadd }: { session: EditorSession; onadd: () => void } =
    $props();
  let search = $state("");
  let dragging = $state("");
  const t = (ru: string, en: string) => session.t(ru, en);
  let filtered = $derived(
    session.actions.filter((a) =>
      `${a.title} ${a.catalog.package.packageId}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
</script>

<aside
  class="library-panel"
  aria-label={t("Устройства и действия", "Devices and actions")}
>
  <div class="panel-heading">
    <h2>{t("Устройства", "Devices")}</h2>
    <button
      class="icon-button"
      aria-label={t("Добавить устройство", "Add device")}
      onclick={onadd}><Icon name="plus" size={17} /></button
    >
  </div>
  <nav class="device-list" aria-label={t("Мои устройства", "My devices")}>
    {#each session.devices as device}<button
        draggable={!session.busy && !session.pending}
        ondragstart={(e) => {
          if (e.dataTransfer)
            writeSelectionDrag(e.dataTransfer, {
              kind: "device",
              deviceId: device.id,
              controlId: "",
            });
        }}
        class:active={session.selectedDevice === device.id}
        onclick={() => session.select(device.id)}
        ><Icon name="device" size={17} /><span>{device.descriptor.label}</span
        ><span class="count-badge">{device.descriptor.elements.length}</span
        ></button
      >{:else}<p class="library-help">
        {t("Здесь будут ваши устройства", "Your devices will appear here")}
      </p>{/each}
  </nav>
  <div class="library-divider"></div>
  <div class="panel-heading">
    <h2>{t("Действия", "Actions")}</h2>
    <span class="count-badge">{session.actions.length}</span>
  </div>
  <label class="search-field"
    ><Icon name="search" size={16} /><input
      id="action-search"
      aria-label={t("Найти действие", "Find an action")}
      placeholder={t("Найти действие…", "Find an action…")}
      bind:value={search}
    /><kbd>/</kbd></label
  >
  <div class="action-library">
    {#if session.ambiguousPackages.length}<p class="library-help invalid">
        {t(
          "Несколько версий пакета. Нужна зафиксированная версия в workspace:",
          "Multiple package versions. Pin a version in the workspace:",
        )}
        {session.ambiguousPackages.join(", ")}
      </p>{/if}
    {#each [...new Set(filtered.map((a) => a.catalog.package.packageId))] as packageId}<div
        class="library-group"
      >
        <span class="package-mark"><Icon name="layers" size={14} /></span><span
          >{session.preview
            ? t("Работа с текстом", "Text tools")
            : packageId.split(".").at(-1)}</span
        >
      </div>
      {#each filtered.filter((a) => a.catalog.package.packageId === packageId) as action}{@const supported =
          actionShape(action)}<button
          class="library-action"
          class:dragging={dragging === action.contributionId}
          draggable={Boolean(supported)}
          disabled={!supported || session.busy || Boolean(session.pending)}
          title={!supported
            ? t(
                "Схема пока не поддерживается инспектором",
                "Schema not supported by the inspector",
              )
            : !session.selectedControl
              ? t("Сначала выберите кнопку", "Select a key first")
              : t(
                  "Добавить действие к выбранной кнопке",
                  "Add action to selected key",
                )}
          onclick={() => session.assign(action)}
          ondragstart={(e) => {
            if (!e.dataTransfer || !supported) {
              e.preventDefault();
              return;
            }
            dragging = action.contributionId;
            writeActionDrag(e.dataTransfer, action.endpoint);
          }}
          ondragend={() => (dragging = "")}
          ><span class="action-symbol"><Icon name="text" size={17} /></span
          ><span>{action.title}</span><Icon name="plus" size={14} /></button
        >{/each}{/each}
    {#if !filtered.length}<div class="library-empty">
        <Icon name="search" size={21} />
        <p>
          {search
            ? t("Ничего не найдено", "No matching actions")
            : t(
                "Действия появятся после подключения плагинов",
                "Actions appear when plugins are available",
              )}
        </p>
        {#if search}<button class="text-button" onclick={() => (search = "")}
            >{t("Сбросить поиск", "Clear search")}</button
          >{/if}
      </div>{/if}
  </div>
  <div class="library-tip">
    <Icon name="arrow" size={16} />
    <p>
      {t(
        "Перетащите действие на кнопку или выберите кнопку и нажмите на действие.",
        "Drag an action onto a key, or select a key and click an action.",
      )}
    </p>
  </div>
</aside>
