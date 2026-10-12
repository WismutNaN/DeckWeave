<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { EditorSession } from "../editor/session.svelte";
  import { summarizeChanges, diagnosticMessage } from "../editor/review";
  let { session }: { session: EditorSession } = $props();
  let dialog = $state<HTMLDialogElement>();
  const t = (ru: string, en: string) => session.t(ru, en);
  $effect(() => {
    if (session.review && !dialog?.open) dialog?.showModal();
    else if (!session.review && dialog?.open) dialog.close();
  });
  let diff = $derived(
    session.review &&
      summarizeChanges(session.review.before, session.review.document),
  );
</script>

<dialog
  class="device-picker apply-review"
  bind:this={dialog}
  onclose={() => {
    session.review = null;
  }}
  oncancel={(e) => {
    if (session.busy) e.preventDefault();
  }}
>
  <div class="dialog-heading">
    <h2>{t("Проверка изменений", "Review changes")}</h2>
    <button
      class="icon-button"
      aria-label={t("Закрыть", "Close")}
      disabled={session.busy}
      onclick={() => (session.review = null)}>×</button
    >
  </div>
  {#if session.review && diff}<p class="help">
      {session.review.expectedActive
        ? t(
            "Изменения относительно работающей конфигурации.",
            "Changes compared with the running configuration.",
          )
        : t(
            "Первая активация этого рабочего пространства.",
            "First activation of this workspace.",
          )}
    </p>
    <dl class="review-diff">
      <dt>{t("Новые действия", "New actions")}</dt>
      <dd>{diff.added}</dd>
      <dt>{t("Изменённые действия", "Changed actions")}</dt>
      <dd>{diff.changed}</dd>
      <dt>{t("Удалённые действия", "Removed actions")}</dt>
      <dd>{diff.removed}</dd>
    </dl>
    {#if diff.routing}<p>
        {t(
          "Изменены связи между действиями.",
          "Action connections have changed.",
        )}
      </p>{/if}
    {#if diff.connections}<p>
        {t(
          "Изменены подключения и их настройки.",
          "Connections and their settings have changed.",
        )}
      </p>{/if}
    {#if diff.layout}<p>
        {t(
          "Изменён макет устройств и кнопок.",
          "The device and control layout has changed.",
        )}
      </p>{/if}
    {#if session.review.requiresDisruption}<label
        class="check-row disruption-consent"
        ><input
          type="checkbox"
          checked={session.review.consent}
          disabled={session.busy}
          onchange={(e) => {
            session.review!.consent = e.currentTarget.checked;
            void session.planReview();
          }}
        />{t(
          "Разрешаю прервать текущую работу затронутых устройств при переключении.",
          "Allow interrupting affected devices when switching configurations.",
        )}</label
      >{/if}
    {#each session.diagnostics as diagnostic}<p class="invalid">
        {diagnosticMessage(diagnostic, session.lang)}
      </p>{/each}
    {#if session.error}<p role="alert" class="invalid">{session.error}</p>{/if}
    {#if session.pending}<button
        class="secondary"
        disabled={session.busy}
        onclick={async () => {
          await session.retry();
          if (!session.pending) session.review = null;
        }}>{t("Проверить результат запроса", "Resolve pending request")}</button
      >{/if}
    <div class="review-actions">
      <button
        class="secondary"
        disabled={session.busy || Boolean(session.pending)}
        onclick={() => session.planReview()}
        >{t("Проверить заново", "Revalidate")}</button
      ><button
        class="primary"
        disabled={session.busy ||
          !session.plan ||
          Boolean(session.pending) ||
          (session.review.requiresDisruption && !session.review.consent)}
        onclick={() => session.confirmApply()}
        >{session.preview
          ? t("Сохранить пример", "Save example")
          : t("Применить", "Apply")}</button
      >
    </div>
  {/if}
</dialog>
