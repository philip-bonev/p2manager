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

const { invoke } = window.__TAURI__.core;
const { open: openDialog } = window.__TAURI__.dialog;
const { getCurrentWindow, PhysicalSize } = window.__TAURI__.window;

document.documentElement.classList.add("settings-root");

const appWindow = getCurrentWindow();
let fitDebounce = null;

async function fitWindow() {
  try {
    const contentW = document.documentElement.scrollWidth;
    const inner = await appWindow.innerSize();
    const outer = await appWindow.outerSize();
    const borderW = Math.max(0, outer.width - inner.width);
    const scale = window.devicePixelRatio || 1;
    const size = await appWindow.size();
    await appWindow.setSize(
      new PhysicalSize(
        Math.max(360, Math.ceil(contentW * scale + borderW)),
        size.height
      )
    );
  } catch {}
}

function scheduleFit() {
  clearTimeout(fitDebounce);
  fitDebounce = setTimeout(fitWindow, 100);
}

const SIZES = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28];

const FONT_MAP = {
  default: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
  monospace: "Menlo, Monaco, Consolas, 'DejaVu Sans Mono', monospace",
  serif: "Georgia, 'Times New Roman', serif",
  sans: "'Segoe UI', 'Helvetica Neue', Arial, sans-serif",
};

const rootEl = document.documentElement;
const themeRadios = document.querySelectorAll('input[name="theme"]');
const fontSelect = document.querySelector("#font");
const sizeSelect = document.querySelector("#font-size");
const diffCommand = document.querySelector("#diff-command");
const diffTerminal = document.querySelector("#diff-terminal");
const diffBrowse = document.querySelector("#diff-browse");
const editCommand = document.querySelector("#edit-command");
const editTerminal = document.querySelector("#edit-terminal");
const editBrowse = document.querySelector("#edit-browse");
const showHidden = document.querySelector("#show-hidden");

SIZES.forEach((s) => {
  const opt = document.createElement("option");
  opt.value = String(s);
  opt.textContent = t("sizePx", { size: s });
  sizeSelect.appendChild(opt);
});

function applyAppearance(appearance) {
  rootEl.classList.toggle("theme-dark", appearance.theme === "dark");
  rootEl.classList.toggle("theme-light", appearance.theme === "light");
  rootEl.style.setProperty("--font", FONT_MAP[appearance.font] || FONT_MAP.default);
  rootEl.style.setProperty("--font-size", `${appearance.fontSize}px`);
  scheduleFit();
}

function setControlValue(appearance) {
  const themeRadio = document.querySelector(`input[name="theme"][value="${appearance.theme}"]`);
  if (themeRadio) themeRadio.checked = true;
  fontSelect.value = appearance.font;
  sizeSelect.value = String(appearance.fontSize);
  diffCommand.value = appearance.diffCommand || "";
  diffTerminal.checked = !!appearance.diffInTerminal;
  editCommand.value = appearance.editCommand || "";
  editTerminal.checked = !!appearance.editInTerminal;
  showHidden.checked = appearance.showHidden !== false;
}

async function init() {
  applyStatic();
  const appearance = await invoke("get_appearance").catch(() => null);
  if (!appearance) return;
  setControlValue(appearance);
  applyAppearance(appearance);
}

themeRadios.forEach((radio) => {
  radio.addEventListener("change", async () => {
    if (!radio.checked) return;
    const appearance = await invoke("set_theme", { theme: radio.value }).catch(() => null);
    if (appearance) applyAppearance(appearance);
  });
});

fontSelect.addEventListener("change", async () => {
  const appearance = await invoke("set_font", { font: fontSelect.value }).catch(() => null);
  if (appearance) applyAppearance(appearance);
});

sizeSelect.addEventListener("change", async () => {
  const appearance = await invoke("set_font_size", { fontSize: Number(sizeSelect.value) }).catch(() => null);
  if (appearance) applyAppearance(appearance);
});

let diffDebounce = null;
diffCommand.addEventListener("input", () => {
  clearTimeout(diffDebounce);
  diffDebounce = setTimeout(async () => {
    await invoke("set_diff_command", { command: diffCommand.value }).catch(() => {});
  }, 400);
});

diffTerminal.addEventListener("change", async () => {
  await invoke("set_diff_in_terminal", { inTerminal: diffTerminal.checked }).catch(() => {});
});

function quotePath(p) {
  return /[\s"']/.test(p) ? `"${p.replace(/"/g, '\\"')}"` : p;
}

function bindBrowse(btn, input, placeholder) {
  btn.addEventListener("mousedown", async () => {
    const picked = await openDialog({ multiple: false, title: t("dialog.pickApp") }).catch(() => null);
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    const quoted = quotePath(path);
    if (input.value.trim() === "" || input.value.includes("%1") || input.value.includes("%2")) {
      input.value = `${quoted} ${placeholder}`.trim();
    } else {
      input.value = `${quoted}`;
    }
    input.dispatchEvent(new Event("input"));
  });
}

bindBrowse(diffBrowse, diffCommand, "%1 %2");
bindBrowse(editBrowse, editCommand, "%1");

let editDebounce = null;
editCommand.addEventListener("input", () => {
  clearTimeout(editDebounce);
  editDebounce = setTimeout(async () => {
    await invoke("set_edit_command", { command: editCommand.value }).catch(() => {});
  }, 400);
});

editTerminal.addEventListener("change", async () => {
  await invoke("set_edit_in_terminal", { inTerminal: editTerminal.checked }).catch(() => {});
});

showHidden.addEventListener("change", async () => {
  await invoke("set_show_hidden", { showHidden: showHidden.checked }).catch(() => {});
});

init();
