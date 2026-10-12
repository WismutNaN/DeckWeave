<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import SchemaField from "./SchemaField.svelte";
  import {
    actionShape,
    BINDING_KEY,
    equal,
    initialValue,
    owner,
    removeLastAction,
    schemaFor,
    steps,
    sourceCandidates,
    sourceFor,
    foreignBinding,
    bindingNodes,
  } from "../editor/bindings";
  import type { EditorSession } from "../editor/session.svelte";
  import { valuePaths, valueShape } from "../editor/variables";
  import type { Binding, Json, Shape } from "../editor/types";
  let { session }: { session: EditorSession } = $props();
  const t = (ru: string, en: string) => session.t(ru, en);
  let chain = $derived(
    session.document && session.device
      ? steps(session.document, session.device.id, session.selectedControl)
      : [],
  );
  let node = $derived(
    chain.find((n) => n.id === session.selectedStep) ?? chain.at(-1),
  );
  let action = $derived(
    session.actions.find((a) => equal(a.endpoint, node?.kind.endpoint)),
  );
  let shape = $derived(action ? actionShape(action) : null);
  let fields = $derived(
    node
      ? (session.document?.graph.actionArguments?.[node.id]?.fields ?? {})
      : {},
  );
  let blocked = $derived(session.busy || Boolean(session.pending));
  let advanced = $derived(
    Boolean(
      session.document &&
      session.device &&
      foreignBinding(session.document, session.device, session.selectedControl),
    ),
  );
  let input = $derived(
    session.device
      ? sourceFor(session.catalogs, session.device, session.selectedControl)
      : undefined,
  );
  let booleanInput = $derived(
    Boolean(
      input &&
      session.catalogs.some(
        (c) => schemaFor(c, input!.port.schema)?.shape === "boolean",
      ),
    ),
  );
  function setField(key: string, binding: Binding | undefined) {
    if (!node) return;
    const id = node.id;
    session.edit((doc) => {
      doc.graph.actionArguments ??= {};
      doc.graph.actionArguments[id] ??= { triggerPort: "trigger", fields: {} };
      if (binding) doc.graph.actionArguments[id].fields[key] = binding;
      else delete doc.graph.actionArguments[id].fields[key];
    });
  }
  let triggers = $derived(
    session.device
      ? sourceCandidates(
          session.catalogs,
          session.device,
          session.selectedControl,
        )
      : [],
  );
  function variables(target: Shape) {
    if (!session.document || !session.device || !node) return [];
    const ordered = bindingNodes(
      session.document,
      session.device.id,
      session.selectedControl,
    );
    const index = ordered.findIndex((n) => n.id === node?.id);
    return ordered
      .slice(0, index)
      .filter(
        (n) =>
          n.extensions[BINDING_KEY] ===
            owner(session.device!.id, session.selectedControl) &&
          ["source", "action"].includes(n.kind.type),
      )
      .flatMap((n) => {
        const c = session.catalogs.find(
            (c) => c.package.packageId === n.kind.endpoint?.packageId,
          ),
          ref = n.outputs[0]?.schema,
          s = c && ref ? schemaFor(c, ref) : undefined;
        const title =
          n.kind.type === "source"
            ? t("Событие кнопки", "Control event")
            : (session.actions.find((a) => equal(a.endpoint, n.kind.endpoint))
                ?.title ?? n.id);
        const sourceShape = s ? valueShape(s) : null;
        const pointers = sourceShape
          ? valuePaths(sourceShape, target).map((p) => ({
              pointer: p.pointer,
              title: p.pointer ? title + " · " + p.label : title,
            }))
          : [];
        return pointers.map((p) => ({ ...p, nodeId: n.id }));
      });
  }
</script>

