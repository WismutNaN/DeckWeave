<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { equal } from "../editor/bindings";
  import Icon from "./Icon.svelte";
  import type { EditorSession } from "../editor/session.svelte";
  let { session }: { session: EditorSession } = $props();
  let open = $state(false);
  const t = (ru: string, en: string) => session.t(ru, en);
  let current = $derived(
    Boolean(
      session.operation?.intent &&
      equal(session.operation.intent.candidate, session.snapshot?.active),
    ),
  );
  let degraded = $derived(
    (current && session.operation?.runtimeHealth === "degraded") ||
      session.snapshot?.runtimes.some((r) =>
        ["degraded", "faulted", "failed"].includes(r.status),
      ),
  );
  let label = $derived(
    session.preview
      ? t("Учебный режим", "Practice mode")
      : degraded
        ? t("Требует внимания", "Needs attention")
        : current && session.operation?.runtimeHealth === "applied"
          ? t("Применено", "Applied")
          : session.snapshot?.active
            ? t("Есть активная конфигурация", "Active configuration")
            : t("Не применено", "Not applied"),
  );
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") open = false;
  }}
/>
<div class="runtime-summary">
  <button
    class="text-button"
    class:invalid={degraded}
    aria-expanded={open}
    onclick={() => (open = !open)}
    ><Icon name={degraded ? "alert" : "layers"} size={13} />{label}</button
  >
  {#if open}<section
      class="runtime-popover"
      aria-label={t("Состояние выполнения", "Runtime status")}
    >
      <div class="section-heading">
        <h2>{t("Выполнение", "Runtime")}</h2>
        <button
          class="icon-button"
          aria-label={t("Закрыть", "Close")}
          onclick={() => (open = false)}><Icon name="close" /></button
        >
      </div>
      <p>{label}</p>
      <dl>
        <dt>{t("Черновик", "Draft")}</dt>
        <dd>
          v{session.draft?.version ?? "—"}{session.dirty
            ? " · " + t("изменён", "modified")
            : ""}
        </dd>
        <dt>{t("Активная версия", "Active revision")}</dt>
        <dd>{session.snapshot?.active?.revisionId ?? "—"}</dd>
        {#if session.operation}<dt>{t("Применение", "Apply")}</dt>
          <dd>
            {session.operation.state} · {session.operation.runtimeHealth}
          </dd>{/if}
      </dl>
      {#each session.snapshot?.runtimes ?? [] as runtime}<div
          class="runtime-row"
        >
          <span>{runtime.packageId}</span><strong>{runtime.status}</strong>
        </div>{/each}
      <p class="help">
        {t(
          "Сохранение черновика не меняет работающие действия. Состояние устройств обновляется отдельным запросом.",
          "Saving a draft does not change running actions. Device status is refreshed separately.",
        )}
      </p>
      <button
        class="secondary full-width"
        disabled={session.busy}
        onclick={() => session.run(() => session.refresh())}
        >{t("Обновить состояние", "Refresh status")}</button
      >
    </section>{/if}
</div>
