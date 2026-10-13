<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { dragSource, dropZone } from "../editor/pointer-drag";
  import {
    hasActionDrag,
    writeSelectionDrag,
    readSelectionDrag,
  } from "../editor/drag";
  import { dropOnControl } from "../editor/drops";
  import { reorderAction } from "../editor/binding-edits";
  import { displayEndpoints } from "../editor/feedback";
  import Icon from "./Icon.svelte";
  import { instanceLabel } from "../editor/instances";
  import { diagnosticMessage } from "../editor/review";
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
  let blocked = $derived(session.editLocked);
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

<div
  role="group"
  aria-label={t("Назначение действий", "Action assignment")}
  use:dropZone={{
    accept: (data) =>
      !blocked &&
      hasActionDrag(data) &&
      readSelectionDrag(data)?.kind !== "device" &&
      readSelectionDrag(data)?.kind !== "step",
    drop: (e) =>
      dropOnControl(
        session,
        e,
        session.selectedDevice,
        session.selectedControl,
      ),
  }}
  class="binding-drop-zone"
  ondragover={(e) => {
    if (hasActionDrag(e.dataTransfer) && !blocked) {
      e.preventDefault();
      e.stopPropagation();
    }
  }}
  ondrop={(e) =>
    dropOnControl(session, e, session.selectedDevice, session.selectedControl)}
>
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
            >{trigger.endpoint.port.capabilityId} · {trigger.endpoint.port
              .portId} ({trigger.port.kind})</option
          >{/each}</select
      ></label
    >{/if}
  {#if booleanInput}<label class="form-field"
      ><span>{t("Когда выполнять", "When to run")}</span><select
        disabled={blocked || chain.length > 0}
        value={session.device?.controls[session.selectedControl]?.when ??
          "true"}
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
      {#each chain as step, index}<li
          use:dropZone={{
            accept: (data) =>
              !blocked && readSelectionDrag(data)?.kind === "step",
            drop: (e) =>
              dropOnControl(
                session,
                e,
                session.selectedDevice,
                session.selectedControl,
                step.id,
              ),
          }}
          ondragover={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          ondrop={(e) =>
            dropOnControl(
              session,
              e,
              session.selectedDevice,
              session.selectedControl,
              step.id,
            )}
        >
          <button
            draggable={false}
            use:dragSource={(data) => {
              if (blocked) return false;
              writeSelectionDrag(data, {
                kind: "step",
                deviceId: session.selectedDevice,
                controlId: session.selectedControl,
                stepId: step.id,
              });
              return true;
            }}
            ondragstart={(e) => {
              if (e.dataTransfer)
                writeSelectionDrag(e.dataTransfer, {
                  kind: "step",
                  deviceId: session.selectedDevice,
                  controlId: session.selectedControl,
                  stepId: step.id,
                });
            }}
            class:active={step.id === node?.id}
            onclick={() => (session.selectedStep = step.id)}
            ><span class="step-number">{index + 1}</span><span
              >{session.actions.find((a) =>
                equal(a.endpoint, step.kind.endpoint),
              )?.title ?? step.kind.operation_id}</span
            ><Icon name="chevron" size={14} /></button
          >
          {#if index > 0}<button
              class="step-reorder"
              aria-label={t("Поднять шаг", "Move step up") + " " + (index + 1)}
              disabled={blocked}
              onclick={() =>
                session.edit((doc) =>
                  reorderAction(
                    doc,
                    session.availableCatalogs,
                    session.selectedDevice,
                    session.selectedControl,
                    step.id,
                    chain[index - 1].id,
                  ),
                )}>↑</button
            >{/if}
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
      {@const instances = Object.values(
        session.document?.pluginInstances ?? {},
      ).filter(
        (i) =>
          i.packageId === action.endpoint.packageId &&
          i.contributionId === action.contributionId,
      )}
      {#if instances.length}<label class="form-field"
          ><span>{t("Подключение", "Connection")}</span><select
            disabled={blocked}
            value={session.document?.graph.nodeInstances?.[node.id] ?? ""}
            onchange={(e) => {
              const id = node!.id,
                value = e.currentTarget.value;
              session.edit((doc) => {
                doc.graph.nodeInstances ??= {};
                if (value) doc.graph.nodeInstances[id] = value;
                else delete doc.graph.nodeInstances[id];
              });
            }}
            ><option value="">{t("По умолчанию", "Default")}</option
            >{#each instances as instance}<option value={instance.instanceId}
                >{instanceLabel(instance)}</option
              >{/each}</select
          ></label
        >{/if}
      {#each session.diagnostics.filter((d) => d.nodeId === node?.id && !d.field) as diagnostic}<p
          class="invalid"
          role="alert"
        >
          {diagnosticMessage(diagnostic, session.lang)}
        </p>{/each}
      {#if shape?.type === "record"}{#each Object.entries(shape.fields) as [key, field]}{@const binding =
            fields[key]}
          <div
            class="argument-field"
            class:field-invalid={session.diagnostics.some(
              (d) => d.nodeId === node?.id && d.field === key,
            )}
          >
            {#each session.diagnostics.filter((d) => d.nodeId === node?.id && d.field === key) as diagnostic}<p
                class="invalid"
                role="alert"
              >
                {diagnosticMessage(diagnostic, session.lang)}
              </p>{/each}
            {#if field.optional}<label class="check-field"
                ><input
                  type="checkbox"
                  checked={Boolean(binding)}
                  disabled={blocked}
                  onchange={(e) =>
                    setField(
                      key,
                      e.currentTarget.checked
                        ? {
                            type: "constant",
                            value: initialValue(field.schema),
                          }
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
                        ? {
                            type: "constant",
                            value: initialValue(field.schema),
                          }
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
                  value={binding?.type === "constant"
                    ? binding.value
                    : undefined}
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
    {#if session.device && displayEndpoints(session.availableCatalogs, session.device, session.selectedControl).some((e) => e.port.kind === "ValueIn")}<label
        class="check-row"
        ><input
          type="checkbox"
          checked={Boolean(
            session.device.controls[session.selectedControl]?.showResult,
          )}
          disabled={blocked}
          onchange={(e) =>
            session.updateDevice((d) => {
              d.controls[session.selectedControl] ??= {};
              d.controls[session.selectedControl].showResult =
                e.currentTarget.checked;
            })}
        />{t(
          "Показать результат на этой кнопке",
          "Show the result on this control",
        )}</label
      >{/if}
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
</div>
