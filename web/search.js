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
win.setTitle(t("searchFiles.title")).catch(() => {});
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

const baseInput = document.querySelector("#s-base");
const exclInput = document.querySelector("#s-excl");
const patternInput = document.querySelector("#s-pattern");
const igCase = document.querySelector("#s-igcase");
const rec = document.querySelector("#s-rec");
const contentEnable = document.querySelector("#s-content-enable");
const contentPanel = document.querySelector("#s-content-panel");
const contentInput = document.querySelector("#s-content-pattern");
const cIg = document.querySelector("#s-c-igcase");
const results = document.querySelector("#s-results");
const hint = document.querySelector("#s-hint");
const searchBtn = document.querySelector("#s-search");
const cancelBtn = document.querySelector("#s-cancel");

exclInput.placeholder = t("searchFiles.exclusionsHint");
patternInput.placeholder = t("searchFiles.patternHint");

function closeWin() {
  win.close().catch(() => {});
}

cancelBtn.addEventListener("mousedown", closeWin);

contentEnable.addEventListener("change", () => {
  const en = contentEnable.checked;
  contentPanel.style.opacity = en ? "1" : "0.5";
  contentPanel.style.pointerEvents = en ? "auto" : "none";
});
contentPanel.style.opacity = "0.5";
contentPanel.style.pointerEvents = "none";

const chooseResult = async (idx) => {
  const path = results.children[idx]?.dataset?.path;
  if (!path) return;
  emit("search-navigate", { path }).catch(() => {});
  closeWin();
};

results.addEventListener("keydown", (ev) => {
  const items = results.querySelectorAll("li:not(.empty)");
  if (items.length === 0) return;
  let sel = Array.from(items).findIndex((li) => li.classList.contains("selected"));
  if (ev.key === "ArrowDown") {
    ev.preventDefault();
    sel = Math.min(items.length - 1, sel + 1);
    items.forEach((li, i) => li.classList.toggle("selected", i === sel));
    items[sel].scrollIntoView({ block: "nearest" });
  } else if (ev.key === "ArrowUp") {
    ev.preventDefault();
    sel = Math.max(0, sel - 1);
    items.forEach((li, i) => li.classList.toggle("selected", i === sel));
    items[sel].scrollIntoView({ block: "nearest" });
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    const idx = Array.from(results.children).findIndex((li) => li.classList.contains("selected"));
    if (idx >= 0) chooseResult(idx);
  }
});

results.addEventListener("dblclick", (ev) => {
  const li = ev.target.closest("li");
  if (!li || li.classList.contains("empty")) return;
  const idx = Array.from(results.children).indexOf(li);
  chooseResult(idx);
});
results.addEventListener("mousedown", (ev) => {
  const li = ev.target.closest("li");
  if (!li || li.classList.contains("empty")) return;
  results.querySelectorAll("li").forEach((x) => x.classList.remove("selected"));
  li.classList.add("selected");
});

async function doSearch() {
  const params = {
    basePath: baseInput.value.trim(),
    exclusions: exclInput.value.trim(),
    pattern: patternInput.value,
    patternMode: document.querySelector('input[name="fileMode"]:checked')?.value || "glob",
    ignoreCase: igCase.checked,
    recursive: rec.checked,
    contentEnabled: contentEnable.checked,
    contentPattern: contentInput.value,
    contentMode: document.querySelector('input[name="contentMode"]:checked')?.value || "text",
    contentIgnoreCase: cIg.checked,
  };
  if (!params.basePath) {
    hint.textContent = t("err.noSelection");
    return;
  }
  searchBtn.disabled = true;
  searchBtn.textContent = t("searchFiles.searching");
  results.innerHTML = "";
  hint.textContent = "";
  try {
    const found = await invoke("search_files", { params });
    results.innerHTML = "";
    if (found.length === 0) {
      const li = document.createElement("li");
      li.className = "empty";
      li.textContent = t("searchFiles.noResults");
      results.appendChild(li);
      hint.textContent = "";
    } else {
      found.forEach((p, i) => {
        const li = document.createElement("li");
        li.textContent = p;
        li.dataset.path = p;
        if (i === 0) li.classList.add("selected");
        li.addEventListener("mousedown", (ev) => {
          ev.preventDefault();
          results.querySelectorAll("li").forEach((x) => x.classList.remove("selected"));
          li.classList.add("selected");
        });
        li.addEventListener("dblclick", () => chooseResult(i));
        results.appendChild(li);
      });
      hint.textContent = t("searchFiles.resultsCount", { count: found.length });
      results.focus();
    }
  } catch (err) {
    hint.textContent = String(err);
  } finally {
    searchBtn.disabled = false;
    searchBtn.textContent = t("searchFiles.search");
  }
}

searchBtn.addEventListener("mousedown", doSearch);
patternInput.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") {
    ev.preventDefault();
    doSearch();
  }
});

win
  .listen("search-open", ({ payload }) => {
    baseInput.value = (payload && payload.basePath) || "";
    patternInput.focus();
    patternInput.select();
  })
  .catch(() => {});

emit("search-window-ready").catch(() => {});
patternInput.focus();
