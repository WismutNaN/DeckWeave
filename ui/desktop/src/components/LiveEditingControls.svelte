<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { EditorSession } from "../editor/session.svelte";
  let { session }: { session: EditorSession } = $props();
  const t = (ru: string, en: string) => session.t(ru, en);
</script>

<button
  class="secondary live-edit-toggle"
  class:enabled={session.liveEnabled}
  aria-pressed={session.liveEnabled}
  disabled={!session.document}
  title={t(
    "Сохранять и применять правки автоматически. Пауза останавливает следующие обновления.",
    "Save and apply edits automatically. Pausing stops subsequent updates.",
  )}
  onclick={() =>
    session.liveEnabled
      ? session.liveEditing.pause()
      : session.liveEditing.resume()}
  >{t("Применять сразу", "Apply as you edit")}</button
>
{#if !session.liveEnabled}
  <button
    class="secondary save-button"
    disabled={!session.dirty || session.busy || Boolean(session.pending)}
    onclick={() => session.run(() => session.save())}
    >{t("Сохранить", "Save")}</button
  >
  <button
    class="primary"
    disabled={!session.document || session.busy || Boolean(session.pending)}
    onclick={() => session.apply()}
    >{session.preview
      ? t("Сохранить пример", "Save example")
      : t("Применить действия", "Apply actions")}</button
  >
{:else if session.liveStatus === "error" && !session.pending && !session.conflict}
  <button
    class="secondary"
    disabled={session.busy}
    onclick={() => session.liveEditing.resume()}
    >{t("Повторить применение", "Retry apply")}</button
  >
{/if}

<style>
  .enabled {
    background: var(--soft);
    color: var(--accent);
    border-color: var(--accent);
  }
  .live-edit-toggle {
    white-space: nowrap;
  }
</style>
