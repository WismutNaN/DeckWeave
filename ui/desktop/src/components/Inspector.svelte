<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { instanceLabel } from "../editor/instances";
  import Icon from "./Icon.svelte";
  import AppearanceEditor from "./AppearanceEditor.svelte";
  import ActionEditor from "./ActionEditor.svelte";
  import SchemaField from "./SchemaField.svelte";
  import { schemaFor } from "../editor/bindings";
  import type { EditorSession } from "../editor/session.svelte";
  let { session }: { session: EditorSession } = $props();
  let tab = $state<"actions" | "appearance">("actions");
  const t = (ru: string, en: string) => session.t(ru, en);
  $effect(() => {
    if (session.selectedStep) tab = "actions";
  });
  let device = $derived(session.device),
    element = $derived(
      device?.descriptor.elements.find(
        (e) => e.elementId === session.selectedControl,
      ),
    );
</script>

<aside
  class="inspector-panel"
  aria-label={t("Настройки выбранного элемента", "Selection settings")}
>
  {#if device}<div class="inspector-heading">
      <div class="selection-symbol">
        <Icon name={element ? "grid" : "device"} size={20} />
      </div>
      <div>
        <h2>
          {element
            ? device.controls[element.elementId]?.label ||
              element.label ||
              element.elementId
            : device.descriptor.label}
        </h2>
        <small
          >{element
            ? device.descriptor.label
            : t("Общие настройки устройства", "Device settings")}</small
        >
      </div>
      {#if element}<button
          class="icon-button"
          title={t("К настройкам устройства", "Device settings")}
          onclick={() => session.select(device!.id)}
          ><Icon name="sliders" /></button
        >{/if}
    </div>
    {#if element}<div class="inspector-tabs">
        <button
          class:active={tab === "actions"}
          onclick={() => (tab = "actions")}
          ><Icon name="bolt" size={15} />{t("Действия", "Actions")}</button
        ><button
          class:active={tab === "appearance"}
          onclick={() => (tab = "appearance")}
          ><Icon name="palette" size={15} />{t(
            "Оформление",
            "Appearance",
          )}</button
        >
      </div>{/if}
    <div class="inspector-content">
      {#if element}<label class="form-field"
          ><span>{t("Подпись на кнопке", "Key label")}</span><input
            maxlength="80"
            placeholder={t("Название действия", "Action name")}
            value={device.controls[element.elementId]?.label ?? ""}
            disabled={session.editLocked}
            oninput={(e) => {
              const value = e.currentTarget.value,
                id = element.elementId;
              session.updateDevice((d) => {
                d.controls[id] ??= {};
                d.controls[id].label = value;
              });
            }}
          /></label
        >
        <div class="section-divider"></div>{/if}
      {#if element && tab === "actions"}<ActionEditor
          {session}
        />{:else}<AppearanceEditor {session} />{/if}
      {#if !element}<div class="section-divider"></div>
        <div class="section-heading">
          <h3>{t("Возможности устройства", "Device capabilities")}</h3>
        </div>
        <div class="capability-list">
          {#each [...new Set(device.descriptor.elements.flatMap( (e) => e.capabilities.map((c) => c.kind) ))] as capability}<span
              >{capability.split(".").at(-1)}</span
            >{/each}
        </div>
        {#each Object.values(session.document?.pluginInstances ?? {}).filter((i) => i.packageId === device?.package.packageId) as instance}{@const catalog =
            session.availableCatalogs.find(
              (c) => c.package.packageId === instance.packageId,
            )}{@const shape = catalog
            ? schemaFor(catalog, instance.settingsSchema)?.shape
            : null}{#if shape && typeof shape === "object"}<div
              class="section-divider"
            ></div>
            <SchemaField
              shape={shape.structured}
              value={instance.settings}
              label={instanceLabel(instance)}
              lang={session.lang}
              disabled={session.editLocked}
              onchange={(value) =>
                session.edit((doc) => {
                  const current = doc.pluginInstances?.[instance.instanceId];
                  if (current) {
                    current.settings = value;
                    current.settingsRevision++;
                  }
                })}
            />{/if}{/each}{/if}
    </div>{:else}<div class="inspector-placeholder">
      <Icon name="sliders" size={25} />
      <h3>{t("Всё начинается с выбора", "Start with a selection")}</h3>
      <p>
        {t(
          "Выберите устройство для общих настроек или кнопку для её действий и оформления.",
          "Select a device for shared settings, or a control for its actions and appearance.",
        )}
      </p>
    </div>{/if}
</aside>
