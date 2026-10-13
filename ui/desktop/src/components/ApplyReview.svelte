<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { EditorSession } from "../editor/session.svelte";
  import { summarizeChanges, diagnosticMessage } from "../editor/review";
  import { reviewDetails } from "../editor/review-details";
  import { BINDING_KEY, equal } from "../editor/bindings";
  let { session }: { session: EditorSession } = $props();
  let dialog = $state<HTMLDialogElement>(),
    limit = $state(20);
  const t = (ru: string, en: string) => session.t(ru, en);
  $effect(() => {
    if (session.review && !dialog?.open) {
      limit = 20;
      dialog?.showModal();
    } else if (!session.review && dialog?.open) dialog.close();
  });
  let diff = $derived(
    session.review &&
      summarizeChanges(session.review.before, session.review.document),
  );
  let rows = $derived(
    session.review
      ? reviewDetails(session.review.before, session.review.document)
      : [],
  );
  let invalid = $derived(Boolean(session.error || session.diagnostics.length));
  function goTo(nodeId: string | null) {
    const node = session.document?.graph.nodes.find((n) => n.id === nodeId),
      identity = node?.extensions[BINDING_KEY];
    if (typeof identity !== "string") return;
    try {
      const [device, control] = JSON.parse(identity);
      session.select(device, control);
      session.selectedStep = nodeId ?? "";
      session.review = null;
    } catch {}
  }
</script>

<dialog
  class="apply-review"
  aria-labelledby="apply-review-title"
  aria-describedby="apply-review-description"
  bind:this={dialog}
  onclose={() => (session.review = null)}
  oncancel={(e) => {
    if (session.busy) e.preventDefault();
  }}
