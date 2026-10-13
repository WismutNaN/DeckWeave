<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { EditorSession } from "../editor/session.svelte";
  import {
    migrationSources,
    planEndpointMigration,
  } from "../editor/package-migration";
  let { session }: { session: EditorSession } = $props();
  const t = (ru: string, en: string) => session.t(ru, en);
  const sources = $derived(
    session.document
      ? migrationSources(session.document, session.availableCatalogs)
      : [],
  );
  function preview(source: string) {
    try {
      return {
        plan: planEndpointMigration(
          session.document!,
          session.availableCatalogs,
          source,
        ),
        error: "",
      };
    } catch (cause) {
      return {
        plan: null,
        error: String(cause instanceof Error ? cause.message : cause),
      };
    }
  }
</script>

{#if sources.length}
  <div class="section-divider"></div>
  <h3>{t("Обновление назначений плагинов", "Update plugin assignments")}</h3>
  <p class="help">
    {t(
      "Можно перенести назначения в отдельные плагины. Прежняя версия останется для отката; доступ новым плагинам выдаётся отдельно в разрешениях. Перенос можно отменить.",
      "Assignments can move into separate plugins. The previous version remains for rollback; grant access to the new plugins separately in permissions. You can undo the migration.",
    )}
  </p>
  {#each sources as source}
    {@const candidate = preview(source)}
    <p class="help">{source}</p>
    {#if candidate.plan}<p class="help">
        {candidate.plan.packages.join(", ")}
      </p>{/if}
    <button
      class="secondary full-width"
      disabled={session.busy || Boolean(session.pending) || !candidate.plan}
      onclick={() =>
        session.edit((doc) =>
          Object.assign(
            doc,
            planEndpointMigration(doc, session.availableCatalogs, source)
              .document,
          ),
        )}
    >
      {t("Перенести назначения", "Migrate assignments")}
    </button>
    {#if candidate.error}<p class="help invalid" role="alert">
        {candidate.error}
      </p>{/if}
  {/each}
{/if}
