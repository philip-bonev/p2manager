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
const { open: openDialog } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

applyStatic();

const win = getCurrentWindow();
win.setTitle(t("settings.title")).catch(() => {});
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

function quotePath(p) {
  return /[\s"']/.test(p) ? `"${p.replace(/"/g, '\\"')}"` : p;
}

const SIZES = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28];
const sizeSelect = document.querySelector("#set-font-size");
SIZES.forEach((s) => {
  const o = document.createElement("option");
  o.value = String(s);
  o.textContent = t("sizePx", { size: s });
  sizeSelect.appendChild(o);
});

const fontSelect = document.querySelector("#set-font");
const showHiddenCb = document.querySelector("#set-show-hidden");
const diffInput = document.querySelector("#set-diff");
const diffTermCb = document.querySelector("#set-diff-terminal");
const diffBrowse = document.querySelector("#set-diff-browse");
const editInput = document.querySelector("#set-edit");
const editTermCb = document.querySelector("#set-edit-terminal");
const editBrowse = document.querySelector("#set-edit-browse");
const themeRadios = document.querySelectorAll('input[name="theme"]');

invoke("get_appearance").then((a) => {
  if (!a) return;
  const r = document.querySelector(`input[name="theme"][value="${a.theme}"]`);
  if (r) r.checked = true;
  fontSelect.value = a.font;
  sizeSelect.value = String(a.fontSize);
  diffInput.value = a.diffCommand || "";
  diffTermCb.checked = !!a.diffInTerminal;
  editInput.value = a.editCommand || "";
  editTermCb.checked = !!a.editInTerminal;
  showHiddenCb.checked = a.showHidden !== false;
}).catch(() => {});

themeRadios.forEach((radio) => {
  radio.addEventListener("change", async () => {
    if (!radio.checked) return;
    try { await invoke("set_theme", { theme: radio.value }); } catch {}
  });
});
fontSelect.addEventListener("change", async () => {
  try { await invoke("set_font", { font: fontSelect.value }); } catch {}
});
sizeSelect.addEventListener("change", async () => {
  try { await invoke("set_font_size", { fontSize: Number(sizeSelect.value) }); } catch {}
});
showHiddenCb.addEventListener("change", async (ev) => {
  try { await invoke("set_show_hidden", { showHidden: ev.target.checked }); } catch {}
});

let diffDebounce = null;
diffInput.addEventListener("input", () => {
  clearTimeout(diffDebounce);
  diffDebounce = setTimeout(async () => {
    try { await invoke("set_diff_command", { command: diffInput.value }); } catch {}
  }, 400);
});
diffTermCb.addEventListener("change", async (ev) => {
  try { await invoke("set_diff_in_terminal", { inTerminal: ev.target.checked }); } catch {}
});
diffBrowse.addEventListener("mousedown", async () => {
  try {
    const picked = await openDialog({ multiple: false, title: t("dialog.pickApp") });
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    const quoted = quotePath(path);
    if (diffInput.value.trim() === "" || diffInput.value.includes("%1") || diffInput.value.includes("%2")) {
      diffInput.value = `${quoted} %1 %2`.trim();
    } else {
      diffInput.value = quoted;
    }
    diffInput.dispatchEvent(new Event("input"));
  } catch {}
});

let editDebounce = null;
editInput.addEventListener("input", () => {
  clearTimeout(editDebounce);
  editDebounce = setTimeout(async () => {
    try { await invoke("set_edit_command", { command: editInput.value }); } catch {}
  }, 400);
});
editTermCb.addEventListener("change", async (ev) => {
  try { await invoke("set_edit_in_terminal", { inTerminal: ev.target.checked }); } catch {}
});
editBrowse.addEventListener("mousedown", async () => {
  try {
    const picked = await openDialog({ multiple: false, title: t("dialog.pickApp") });
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    const quoted = quotePath(path);
    if (editInput.value.trim() === "" || editInput.value.includes("%1")) {
      editInput.value = `${quoted} %1`.trim();
    } else {
      editInput.value = quoted;
    }
    editInput.dispatchEvent(new Event("input"));
  } catch {}
});
