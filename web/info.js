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
win.setTitle(t("info.title")).catch(() => {});
initAppearance().catch(() => {});

const table = document.querySelector("#i-table");
const hint = document.querySelector("#i-hint");

document.querySelector("#btn-close").addEventListener("mousedown", () => {
  win.close().catch(() => {});
});

function fmtSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let u = -1;
  do {
    v /= 1024;
    u += 1;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[u]}`;
}

function fmtDate(secs) {
  if (!secs) return "";
  const d = new Date(secs * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

async function loadInfo(path) {
  if (!path) return;
  try {
    const info = await invoke("path_info", { path });
    const rows = [
      [t("info.name"), info.name],
      [t("info.path"), info.path],
      [t("info.type"), info.isDir ? t("info.typeDir") : t("info.typeFile")],
      [t("info.size"), info.isDir ? "—" : fmtSize(info.size)],
      [t("info.modified"), fmtDate(info.modified)],
      [t("info.created"), fmtDate(info.created)],
      [t("info.permissions"), info.permissions],
    ];
    table.innerHTML = "";
    for (const [k, v] of rows) {
      const tr = document.createElement("tr");
      const tdKey = document.createElement("td");
      tdKey.textContent = k;
      const tdVal = document.createElement("td");
      tdVal.textContent = v == null ? "" : String(v);
      tr.append(tdKey, tdVal);
      table.appendChild(tr);
    }
    hint.textContent = "";
  } catch (err) {
    table.innerHTML = "";
    hint.textContent = String(err);
  }
}

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") {
    ev.preventDefault();
    win.close().catch(() => {});
  }
});

win.listen("info-open", ({ payload }) => loadInfo(payload && payload.path)).catch(() => {});
emit("info-window-ready").catch(() => {});
