<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import SchemaField from "./SchemaField.svelte";
  import type { Json, Shape } from "../editor/types";
  import { initialValue } from "../editor/bindings";
  let {
    shape,
    value,
    onchange,
    label,
    disabled = false,
    lang = "ru",
    depth = 0,
  }: {
    shape: Shape;
    value: Json | undefined;
    onchange: (value: Json) => void;
    label: string;
    disabled?: boolean;
    lang?: string;
    depth?: number;
  } = $props();
  const t = (ru: string, en: string) => (lang === "ru" ? ru : en);
  function record() {
    return (
      value && !Array.isArray(value) && typeof value === "object" ? value : {}
    ) as Record<string, Json>;
  }
</script>

{#if depth > 8}<p class="field-help">
    {t(
      "Слишком глубокая структура для формы.",
      "This structure is too deep for the form.",
    )}
  </p>
{:else if shape.type === "string"}
  <label class="form-field"
    ><span>{label}</span>{#if shape.choices?.length}<select
        {disabled}
        value={String(value ?? "")}
        onchange={(e) => onchange(e.currentTarget.value)}
        >{#each shape.choices as choice}<option>{choice}</option>{/each}</select
      >{:else}<textarea
        rows={shape.maximumBytes > 200 ? 3 : 1}
        {disabled}
        value={String(value ?? "")}
        oninput={(e) => onchange(e.currentTarget.value)}
        title={`${shape.maximumBytes} bytes`}
      ></textarea>{#if new TextEncoder().encode(String(value ?? "")).length > shape.maximumBytes}<small
          class="invalid"
          >{t("Превышен размер текста", "Text is too long")}</small
        >{/if}{/if}</label
  >
{:else if shape.type === "number"}
  <label class="form-field"
    ><span>{label}</span><input
      type="number"
      {disabled}
      min={shape.minimum}
      max={shape.maximum}
      step="any"
      value={typeof value === "number" ? value : shape.minimum}
      onchange={(e) => {
        if (Number.isFinite(e.currentTarget.valueAsNumber))
          onchange(e.currentTarget.valueAsNumber);
      }}
    /></label
  >
{:else if shape.type === "boolean"}
  <label class="check-field"
    ><input
      type="checkbox"
      {disabled}
      checked={value === true}
      onchange={(e) => onchange(e.currentTarget.checked)}
    />{label}</label
  >
{:else if shape.type === "record"}
  <fieldset class="nested-fields">
    <legend>{label}</legend
    >{#each Object.entries(shape.fields) as [key, field]}{#if field.optional}<label
          class="check-field"
          ><input
            type="checkbox"
            {disabled}
            checked={key in record()}
            onchange={(e) => {
              const next = { ...record() };
              if (e.currentTarget.checked)
                next[key] = initialValue(field.schema);
              else delete next[key];
              onchange(next);
            }}
          />{key}</label
        >{/if}{#if !field.optional || key in record()}<SchemaField
          shape={field.schema}
          value={record()[key]}
          label={key}
          {lang}
          {disabled}
          depth={depth + 1}
          onchange={(v) => onchange({ ...record(), [key]: v })}
        />{/if}{/each}
  </fieldset>
{:else if shape.type === "nullable"}
  <label class="check-field"
    ><input
      type="checkbox"
      {disabled}
      checked={value !== null}
      onchange={(e) =>
        onchange(e.currentTarget.checked ? initialValue(shape.inner) : null)}
    />{label}</label
  >{#if value !== null}<SchemaField
      shape={shape.inner}
      {value}
      {onchange}
      {label}
      {disabled}
      {lang}
      depth={depth + 1}
    />{/if}
{:else if shape.type === "array"}
  <fieldset class="nested-fields">
    <legend>{label}</legend
    >{#each Array.isArray(value) ? value : [] as item, i}<div class="array-row">
        <SchemaField
          shape={shape.items}
          value={item}
          label={String(i + 1)}
          {lang}
          {disabled}
          depth={depth + 1}
          onchange={(v) => {
            const next = Array.isArray(value) ? [...value] : [];
            next[i] = v;
            onchange(next);
          }}
        /><button
          class="icon-button"
          {disabled}
          aria-label={t("Удалить элемент", "Remove item")}
          onclick={() =>
            onchange(
              (Array.isArray(value) ? value : []).filter(
                (_, index) => index !== i,
              ),
            )}>×</button
        >
      </div>{/each}<button
      class="subtle-button"
      disabled={disabled ||
        (Array.isArray(value) ? value.length : 0) >=
          Math.min(64, shape.maximumItems)}
      onclick={() =>
        onchange([
          ...(Array.isArray(value) ? value : []),
          initialValue(shape.items),
        ])}>+ {t("Добавить элемент", "Add item")}</button
    >
  </fieldset>
{:else}<p class="field-help">{t("Без параметров", "No parameters")}</p>{/if}