>
  <header class="review-header">
    <span class="review-symbol"><Icon name="layers" size={22} /></span>
    <div>
      <h2 id="apply-review-title">
        {t("Проверка изменений", "Review changes")}
      </h2>
      <p id="apply-review-description">
        {t(
          "Проверьте, что изменится на ваших устройствах.",
          "Review what will change on your devices.",
        )}
      </p>
    </div>
    <button
      class="icon-button"
      aria-label={t("Закрыть", "Close")}
      disabled={session.busy}
      onclick={() => (session.review = null)}><Icon name="close" /></button
    >
  </header>
  {#if session.review && diff}
    <div class="review-content">
      <div class="review-context">
        <span
          >{session.review.expectedActive
            ? t("Работающая конфигурация", "Running configuration")
            : t("Новое рабочее пространство", "New workspace")}</span
        ><Icon name="arrow" size={14} /><strong
          >{t("Черновик", "Draft")} v{session.review.draftVersion}</strong
        >
      </div>
      <div
        class="review-counts"
        aria-label={t("Изменения действий", "Action changes")}
      >
        <div>
          <strong>{diff.added}</strong><span>{t("Добавлено", "Added")}</span>
        </div>
        <div>
          <strong>{diff.changed}</strong><span>{t("Изменено", "Changed")}</span>
        </div>
        <div>
          <strong>{diff.removed}</strong><span>{t("Удалено", "Removed")}</span>
        </div>
      </div>
      {#if rows.length}<h3 class="review-section-title">
          {t("Действия на кнопках", "Control actions")}
        </h3>
        <ul class="review-change-list">
          {#each rows.slice(0, limit) as row (row.node.id)}<li>
              <span
                class="review-change-icon"
                class:removed={row.kind === "removed"}
                ><Icon
                  name={row.kind === "added"
                    ? "plus"
                    : row.kind === "removed"
                      ? "minus"
                      : "sliders"}
                  size={15}
                /></span
              >
              <div>
                <strong
                  >{session.actions.find((a) =>
                    equal(a.endpoint, row.node.kind.endpoint),
                  )?.title ?? row.node.kind.operation_id}</strong
                ><small>{row.location}</small>
              </div>
              <span class="review-change-label"
                >{row.kind === "added"
                  ? t("Новое", "Added")
                  : row.kind === "removed"
                    ? t("Удаление", "Removed")
                    : t("Настройки", "Updated")}</span
              >
            </li>{/each}
        </ul>
        {#if rows.length > limit}<button
            class="text-button review-more"
            onclick={() => (limit += 20)}
            >{t("Показать ещё", "Show more")} · {rows.length - limit}</button
          >{/if}{/if}
      {#if diff.layout || diff.connections || diff.routing}<div
          class="review-other"
        >
          {#if diff.layout}<p>
              <Icon name="palette" size={16} /><span
                >{t(
                  "Оформление и расположение устройств",
                  "Device appearance and layout",
                )}</span
              >
            </p>{/if}
          {#if diff.connections}<p>
              <Icon name="settings" size={16} /><span
                >{t(
                  "Подключения плагинов и их настройки",
                  "Plugin connections and settings",
                )}</span
              >
            </p>{/if}
          {#if diff.routing}<p>
              <Icon name="bolt" size={16} /><span
                >{t(
                  "Последовательность и связи действий",
                  "Action sequences and connections",
                )}</span
              >
            </p>{/if}
        </div>{/if}
      {#if !rows.length && !diff.layout && !diff.connections && !diff.routing}<p
          class="review-empty"
        >
          {t(
            "Конфигурация совпадает с работающей версией.",
            "This configuration matches the running version.",
          )}
        </p>{/if}
      {#if session.review.requiresDisruption}<section class="review-warning">
          <div>
            <Icon name="alert" size={18} /><strong
              >{t(
                "Потребуется прервать текущую работу",
                "Running actions will be interrupted",
              )}</strong
            >
          </div>
          <p>
            {t(
              "Затронутые устройства переподключатся при применении.",
              "Affected devices will reconnect when changes are applied.",
            )}
          </p>
          <label class="check-row"
            ><input
              type="checkbox"
              checked={session.review.consent}
              disabled={session.busy}
              onchange={(e) => {
                session.review!.consent = e.currentTarget.checked;
                void session.planReview();
              }}
            />{t("Разрешить прерывание", "Allow interruption")}</label
          >
        </section>{/if}
      {#if invalid}<section class="review-errors" role="alert">
          <strong
            >{t(
              "Нужно исправить перед применением",
              "Fix these issues before applying",
            )}</strong
          >{#each session.diagnostics as d}<div>
              <p>
                {diagnosticMessage(d, session.lang)}{d.field
                  ? ` · ${d.field}`
                  : ""}
              </p>
              {#if d.nodeId}<button
                  class="text-button"
                  disabled={session.busy}
                  onclick={() => goTo(d.nodeId)}
                  >{t("К настройке", "Open setting")}<Icon
                    name="arrow"
                    size={13}
                  /></button
                >{/if}
            </div>{/each}{#if session.error && !session.diagnostics.length}<p>
              {session.error}
            </p>{/if}
        </section>{/if}
    </div>
    <footer class="review-footer">
      <div class="review-validation" class:invalid>
        <Icon
          name={invalid ? "alert" : session.plan ? "check" : "refresh"}
          size={16}
        /><span
          >{session.busy
            ? t("Проверяем…", "Checking…")
            : invalid
              ? t("Есть ошибки", "Needs attention")
              : session.plan
                ? t("Готово к применению", "Ready to apply")
                : t("Ожидает проверки", "Awaiting validation")}</span
        >
      </div>
      <div class="review-actions">
        {#if session.pending}<button
            class="secondary"
            disabled={session.busy}
            onclick={async () => {
              await session.retry();
              if (!session.pending) session.review = null;
            }}
            >{t(
              "Проверить результат запроса",
              "Resolve pending request",
            )}</button
          >{:else}
          {#if !session.plan || invalid}<button
              class="secondary"
              disabled={session.busy}
              onclick={() => session.planReview()}
              >{t("Проверить заново", "Revalidate")}</button
            >{:else}<button
              class="secondary"
              disabled={session.busy}
              onclick={() => (session.review = null)}
              >{t("Вернуться", "Back")}</button
            >{/if}
          <button
            class="primary"
            disabled={session.busy ||
              !session.plan ||
              invalid ||
              (session.review.requiresDisruption && !session.review.consent)}
            onclick={() => session.confirmApply()}
            ><Icon name="check" size={15} />{session.preview
              ? t("Сохранить пример", "Save example")
              : t("Применить", "Apply")}</button
          >
        {/if}
      </div>
    </footer>
  {/if}
</dialog>
