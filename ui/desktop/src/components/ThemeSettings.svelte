<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    readTheme,
    applyTheme,
    THEME_KEY,
    defaultTheme,
    type ThemePreference,
  } from "../editor/theme";
  let { lang }: { lang: "ru" | "en" } = $props();
  let preference = $state<ThemePreference>({ ...defaultTheme });
  let error = $state("");
  const t = (ru: string, en: string) => (lang === "ru" ? ru : en);
  onMount(() => {
    preference = readTheme(localStorage);
  });
  function change(next: ThemePreference) {
    preference = next;
    applyTheme(next, matchMedia("(prefers-color-scheme: dark)").matches);
    try {
      localStorage.setItem(THEME_KEY, JSON.stringify(next));
      error = "";
    } catch {
      error = t(
        "Настройка изменена, но не сохранена: хранилище недоступно.",
        "Changed, but not saved: storage unavailable.",
      );
    }
  }
</script>

<label class="form-field"
  ><span>{t("Тема", "Theme")}</span><select
    value={preference.mode}
    onchange={(e) =>
      change({
        ...preference,
        mode: e.currentTarget.value as ThemePreference["mode"],
      })}
  >
    <option value="system">{t("Как в системе", "System")}</option><option
      value="light">{t("Светлая", "Light")}</option
    ><option value="dark">{t("Тёмная", "Dark")}</option>
  </select></label
>
<label class="form-field"
  ><span>{t("Акцентный цвет", "Accent color")}</span><input
    type="color"
    value={preference.accent}
    oninput={(e) => change({ ...preference, accent: e.currentTarget.value })}
  /></label
>
<div class="theme-swatches">
  {#each ["#267567", "#3b69c4", "#8661c5", "#b04873", "#b36b21"] as color}<button
      aria-label={t("Акцент", "Accent") + " " + color}
      style:background={color}
      onclick={() => change({ ...preference, accent: color })}
    ></button>{/each}
</div>
{#if error}<p role="alert" class="invalid">{error}</p>{/if}
