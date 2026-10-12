<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { EditorSession } from "../editor/session.svelte";
  import {
    createInstance,
    deleteInstance,
    instanceLabel,
    INSTANCE_LABEL,
  } from "../editor/instances";
  import { schemaFor } from "../editor/bindings";
  import { valueShape } from "../editor/variables";
  import SchemaField from "./SchemaField.svelte";
  let {
    session,
    open,
    onclose,
  }: { session: EditorSession; open: boolean; onclose: () => void } = $props();
  let dialog = $state<HTMLDialogElement>(),
    selected = $state(""),
    templateId = $state(""),
    name = $state("");
  const t = (ru: string, en: string) => session.t(ru, en);
  $effect(() => {
    if (open && !dialog?.open) dialog?.showModal();
    else if (!open && dialog?.open) dialog.close();
  });
  let templates = $derived(
    session.availableCatalogs.flatMap((c) =>
      (c.instanceTemplates ?? []).map((template) => ({
        catalog: c,
        template,
        id: JSON.stringify([c.package.packageId, template.contributionId]),
      })),
    ),
  );
  let instances = $derived(
    Object.values(session.document?.pluginInstances ?? {}),
  );
  let instance = $derived(instances.find((i) => i.instanceId === selected));
  let catalog = $derived(
    session.availableCatalogs.find(
      (c) => c.package.packageId === instance?.packageId,
    ),
  );
  let schema = $derived(
    instance && catalog && schemaFor(catalog, instance.settingsSchema),
  );
  let shape = $derived(schema ? valueShape(schema) : null);
  let blocked = $derived(session.busy || Boolean(session.pending));
</script>

<dialog class="device-picker instance-manager" bind:this={dialog} {onclose}>
  <div class="dialog-heading">
    <h2>{t("Подключения плагинов", "Plugin connections")}</h2>
    <button
      class="icon-button"
      aria-label={t("Закрыть", "Close")}
      onclick={onclose}>×</button
    >
  </div>
  <p class="help">
    {t(
      "Создайте отдельную конфигурацию для каждого аккаунта или набора настроек. Затем выберите её в действии.",
      "Create a separate configuration for each account or settings preset, then select it in an action.",
    )}
  </p>
  <div class="instance-list">
    {#each instances as item}<button
        class="secondary"
        class:active={item.instanceId === selected}
        onclick={() => (selected = item.instanceId)}
        >{instanceLabel(item)}</button
      >{/each}
  </div>
  {#if instance}<label class="form-field"
      ><span>{t("Название подключения", "Connection name")}</span><input
        maxlength="80"
        disabled={blocked}
        value={instanceLabel(instance)}
        oninput={(e) => {
          const value = e.currentTarget.value;
          session.edit((doc) => {
            doc.pluginInstances![selected][INSTANCE_LABEL] = value;
          });
        }}
      /></label
    >
    <p class="help">{instance.packageId}</p>
    {#if shape}<SchemaField
        {shape}
        value={instance.settings}
        label={t("Настройки", "Settings")}
        lang={session.lang}
        disabled={blocked}
        onchange={(value) =>
          session.edit((doc) => {
            const i = doc.pluginInstances![selected];
            i.settings = value;
            i.settingsRevision++;
          })}
      />{:else}<p class="invalid">
        {t(
          "Схема или версия плагина недоступна. Сохранённые настройки сохранены.",
          "The plugin schema/version is unavailable. Existing settings are preserved.",
        )}
      </p>{/if}
    {#if Object.keys((instance.credentials ?? {}) as object).length}<p
        class="help"
      >
        {t(
          "Ссылки на учётные данные сохранены. Секреты управляются через host broker.",
          "Credential references are preserved. Secrets are managed by the host broker.",
        )}
      </p>{/if}
    <button
      class="subtle-button"
      disabled={blocked ||
        Object.values(session.document?.graph.nodeInstances ?? {}).includes(
          selected,
        )}
      onclick={() => session.edit((doc) => deleteInstance(doc, selected))}
      >{t("Удалить подключение", "Delete connection")}</button
    >
  {/if}
  <div class="section-divider"></div>
  {#if templates.length}<label class="form-field"
      ><span>{t("Плагин и конфигурация", "Plugin configuration")}</span><select
        bind:value={templateId}
        ><option value="">{t("Выберите плагин", "Select plugin")}</option
        >{#each templates as item}<option value={item.id}
            >{session.label(item.catalog, item.template.labelKey)} · {item
              .catalog.package.packageId}</option
          >{/each}</select
      ></label
    >
    <label class="form-field"
      ><span>{t("Имя нового подключения", "New connection name")}</span><input
        bind:value={name}
        maxlength="80"
        placeholder={t(
          "Например, личный аккаунт",
          "For example, personal account",
        )}
      /></label
    >
    <button
      class="primary"
      disabled={!templateId || !name.trim() || blocked}
      onclick={() => {
        const item = templates.find((t) => t.id === templateId);
        if (item)
          session.edit((doc) => {
            selected = createInstance(doc, item.catalog, item.template, name);
          });
      }}>{t("Создать подключение", "Create connection")}</button
    >
  {:else}<p class="help">
      {t(
        "Установленные плагины не объявили форм настройки подключений.",
        "Installed plugins have not declared connection settings forms.",
      )}
    </p>{/if}
  {#if session.error}<p role="alert" class="invalid">{session.error}</p>{/if}
</dialog>
