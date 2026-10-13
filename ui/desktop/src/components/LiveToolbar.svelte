<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { LiveControls } from "../editor/live.svelte";
  let { live }: { live: LiveControls } = $props();
  const t = (ru: string, en: string) => live.session.t(ru, en);
  $effect(() => {
    if (!live.enabled) return;
    let stopped = false,
      timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (!document.hidden)
        try {
          await live.refresh();
        } catch {
          live.error = t(
            "Нет связи с виртуальным устройством.",
            "Virtual device is unavailable.",
          );
        }
      if (!stopped) timer = setTimeout(poll, 500);
    };
    timer = setTimeout(poll, 500);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });
</script>

<div class="live-toolbar">
  <button
    class:active={live.enabled}
    class="secondary"
    aria-pressed={live.enabled}
    disabled={live.pending || live.session.preview || !live.session.document}
    onclick={() => live.toggle()}
    >{live.enabled
      ? t("Вернуться к настройке", "Back to editing")
      : t("Проверить кнопки", "Test controls")}</button
  >
  {#if live.enabled}<span class="help"
      >{live.ready
        ? t(
            "Нажатия запускают применённые действия.",
            "Clicks run your applied actions.",
          )
        : t("Сначала примените изменения.", "Apply your changes first.")}</span
    >{/if}
  {#if live.error}<span class="invalid" role="alert">{live.error}</span
    >{:else if live.enabled && live.notice}<span class="help" role="status"
      >{live.notice}</span
    >{/if}
</div>