{#if triggers.length > 1}<label class="form-field"
    ><span>{t("Событие устройства", "Device event")}</span><select
      disabled={blocked || chain.length > 0}
      value={JSON.stringify(
        session.device?.controls[session.selectedControl]?.trigger ?? null,
      )}
      onchange={(e) => {
        const trigger = JSON.parse(e.currentTarget.value);
        session.updateDevice((d) => {
          d.controls[session.selectedControl] ??= {};
          d.controls[session.selectedControl].trigger = trigger;
        });
      }}
      ><option value="null" disabled
        >{t("Выберите событие", "Select an event")}</option
      >{#each triggers as trigger}<option
          value={JSON.stringify(trigger.endpoint)}
          >{trigger.endpoint.port.capabilityId} · {trigger.endpoint.port.portId} ({trigger
            .port.kind})</option
        >{/each}</select
    ></label
  >{/if}
{#if booleanInput}<label class="form-field"
    ><span>{t("Когда выполнять", "When to run")}</span><select
      disabled={blocked || chain.length > 0}
      value={session.device?.controls[session.selectedControl]?.when ?? "true"}
      onchange={(e) => {
        const when = e.currentTarget.value as "true" | "false" | "any";
        session.updateDevice((d) => {
          d.controls[session.selectedControl] ??= {};
          d.controls[session.selectedControl].when = when;
        });
      }}
      ><option value="true"
        >{t("При значении true · нажатие", "Value true · pressed")}</option
      ><option value="false"
        >{t(
          "При значении false · отпускание",
          "Value false · released",
        )}</option
      ><option value="any">{t("При каждом событии", "On every event")}</option
      ></select
    >{#if chain.length}<small class="field-help"
        >{t(
          "Для смены события сначала удалите шаги этой кнопки.",
          "Remove this key's steps before changing the trigger.",
        )}</small
      >{/if}</label
  >{/if}
{#if !chain.length}<div class="binding-empty">
    <div class="binding-target"><Icon name="bolt" size={24} /></div>
    <h3>{t("Что делает эта кнопка?", "What should this key do?")}</h3>
    <p>
      {advanced
        ? t(
            "Этот элемент связан со сложным графом. Его узлы сохранены; для изменения потребуется редактор графа.",
            "This control belongs to an advanced graph. Its nodes are preserved and require the graph editor.",
          )
        : t(
            "Выберите действие слева или перетащите его на кнопку.",
            "Choose an action on the left or drag it onto this key.",
          )}
    </p>
    {#if !session.hasSource}<p class="field-help invalid">
        {t(
          "Плагин ещё не предоставил контракт ввода для этого элемента.",
          "The plugin has not provided an input contract for this control.",
        )}
      </p>{/if}
  </div>{:else}
  <div class="section-heading">
    <h3>{t("При событии кнопки", "On control event")}</h3>
    <span class="count-badge">{chain.length}</span>
  </div>
  <ol class="action-chain">
    {#each chain as step, index}<li>
        <button
          class:active={step.id === node?.id}
          onclick={() => (session.selectedStep = step.id)}
          ><span class="step-number">{index + 1}</span><span
            >{session.actions.find((a) => equal(a.endpoint, step.kind.endpoint))
              ?.title ?? step.kind.operation_id}</span
          ><Icon name="chevron" size={14} /></button
        >
      </li>{/each}
  </ol>
  <p class="field-help">
    {t(
      "Следующее действие запускается после успешного результата предыдущего.",
      "Each action starts after the previous one succeeds.",
    )}
  </p>
  {#if action && node}<div class="section-heading">
      <h3>{t("Параметры действия", "Action parameters")}</h3>
    </div>
    {#if shape?.type === "record"}{#each Object.entries(shape.fields) as [key, field]}{@const binding =
          fields[key]}
        <div class="argument-field">
          {#if field.optional}<label class="check-field"
              ><input
                type="checkbox"
                checked={Boolean(binding)}
                disabled={blocked}
                onchange={(e) =>
                  setField(
                    key,
                    e.currentTarget.checked
                      ? { type: "constant", value: initialValue(field.schema) }
                      : undefined,
                  )}
              />{key}</label
            >{/if}
          {#if !field.optional || binding}<div class="argument-source">
              <label for={`source-${node.id}-${key}`}>{key}</label><select
                id={`source-${node.id}-${key}`}
                disabled={blocked}
                value={binding && binding.type !== "constant"
                  ? JSON.stringify(binding)
                  : "constant"}
                onchange={(e) =>
                  setField(
                    key,
                    e.currentTarget.value === "constant"
                      ? { type: "constant", value: initialValue(field.schema) }
                      : (JSON.parse(e.currentTarget.value) as Binding),
                  )}
                ><option value="constant"
                  >{t("Моё значение", "My value")}</option
                >{#each variables(field.schema) as variable}<option
                    value={JSON.stringify({
                      type: "run_result",
                      nodeId: variable.nodeId,
                      pointer: variable.pointer,
                    })}>{variable.title}</option
                  >{/each}</select
              >
            </div>
            {#if !binding || binding.type === "constant"}<SchemaField
                shape={field.schema}
                value={binding?.type === "constant" ? binding.value : undefined}
                label={t("Значение", "Value")}
                lang={session.lang}
                disabled={blocked}
                onchange={(value: Json) =>
                  setField(key, { type: "constant", value })}
              />{:else}<p class="field-help variable-info">
                <Icon name="layers" size={14} />{t(
                  "Значение придёт во время выполнения",
                  "Value is supplied during execution",
                )}
              </p>{/if}{/if}
        </div>{/each}
    {:else if shape?.type === "null"}<p class="field-help">
        {t(
          "Действие готово к использованию без параметров.",
          "This action is ready without parameters.",
        )}
      </p>{:else}<p class="field-help invalid">
        {t(
          "Для этой схемы нужен специализированный инспектор.",
          "This schema requires a specialized inspector.",
        )}
      </p>{/if}
  {/if}
  <button
    class="subtle-button wide"
    disabled={blocked}
    onclick={() => {
      if (session.device)
        session.edit((doc) =>
          removeLastAction(doc, session.device!.id, session.selectedControl),
        );
    }}>{t("Удалить последний шаг", "Remove last step")}</button
  >
{/if}
