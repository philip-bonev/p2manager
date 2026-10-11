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
const { emit } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

applyStatic();

const win = getCurrentWindow();
win.setTitle(t("permissions.title")).catch(() => {});
initAppearance().catch(() => {});

const pathEl = document.querySelector("#p-path");
const hintEl = document.querySelector("#p-hint");
const applyButton = document.querySelector("#p-apply");
const checks = Array.from(document.querySelectorAll("[data-bit]"));
let currentPath = "";

document.querySelector("#btn-close").addEventListener("mousedown", () => {
  win.close().catch(() => {});
});

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") {
    ev.preventDefault();
    win.close().catch(() => {});
  }
});

async function loadPermissions(path) {
  if (!path) return;
  currentPath = path;
  pathEl.textContent = path;
  hintEl.textContent = "";
  applyButton.disabled = true;
  checks.forEach((check) => {
    check.checked = false;
    check.disabled = true;
  });

  try {
    const result = await invoke("get_file_permissions", { path });
    if (!result.supported) {
      hintEl.textContent = t("permissions.unsupported");
      return;
    }
    checks.forEach((check) => {
      const bit = Number(check.dataset.bit);
      check.checked = (result.mode & bit) !== 0;
      check.disabled = false;
    });
    applyButton.disabled = false;
  } catch (err) {
    hintEl.textContent = String(err);
  }
}

applyButton.addEventListener("mousedown", async () => {
  if (!currentPath) return;
  const mode = checks.reduce((value, check) => {
    return check.checked ? value | Number(check.dataset.bit) : value;
  }, 0);
  applyButton.disabled = true;
  hintEl.textContent = "";
  try {
    await invoke("set_file_permissions", { path: currentPath, mode });
    hintEl.textContent = t("permissions.saved");
  } catch (err) {
    hintEl.textContent = String(err);
  } finally {
    applyButton.disabled = false;
  }
});

win.listen("permissions-open", ({ payload }) => loadPermissions(payload && payload.path)).catch(() => {});
emit("permissions-window-ready").catch(() => {});
