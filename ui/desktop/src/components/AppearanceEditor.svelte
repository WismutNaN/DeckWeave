<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import { copy } from "../editor/api";
  import { geometry } from "../editor/appearance";
  import type { EditorSession } from "../editor/session.svelte";
  import type { Background } from "../editor/types";
  let { session }: { session: EditorSession } = $props();
  const t = (ru: string, en: string) => session.t(ru, en);
  let device = $derived(session.device),
    control = $derived(session.selectedControl),
    own = $derived(device?.controls[control]?.background),
    background = $derived(own ?? device?.appearance),
    blocked = $derived(session.editLocked);
  function change(patch: Partial<Background>) {
    session.updateDevice((d) => {
      if (control) {
        d.controls[control] ??= {};
        d.controls[control].background = {
          ...copy(d.controls[control].background ?? d.appearance),
          ...patch,
        };
      } else d.appearance = { ...d.appearance, ...patch };
    });
  }
  const palettes = [
    ["#3e6687", "#b47769"],
    ["#296857", "#d0ad62"],
    ["#4c477b", "#b9759a"],
    ["#335f8f", "#62b8af"],
    ["#353b49", "#778092"],
  ];
</script>

{#if device && background}
  <div class="section-heading">
    <h3>
      {control
        ? t("Фон кнопки", "Key background")
        : t("Общий фон", "Device background")}
    </h3>
    <span class="small-tag">{t("Макет", "Layout")}</span>
  </div>
  {#if control}<div class="inheritance-row">
      <Icon name="layers" size={16} /><span
        >{own
          ? t("Собственное правило", "Custom rule")
          : t("Наследуется от устройства", "Inherited from device")}</span
      >{#if own}<button
          class="text-button"
          disabled={blocked}
          onclick={() =>
            session.updateDevice((d) => {
              delete d.controls[control]?.background;
            })}>{t("Сбросить", "Reset")}</button
        >{/if}
    </div>{/if}
  <div class="segmented">
    <button
      class:active={background.mode === "solid"}
      disabled={blocked}
      onclick={() => change({ mode: "solid" })}>{t("Цвет", "Color")}</button
    ><button
      class:active={background.mode === "gradient"}
      disabled={blocked || (!control && !geometry(device.descriptor).measured)}
      onclick={() => change({ mode: "gradient" })}
      >{t("Градиент", "Gradient")}</button
    >
  </div>
  <div class="color-fields">
    <label class="color-field"
      ><input
        aria-label={t("Первый цвет", "First color")}
        type="color"
        value={background.start}
        disabled={blocked}
        oninput={(e) => change({ start: e.currentTarget.value })}
      /><span>{background.start.toUpperCase()}</span></label
    >{#if background.mode === "gradient"}<span class="color-arrow">→</span
      ><label class="color-field"
        ><input
          aria-label={t("Второй цвет", "Second color")}
          type="color"
          value={background.end}
          disabled={blocked}
          oninput={(e) => change({ end: e.currentTarget.value })}
        /><span>{background.end.toUpperCase()}</span></label
      >{/if}
  </div>
  {#if background.mode === "gradient"}<label class="range-field"
      ><span
        >{t("Направление", "Direction")}<output>{background.angle}°</output
        ></span
      ><input
        type="range"
        min="0"
        max="360"
        step="5"
        disabled={blocked}
        value={background.angle}
        oninput={(e) => change({ angle: e.currentTarget.valueAsNumber })}
      /></label
    >{/if}
  <label class="range-field"
    ><span
      >{t("Яркость макета", "Layout brightness")}<output
        >{background.brightness}%</output
      ></span
    ><input
      type="range"
      min="10"
      max="100"
      disabled={blocked}
      value={background.brightness}
      oninput={(e) => change({ brightness: e.currentTarget.valueAsNumber })}
    /></label
  >
  <div class="section-heading"><h3>{t("Палитра", "Palette")}</h3></div>
  <div class="palette-presets">
    {#each palettes as [start, end]}<button
        title={`${start} → ${end}`}
        aria-label={`${t("Палитра", "Palette")} ${start} → ${end}`}
        disabled={blocked}
        style:background={`linear-gradient(120deg,${start},${end})`}
        onclick={() => change({ start, end })}
      ></button>{/each}
  </div>
  {#if !control}<p class="field-help">
      {t(
        "Фон распределяется по положению кнопок. Собственные настройки кнопки сохраняются при смене общей палитры.",
        "The background follows control positions. Key overrides stay intact when the device palette changes.",
      )}
    </p>{:else}<label class="form-field"
      ><span>{t("Цвет текста", "Text color")}</span><input
        type="color"
        value={device.controls[control]?.foreground ?? "#ffffff"}
        disabled={blocked}
        oninput={(e) => {
          const value = e.currentTarget.value;
          session.updateDevice((d) => {
            d.controls[control] ??= {};
            d.controls[control].foreground = value;
          });
        }}
      /></label
    >{/if}
  <div class="scope-note">
    <Icon name="palette" size={15} />
    <p>
      {t(
        "Внешний вид сохраняется в макете. Передача изображения и подсветки на оборудование появится с renderer устройства.",
        "Appearance is saved in the layout. Hardware image and lighting output requires the device renderer.",
      )}
    </p>
  </div>
{/if}
