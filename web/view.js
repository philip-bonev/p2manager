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
win.setTitle(t("view.window")).catch(() => {});

const titleEl = document.querySelector("#v-title");
const pre = document.querySelector("#v-pre");
const hint = document.querySelector("#v-hint");
const wrapCheckbox = document.querySelector("#v-wrap");
const measureCanvas = document.createElement("canvas");
const measureContext = measureCanvas.getContext("2d");

initAppearance(() => renderViewerText()).catch(() => {});

wrapCheckbox.addEventListener("change", () => {
  pre.classList.toggle("word-wrap", wrapCheckbox.checked);
  renderViewerText();
});

document.querySelector("#btn-close").addEventListener("mousedown", () => {
  win.close().catch(() => {});
});

const VIEW_CHUNK = 256 * 1024;
let viewState = null;
let loading = false;

function renderViewerText() {
  if (!viewState) return;
  const scrollTop = pre.scrollTop;
  if (!wrapCheckbox.checked) {
    pre.textContent = viewState.text;
    pre.scrollTop = scrollTop;
    return;
  }

  const style = getComputedStyle(pre);
  if (measureContext) measureContext.font = style.font;
  const availableWidth = Math.max(
    1,
    pre.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)
  );
  const lines = viewState.text.split("\n");
  const fragment = document.createDocumentFragment();
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].endsWith("\r") ? lines[i].slice(0, -1) : lines[i];
    const text = document.createTextNode(line);
    fragment.appendChild(text);
    const measured = line.replace(/\t/g, "        ");
    if (measureContext && measureContext.measureText(measured).width > availableWidth) {
      const marker = document.createElement("span");
      marker.className = "wrap-marker";
      marker.setAttribute("aria-hidden", "true");
      fragment.appendChild(marker);
    }
    if (i < lines.length - 1) fragment.appendChild(document.createElement("br"));
  }
  pre.replaceChildren(fragment);
  pre.scrollTop = scrollTop;
}

async function loadFile(path) {
  if (!path) return;
  try {
    const info = await invoke("path_info", { path }).catch(() => null);
    const fileSize = info ? info.size : 0;
    const firstChunk = await invoke("read_file_chunk", { path, offset: 0, limit: VIEW_CHUNK });
    viewState = { path, fileSize, loaded: firstChunk.length, text: firstChunk };
    pre.classList.toggle("word-wrap", wrapCheckbox.checked);
    renderViewerText();
    hint.textContent = "";
    const name = path.split(/[\\/]/).pop() || path;
    const title = t("view.title", { name });
    titleEl.textContent = title;
    win.setTitle(title).catch(() => {});
    pre.focus();
  } catch (err) {
    viewState = null;
    pre.textContent = "";
    hint.textContent = String(err);
  }
}

pre.addEventListener("scroll", async () => {
  if (!viewState || loading) return;
  const nearBottom = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 200;
  if (!nearBottom || viewState.loaded >= viewState.fileSize) return;
  loading = true;
  try {
    const chunk = await invoke("read_file_chunk", {
      path: viewState.path,
      offset: viewState.loaded,
      limit: VIEW_CHUNK,
    });
    if (chunk.length > 0) {
      viewState.text += chunk;
      viewState.loaded += chunk.length;
      renderViewerText();
    }
  } catch (err) {
    hint.textContent = String(err);
  } finally {
    loading = false;
  }
});

new ResizeObserver(() => renderViewerText()).observe(pre);

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") {
    ev.preventDefault();
    win.close().catch(() => {});
  }
});

win.listen("view-open", ({ payload }) => loadFile(payload && payload.path)).catch(() => {});
emit("view-window-ready").catch(() => {});
