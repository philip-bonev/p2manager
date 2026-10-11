/*
 * Copyright 2026 Philip Bonev
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://apache.org
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { t, applyStatic } from "/i18n.js";
import { initAppearance } from "/appearance.js";

const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

applyStatic();

const win = getCurrentWindow();
win.setTitle(t("help.title")).catch(() => {});
initAppearance().catch(() => {});

document.querySelector("#btn-close").addEventListener("mousedown", () => {
  win.close().catch(() => {});
});

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") {
    ev.preventDefault();
    win.close().catch(() => {});
  }
});

document.querySelector("#help-text").textContent = t("help.text");

invoke("get_about_info").then((info) => {
  const a = info || {};
  document.querySelector("#app-version").textContent = a.version || "";
  const dateEl = document.querySelector("#app-date");
  dateEl.textContent = a.date ? ` · ${t("help.releaseDate")}: ${a.date}` : "";
  document.querySelector("#license-text").textContent = a.license || "";
}).catch(() => {});

document.querySelectorAll(".about-tab").forEach((tab) => {
  tab.addEventListener("mousedown", () => {
    document.querySelectorAll(".about-tab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".about-tab-panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    document.querySelector(`.about-tab-panel[data-panel="${tab.dataset.tab}"]`).classList.add("active");
  });
});
