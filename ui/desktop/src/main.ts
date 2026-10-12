// SPDX-License-Identifier: GPL-3.0-or-later
import { mount } from "svelte";
import App from "./App.svelte";
import "./style.css";

import { applyTheme, readTheme } from "./editor/theme";
const systemTheme = matchMedia("(prefers-color-scheme: dark)");
const refreshTheme = () =>
  applyTheme(readTheme(localStorage), systemTheme.matches);
refreshTheme();
systemTheme.addEventListener("change", refreshTheme);
window.addEventListener("storage", refreshTheme);
const target = document.getElementById("app");
if (!target) throw new Error("Отсутствует корневой элемент приложения");
mount(App, { target });
