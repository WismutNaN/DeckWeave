// SPDX-License-Identifier: GPL-3.0-or-later
import { mount } from "svelte";
import App from "./App.svelte";
import "./style.css";

const target = document.getElementById("app");
if (!target) throw new Error("Отсутствует корневой элемент приложения");
mount(App, { target });
