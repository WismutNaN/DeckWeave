// SPDX-License-Identifier: GPL-3.0-or-later
export type ThemePreference = {
  mode: "system" | "light" | "dark";
  accent: string;
};
export const THEME_KEY = "deckweave.appearance.v1";
export const defaultTheme: ThemePreference = {
  mode: "system",
  accent: "#267567",
};
export function readTheme(storage: Pick<Storage, "getItem">): ThemePreference {
  try {
    const value = JSON.parse(storage.getItem(THEME_KEY) ?? "null");
    if (
      ["system", "light", "dark"].includes(value?.mode) &&
      /^#[0-9a-f]{6}$/i.test(value?.accent)
    )
      return value;
  } catch {
    /* Повреждённая настройка не мешает открыть workspace. */
  }
  return { ...defaultTheme };
}
const rgb = (hex: string) =>
  [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
function luminance(hex: string) {
  const values = rgb(hex)
    .map((c) => c / 255)
    .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
}
export function contrast(a: string, b: string) {
  const [x, y] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (x + 0.05) / (y + 0.05);
}
export function accentTokens(accent: string, dark: boolean) {
  const original = /^#[0-9a-f]{6}$/i.test(accent)
    ? accent
    : defaultTheme.accent;
  const base = rgb(original),
    background = dark ? "#20272d" : "#ffffff";
  let color = original;
  for (let step = 1; contrast(color, background) < 4.5 && step <= 100; step++) {
    color =
      "#" +
      base
        .map((c) =>
          Math.round(c + (((dark ? 255 : 0) - c) * step) / 100)
            .toString(16)
            .padStart(2, "0"),
        )
        .join("");
  }
  return {
    color,
    foreground: contrast(color, "#ffffff") >= 4.5 ? "#ffffff" : "#101519",
  };
}
export function applyTheme(
  value: ThemePreference,
  systemDark: boolean,
  target = document.documentElement,
) {
  const dark = value.mode === "dark" || (value.mode === "system" && systemDark);
  const tokens = accentTokens(value.accent, dark);
  target.dataset.theme = dark ? "dark" : "light";
  target.style.setProperty("--accent", tokens.color);
  target.style.setProperty("--on-accent", tokens.foreground);
  target.style.colorScheme = dark ? "dark" : "light";
}
