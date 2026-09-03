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

import { t, tp, applyStatic } from "/i18n.js";

const { invoke } = window.__TAURI__.core;
const { open: openDialog } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

applyStatic();

const rootEl = document.documentElement;
let themePref = "system";
let currentOsTheme = null;
let showHidden = true;
const isMac = navigator.platform.startsWith("Mac") || navigator.userAgent.includes("Mac");
let clipboard = { files: [], isCut: false };

const FONT_MAP = {
  default: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
  monospace: "Menlo, Monaco, Consolas, 'DejaVu Sans Mono', monospace",
  serif: "Georgia, 'Times New Roman', serif",
  sans: "'Segoe UI', 'Helvetica Neue', Arial, sans-serif",
};

function setThemeClass(theme) {
  rootEl.classList.toggle("theme-dark", theme === "dark");
  rootEl.classList.toggle("theme-light", theme === "light");
}

function applyTheme() {
  if (themePref === "light") {
    setThemeClass("light");
  } else if (themePref === "dark") {
    setThemeClass("dark");
  } else {
    setThemeClass(currentOsTheme);
  }
}

function applyAppearance(appearance) {
  rootEl.style.setProperty("--font", FONT_MAP[appearance.font] || FONT_MAP.default);
  rootEl.style.setProperty("--font-size", `${appearance.fontSize}px`);
}

async function syncAppearance() {
  const appearance = await invoke("get_appearance").catch(() => null);
  if (appearance) {
    themePref = appearance.theme;
    applyAppearance(appearance);
    const nextShowHidden = appearance.showHidden !== false;
    if (nextShowHidden !== showHidden) {
      showHidden = nextShowHidden;
      if (state.left.path) refresh("left");
      if (state.right.path) refresh("right");
    } else {
      showHidden = nextShowHidden;
    }
    fuzzyPref = !!appearance.fuzzySearch;
  }
  try {
    currentOsTheme = await getCurrentWindow().theme();
  } catch {
    currentOsTheme = null;
  }
  if (!currentOsTheme) {
    currentOsTheme =
      window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
  }
  applyTheme();
}

const state = {
  left: { path: null, parent: null, items: [], selected: 0, marked: new Set(), selAnchor: 0, sortKey: "name", sortDir: "asc" },
  right: { path: null, parent: null, items: [], selected: 0, marked: new Set(), selAnchor: 0, sortKey: "name", sortDir: "asc" },
};
let activeSide = "left";
let fuzzyPref = false;

const lists = {
  left: document.querySelector("#panel-left .panel-list"),
  right: document.querySelector("#panel-right .panel-list"),
};
const heads = {
  left: document.querySelector("#panel-left .panel-head"),
  right: document.querySelector("#panel-right .panel-head"),
};
const cols = {
  left: document.querySelector("#panel-left .panel-cols"),
  right: document.querySelector("#panel-right .panel-cols"),
};
const foots = {
  left: document.querySelector("#panel-left .panel-foot"),
  right: document.querySelector("#panel-right .panel-foot"),
};
const overlay = document.querySelector("#modal-overlay");
const modalTitle = document.querySelector("#modal-title");
const modalBody = document.querySelector("#modal-body");
const modalActions = document.querySelector("#modal-actions");
const statusbar = document.querySelector("#statusbar");
const nameTooltip = document.querySelector("#name-tooltip");
let tooltipTimer = null;

function showNameTooltip(el, text, ms, fromMouse, ev) {
  hideNameTooltip();
  if (!text) return;
  nameTooltip.innerHTML = "";
  const item = document.createElement("span");
  item.className = "tt-item";
  item.textContent = text;
  nameTooltip.appendChild(item);
  nameTooltip.style.display = "block";
  if (fromMouse && ev) {
    nameTooltip.style.left = Math.min(ev.clientX + 12, window.innerWidth - nameTooltip.offsetWidth - 8) + "px";
    nameTooltip.style.top = (ev.clientY + 18) + "px";
  } else {
    const r = el.getBoundingClientRect();
    nameTooltip.style.left = Math.min(r.left, window.innerWidth - nameTooltip.offsetWidth - 8) + "px";
    nameTooltip.style.top = (r.bottom + 2) + "px";
  }
  if (ms) tooltipTimer = setTimeout(hideNameTooltip, ms);
}

function hideNameTooltip() {
  if (tooltipTimer) { clearTimeout(tooltipTimer); tooltipTimer = null; }
  nameTooltip.style.display = "none";
}

const modalEl = overlay.querySelector(".modal");
modalTitle.addEventListener("dblclick", () => {
  modalEl.classList.toggle("maximized");
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

function fmtSpeed(bytesPerSec) {
  if (!bytesPerSec || bytesPerSec <= 0) return "—";
  return `${fmtSize(bytesPerSec)}/s`;
}

function fmtTime(secs) {
  secs = Math.max(0, Math.round(secs));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const p = (n) => String(n).padStart(2, "0");
  if (h > 0) return `${h}:${p(m)}:${p(s)}`;
  return `${m}:${p(s)}`;
}

function getActive() {
  return state[activeSide];
}

function getOther() {
  return state[activeSide === "left" ? "right" : "left"];
}

function sortedItems(side) {
  const s = state[side];
  const dir = s.sortDir === "asc" ? 1 : -1;
  const key = s.sortKey;
  const items = [...s.items];
  items.sort((a, b) => {
    const cmpDirs = b.isDir - a.isDir;
    if (cmpDirs !== 0) return cmpDirs;
    let r;
    if (key === "size") r = a.size - b.size;
    else if (key === "date") r = a.modified - b.modified;
    else r = a.name.toLowerCase() < b.name.toLowerCase() ? -1 : a.name.toLowerCase() > b.name.toLowerCase() ? 1 : 0;
    return r * dir;
  });
  return items;
}

function fullList(side) {
  const s = state[side];
  const rows = [];
  if (s.parent) rows.push({ kind: "parent" });
  for (const it of sortedItems(side)) rows.push({ kind: "item", entry: it });
  return rows;
}

function toggleSort(side, key) {
  const s = state[side];
  if (s.sortKey === key) {
    s.sortDir = s.sortDir === "asc" ? "desc" : "asc";
  } else {
    s.sortKey = key;
    s.sortDir = "asc";
  }
  render(side);
  updateCols(side);
}

function updateCols(side) {
  const s = state[side];
  if (!cols[side]) return;
  cols[side].querySelectorAll("span[data-sort]").forEach((sp) => {
    sp.classList.toggle("sort-active", sp.dataset.sort === s.sortKey);
    sp.classList.toggle("asc", sp.dataset.sort === s.sortKey && s.sortDir === "asc");
    sp.classList.toggle("desc", sp.dataset.sort === s.sortKey && s.sortDir === "desc");
  });
}

async function loadDir(side, path) {
  try {
    const listing = await invoke("list_dir", { path });
    const s = state[side];
    s.path = listing.path;
    s.parent = listing.parent ?? null;
    s.items = listing.items;
    s.selected = 0;
    s.marked = new Set();
    s.selAnchor = 0;
    render(side);
    heads[side].textContent = listing.path;
  } catch (err) {
    alertModal(t("err.title"), String(err));
    const s = state[side];
    if (s.path) {
      heads[side].textContent = s.path;
      render(side);
    }
  }
}

function render(side) {
  const list = lists[side];
  const s = state[side];
  list.innerHTML = "";
  const rows = fullList(side);
  s.rows = rows;

  rows.forEach((row, idx) => {
    const li = document.createElement("li");
    li.dataset.index = String(idx);

    if (row.kind === "parent") {
      li.classList.add("parent");
      const mark = document.createElement("span");
      mark.className = "mark";
      li.appendChild(mark);
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = "..";
      li.appendChild(name);
      const ext = document.createElement("span");
      ext.className = "ext";
      li.appendChild(ext);
      const date = document.createElement("span");
      date.className = "date";
      date.textContent = t("updir");
      li.appendChild(date);
    } else {
      const e = row.entry;
      if (e.isDir) li.classList.add("dir");
      if (s.marked.has(idx)) li.classList.add("marked");
      const mark = document.createElement("span");
      mark.className = "mark";
      mark.textContent = s.marked.has(idx) ? "*" : "";
      li.appendChild(mark);
      const name = document.createElement("span");
      name.className = "name";
      const dot = e.isDir ? -1 : e.name.lastIndexOf(".");
      if (dot > 0) {
        name.textContent = e.name.substring(0, dot);
        const ext = document.createElement("span");
        ext.className = "ext";
        ext.textContent = e.name.substring(dot + 1);
        li.appendChild(name);
        li.appendChild(ext);
      } else {
        name.textContent = e.name;
        const ext = document.createElement("span");
        ext.className = "ext";
        li.appendChild(name);
        li.appendChild(ext);
      }
      const size = document.createElement("span");
      size.className = "size";
      size.textContent = e.isDir ? "" : fmtSize(e.size);
      li.appendChild(size);
      const date = document.createElement("span");
      date.className = "date";
      date.textContent = fmtDate(e.modified);
      li.appendChild(date);
    }

    li.addEventListener("mousedown", (ev) => {
      ev.preventDefault();
      setActiveSide(side);
      select(side, idx, { shift: ev.shiftKey });
    });
    li.addEventListener("dblclick", (ev) => {
      ev.preventDefault();
      openRow(side, idx);
    });
    if (row.kind !== "parent") {
      li.addEventListener("mouseenter", (ev) => {
        showNameTooltip(li, row.entry.name, 0, true, ev);
      });
      li.addEventListener("mousemove", (ev) => {
        if (nameTooltip.style.display === "block") {
          nameTooltip.style.left = Math.min(ev.clientX + 12, window.innerWidth - nameTooltip.offsetWidth - 8) + "px";
          nameTooltip.style.top = (ev.clientY + 18) + "px";
        }
      });
      li.addEventListener("mouseleave", hideNameTooltip);
    }

    list.appendChild(li);
  });

  select(side, s.selected);
  updateFoot(side);
  updateCols(side);
}

function select(side, idx, opts = {}) {
  const s = state[side];
  const rows = s.rows || [];
  if (rows.length === 0) return;
  if (idx < 0) idx = 0;
  if (idx >= rows.length) idx = rows.length - 1;
  s.selected = idx;

  if (opts.shift) {
    const from = Math.min(s.selAnchor, idx);
    const to = Math.max(s.selAnchor, idx);
    for (let i = from; i <= to; i++) {
      const row = rows[i];
      if (row && row.kind !== "parent") s.marked.add(i);
    }
  } else {
    s.selAnchor = idx;
  }

  listItems(side).forEach((li, i) => {
    li.classList.toggle("selected", i === idx);
    const isMarked = s.marked.has(i);
    li.classList.toggle("marked", isMarked);
    const mark = li.querySelector(".mark");
    if (mark) mark.textContent = isMarked ? "*" : "";
  });

  const el = listItems(side)[idx];
  if (el) el.scrollIntoView({ block: "nearest" });

  hideNameTooltip();
  const row = rows[idx];
  if (row && row.kind !== "parent") {
    showNameTooltip(el, row.entry.name, 2500);
  }

  updateStatus();
}

function listItems(side) {
  return Array.from(lists[side].children);
}

function toggleMark(side, idx) {
  const s = state[side];
  const rows = s.rows || [];
  const row = rows[idx];
  if (!row || row.kind === "parent") return;
  if (s.marked.has(idx)) s.marked.delete(idx);
  else s.marked.add(idx);
  const li = listItems(side)[idx];
  if (li) {
    li.classList.toggle("marked", s.marked.has(idx));
    const mark = li.querySelector(".mark");
    if (mark) mark.textContent = li.classList.contains("marked") ? "*" : "";
  }
  updateStatus();
}

function markByPattern(side, pattern, mark) {
  const s = state[side];
  const rows = s.rows || [];
  let re;
  try {
    re = new RegExp(
      "^" + pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".") + "$",
      "i"
    );
  } catch {
    return 0;
  }
  let count = 0;
  rows.forEach((row, i) => {
    if (row.kind === "parent") return;
    if (re.test(row.entry.name)) {
      if (mark) s.marked.add(i);
      else s.marked.delete(i);
      count++;
    }
  });
  return count;
}

function invertMarks(side) {
  const s = state[side];
  const rows = s.rows || [];
  const newSet = new Set();
  rows.forEach((row, i) => {
    if (row.kind === "parent") return;
    if (!s.marked.has(i)) newSet.add(i);
  });
  s.marked = newSet;
  render(side);
}

function selectAll(side) {
  const s = state[side];
  const rows = s.rows || [];
  rows.forEach((row, i) => {
    if (row.kind === "parent") return;
    s.marked.add(i);
  });
  render(side);
}

function copyToClipboard(isCut) {
  const side = activeSide;
  const s = state[side];
  const rows = s.rows || [];
  const markedIdx = [...s.marked]
    .filter((i) => rows[i] && rows[i].kind === "item")
    .sort((a, b) => a - b);
  let files = [];
  if (markedIdx.length > 0) {
    files = markedIdx.map((i) => `${s.path.replace(/\/$/, "")}/${rows[i].entry.name}`);
  } else {
    const row = selectedRow(side);
    if (row && row.kind !== "parent") {
      files = [selectedPath(side)];
    }
  }
  if (files.length === 0) return;
  clipboard = { files, isCut };
  document.querySelector("#btn-paste").classList.toggle("has-clip", true);
  listItems(side).forEach((li, i) => {
    li.classList.toggle("cut", isCut && s.marked.has(i));
  });
  updateStatus();
}

async function pasteFromClipboard() {
  if (clipboard.files.length === 0) return;
  const side = activeSide;
  const s = state[side];
  if (!s.path) return;
  const wasCut = clipboard.isCut;
  const dest = s.path.replace(/\/$/, "");
  for (const src of clipboard.files) {
    try {
      if (wasCut) {
        await invoke("move_path", { src, dstDir: dest });
      } else {
        await invoke("copy_path", { src, dstDir: dest });
      }
    } catch (err) {
      alertModal(t("err.title"), String(err));
    }
  }
  if (wasCut) {
    clipboard = { files: [], isCut: false };
    document.querySelector("#btn-paste").classList.remove("has-clip");
  }
  refresh(side);
  const other = side === "left" ? "right" : "left";
  if (wasCut && state[other].path) refresh(other);
}

async function markPatternModal(mark) {
  const pattern = await promptModal(
    mark ? t("mark.titleMark") : t("mark.titleUnmark"),
    mark ? t("mark.promptMark") : t("mark.promptUnmark")
  );
  if (pattern === null) return;
  if (pattern.trim() === "") {
    const s = state[activeSide];
    const rows = s.rows || [];
    rows.forEach((row, i) => {
      if (row.kind === "parent") return;
      if (mark) s.marked.add(i);
      else s.marked.delete(i);
    });
    render(activeSide);
    return;
  }
  markByPattern(activeSide, pattern, mark);
  render(activeSide);
}

function setActiveSide(side) {
  activeSide = side;
  for (const s of ["left", "right"]) {
    heads[s].classList.toggle("active", s === side);
  }
  lists[side].focus?.();
  updateStatus();
}

function updateFoot(side) {
  const s = state[side];
  const rows = s.rows || [];
  const total = rows.filter((r) => r.kind === "item").length;
  foots[side].textContent = tp("foot.items", total);
}

function updateStatus() {
  const act = getActive();
  const other = getOther();
  const rows = act.rows || [];
  const row = rows[act.selected];
  let sel = t("status.selectedNone");
  if (row) {
    if (row.kind === "parent") {
      sel = t("status.up", { path: act.parent });
    } else if (row.entry.isDir) {
      sel = t("status.dir", { name: row.entry.name });
    } else {
      sel = t("status.file", {
        name: row.entry.name,
        size: fmtSize(row.entry.size),
        date: fmtDate(row.entry.modified),
      });
    }
  }
  statusbar.textContent = `${t("status.active")}: ${act.path} | ${t("status.selected")}: ${sel} | ${t("status.other")}: ${other.path}`;
}

function selectedRow(side) {
  const s = state[side];
  const rows = s.rows || [];
  if (rows.length === 0) return null;
  const idx = Math.min(s.selected, rows.length - 1);
  return rows[idx];
}

function selectedPath(side) {
  const row = selectedRow(side);
  if (!row) return null;
  if (row.kind === "parent") return state[side].parent;
  const p = state[side].path;
  return `${p.replace(/\/$/, "")}/${row.entry.name}`;
}

function openRow(side, idx, forceEnter) {
  const s = state[side];
  const rows = s.rows || [];
  const row = rows[idx];
  if (!row) return;
  if (row.kind === "parent") {
    goUp(side);
    return;
  }
  const e = row.entry;
  const full = `${s.path.replace(/\/$/, "")}/${e.name}`;
  if (e.isDir) {
    const isMac = navigator.userAgent.toLowerCase().includes("mac");
    if (!forceEnter && isMac && e.name.toLowerCase().endsWith(".app")) {
      invoke("open_path", { path: full }).catch((err) =>
        alertModal(t("err.title"), String(err))
      );
      return;
    }
    loadDir(side, full);
  } else {
    invoke("open_path", { path: full }).catch((err) =>
      alertModal(t("err.title"), String(err))
    );
  }
}

async function goUp(side) {
  const s = state[side];
  if (s.parent) {
    const curPath = s.path || "";
    const curName = curPath.replace(/\\/g, "/").replace(/\/$/, "").split("/").pop() || "";
    await loadDir(side, s.parent);
    if (curName) {
      const rows = state[side].rows || [];
      const idx = rows.findIndex((r) => r.kind === "item" && r.entry.name === curName);
      if (idx >= 0) select(side, idx);
    }
  } else {
    alertModal(t("err.title"), t("err.top"));
  }
}

async function refresh(side) {
  if (state[side].path) loadDir(side, state[side].path);
}

function swapPanels() {
  const tmp = state.left;
  state.left = state.right;
  state.right = tmp;
  refresh("left");
  refresh("right");
}

function openSelectedIn(src, dst) {
  const s = state[src];
  const rows = s.rows || [];
  const row = rows[Math.min(s.selected, rows.length - 1)];
  if (!row || row.kind === "parent" || !row.entry.isDir) {
    loadDir(dst, s.path);
    return;
  }
  const full = `${s.path.replace(/\/$/, "")}/${row.entry.name}`;
  loadDir(dst, full);
}

function moveSelection(side, delta, shift) {
  const s = state[side];
  const count = (s.rows || []).length;
  if (count === 0) return;
  let idx = s.selected + delta;
  if (idx < 0) idx = 0;
  if (idx >= count) idx = count - 1;
  select(side, idx, { shift });
}

function pageStep(side) {
  const list = lists[side];
  const first = list.firstElementChild;
  if (!first) return 1;
  const rowHeight = first.getBoundingClientRect().height || 20;
  return Math.max(1, Math.floor(list.clientHeight / rowHeight));
}

async function copyOrMove(op) {
  const side = activeSide;
  const s = state[side];
  const rows = s.rows || [];
  const markedIdx = [...s.marked]
    .filter((i) => rows[i] && rows[i].kind === "item")
    .sort((a, b) => a - b);
  let targets = [];
  if (markedIdx.length > 0) {
    targets = markedIdx.map((i) => ({
      row: rows[i],
      path: `${s.path.replace(/\/$/, "")}/${rows[i].entry.name}`,
    }));
  } else {
    const row = selectedRow(side);
    if (row && row.kind !== "parent") {
      targets = [{ row, path: selectedPath(side) }];
    }
  }
  if (targets.length === 0) {
    alertModal(t("err.title"), t("err.noSelection"));
    return;
  }
  const other = getOther();
  if (!other.path) {
    alertModal(t("err.title"), t("err.noOtherDir"));
    return;
  }
  const verb = op === "copy" ? t("verb.copy") : t("verb.move");
  const checkboxes =
    op === "copy"
      ? [
          { id: "hard", label: t("link.hard"), checked: false },
          { id: "soft", label: t("link.soft"), checked: false },
        ]
      : [];
  const name =
    targets.length === 1
      ? targets[0].row.entry.name
      : t("copyMove.multi", { count: targets.length });
  const res = await confirmModal(
    verb,
    t("copyMove.confirm", {
      verb,
      name,
      src: s.path,
      dst: other.path,
    }),
    checkboxes
  );
  if (!res.ok) return;
  const links = op === "copy" && (res.values.hard || res.values.soft);
  const id =
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random()}`;
  const ctrl = openProgressModal(verb, id);
  progressCtrl = ctrl;
  const totalBytes = targets.reduce((s, t) => s + (t.row.entry.size || 0), 0);
  const startTime = Date.now();
  let doneBytes = 0;
  let lastSample = null;
  let speed = 0;
  const updateOverall = (extra) => {
    const done = doneBytes + (extra || 0);
    const pct = totalBytes > 0 ? Math.round((done / totalBytes) * 100) : 0;
    ctrl.setOverall(
      t("copy.overall", {
        done: fmtSize(done),
        total: fmtSize(totalBytes),
        pct,
      }),
      done,
      totalBytes
    );
  };
  const poll = setInterval(async () => {
    try {
      const p = await invoke("get_copy_progress", { id });
      if (!p) return;
      const now = Date.now();
      if (lastSample && p.copied > lastSample.copied) {
        const dt = (now - lastSample.t) / 1000;
        if (dt > 0) speed = (p.copied - lastSample.copied) / dt;
      }
      lastSample = { t: now, copied: p.copied };
      const pct = p.total > 0 ? Math.round((p.copied / p.total) * 100) : 0;
      const remaining = p.total > 0 && speed > 0 ? (p.total - p.copied) / speed : 0;
      const stats = t("copy.curStats", {
        pct,
        speed: fmtSpeed(speed),
        eta: t("copy.eta", { v: fmtTime(remaining) }),
        elapsed: t("copy.elapsed", { v: fmtTime((now - startTime) / 1000) }),
      });
      ctrl.setCurrent(p.path, p.copied, p.total, stats);
      updateOverall(p.copied);
    } catch (err) {}
  }, 150);
  for (let i = 0; i < targets.length; i++) {
    if (ctrl.cancelled) break;
    const t = targets[i];
    lastSample = null;
    speed = 0;
    ctrl.setCurrent(t.row.entry.name, 0, 0, "");
    updateOverall(0);
    try {
      if (links) {
        await invoke("link_path", {
          src: t.path,
          dstDir: other.path,
          hard: !!res.values.hard,
        });
      } else {
        await invoke(op === "copy" ? "copy_path_progress" : "move_path_progress", {
          src: t.path,
          dstDir: other.path,
          id,
        });
      }
    } catch (err) {
      if (String(err).includes("CANCELLED")) break;
      alertModal(t("err.title"), String(err));
      break;
    }
    doneBytes += t.row.entry.size || 0;
    updateOverall(0);
  }
  clearInterval(poll);
  progressCtrl = null;
  ctrl.close();
  refresh(side);
  refresh(activeSide === "left" ? "right" : "left");
}

async function deleteSelected() {
  const side = activeSide;
  const s = state[side];
  const rows = s.rows || [];
  const markedIdx = [...s.marked]
    .filter((i) => rows[i] && rows[i].kind === "item")
    .sort((a, b) => a - b);
  let targets = [];
  if (markedIdx.length > 0) {
    targets = markedIdx.map((i) => ({
      row: rows[i],
      path: `${s.path.replace(/\/$/, "")}/${rows[i].entry.name}`,
    }));
  } else {
    const row = selectedRow(side);
    if (row && row.kind !== "parent") {
      targets = [{ row, path: selectedPath(side) }];
    }
  }
  if (targets.length === 0) return;
  const name =
    targets.length === 1
      ? targets[0].row.entry.name
      : t("copyMove.multi", { count: targets.length });
  const res = await confirmModal(t("delete.title"), t("delete.confirm", { name }));
  if (!res.ok) return;
  try {
    for (const t of targets) {
      await invoke("delete_path", { path: t.path });
    }
    refresh(side);
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function newFolder() {
  const side = activeSide;
  const name = await promptModal(t("mkdir.title"), t("mkdir.prompt", { path: state[side].path }));
  if (name == null) return;
  try {
    await invoke("make_dir", { parent: state[side].path, name });
    refresh(side);
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function renameSelected() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") {
    alertModal(t("err.title"), t("err.noSelection"));
    return;
  }
  const oldPath = selectedPath(side);
  const oldName = row.entry.name;
  const newName = await promptModal(t("rename.title"), t("rename.prompt", { name: oldName }), oldName);
  if (newName == null) return;
  const trimmed = newName.trim();
  if (!trimmed || trimmed === oldName) return;
  try {
    await invoke("rename_path", { path: oldPath, newName: trimmed });
    await loadDir(side, state[side].path);
    const rows = state[side].rows || [];
    const idx = rows.findIndex((r) => r.kind === "item" && r.entry.name === trimmed);
    if (idx >= 0) select(side, idx);
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

const VIEW_CHUNK = 256 * 1024;
const VIEW_BUFFER = 64 * 1024;
let viewState = null;

async function viewFile() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") return;
  const src = selectedPath(side);
  try {
    const info = await invoke("path_info", { path: src }).catch(() => null);
    const fileSize = info ? info.size : 0;
    const firstChunk = await invoke("read_file_chunk", { path: src, offset: 0, limit: VIEW_CHUNK });
    viewState = { path: src, fileSize, loaded: firstChunk.length, text: firstChunk };
    showModal(t("view.title", { name: row.entry.name }), "pre", firstChunk, true);
    modalEl.classList.add("maximized");
    const pre = modalBody.querySelector("pre");
    if (pre) {
      pre.tabIndex = 0;
      pre.style.outline = "none";
      pre.style.whiteSpace = "pre";
      pre.style.overflow = "auto";
      pre.addEventListener("scroll", onViewScroll);
      pre.focus();
    }
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function onViewScroll(ev) {
  if (!viewState) return;
  const pre = ev.target;
  const nearBottom = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 200;
  if (nearBottom && viewState.loaded < viewState.fileSize) {
    const chunk = await invoke("read_file_chunk", {
      path: viewState.path,
      offset: viewState.loaded,
      limit: VIEW_CHUNK,
    });
    if (chunk.length > 0) {
      viewState.text += chunk;
      viewState.loaded += chunk.length;
      const pos = pre.scrollTop;
      pre.textContent = viewState.text;
      pre.scrollTop = pos;
    }
  }
}

async function editFile() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") return;
  const src = selectedPath(side);
  const appearance = await invoke("get_appearance").catch(() => null);
  const external = (appearance && appearance.editCommand || "").trim();
  try {
    if (external) {
      await invoke("run_edit", { path: src });
    } else {
      await invoke("edit_path", { path: src });
    }
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function fileInfo() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") return;
  const src = selectedPath(side);
  try {
    const info = await invoke("path_info", { path: src });
    const rows = [
      [t("info.name"), info.name],
      [t("info.path"), info.path],
      [t("info.type"), info.isDir ? t("info.typeDir") : t("info.typeFile")],
      [t("info.size"), info.isDir ? "—" : fmtSize(info.size)],
      [t("info.modified"), fmtDate(info.modified)],
      [t("info.created"), fmtDate(info.created)],
      [t("info.permissions"), info.permissions],
    ];
    let html = `<table class="info">`;
    for (const [k, v] of rows) html += `<tr><td>${k}</td><td>${v}</td></tr>`;
    html += `</table>`;
    showModal(t("info.title"), "html", html, true);
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

function esc(str) {
  return String(str).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  })[c]);
}

function computeDiff(aLines, bLines) {
  const m = aLines.length;
  const n = bLines.length;
  const dp = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] =
        aLines[i] === bLines[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const out = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (aLines[i] === bLines[j]) {
      out.push([" ", aLines[i]]);
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push(["-", aLines[i]]);
      i++;
    } else {
      out.push(["+", bLines[j]]);
      j++;
    }
  }
  while (i < m) {
    out.push(["-", aLines[i]]);
    i++;
  }
  while (j < n) {
    out.push(["+", bLines[j]]);
    j++;
  }
  return out;
}

async function diffSelected() {
  const left = selectedRow("left");
  const right = selectedRow("right");
  if (
    !left ||
    !right ||
    left.kind === "parent" ||
    right.kind === "parent" ||
    left.kind !== "item" ||
    right.kind !== "item" ||
    left.entry.isDir ||
    right.entry.isDir
  ) {
    alertModal(t("err.title"), t("err.noFile"));
    return;
  }
  const leftPath = selectedPath("left");
  const rightPath = selectedPath("right");

  const appearance = await invoke("get_appearance").catch(() => null);
  const external = (appearance && appearance.diffCommand || "").trim();
  if (external) {
    try {
      await invoke("run_diff", { pathA: leftPath, pathB: rightPath });
    } catch (err) {
      alertModal(t("err.title"), String(err));
    }
    return;
  }

  try {
    const [a, b] = await Promise.all([
      invoke("read_text_file", { path: leftPath }),
      invoke("read_text_file", { path: rightPath }),
    ]);
    const aLines = a.split("\n");
    const bLines = b.split("\n");
    if (a === b) {
      showModal(t("diff.title"), "pre", t("diff.same"), true);
      return;
    }
    const diff = computeDiff(aLines, bLines);
    let html = `<pre class="diff">`;
    for (const [mark, line] of diff) {
      const cls = mark === "-" ? "diff-del" : mark === "+" ? "diff-add" : "diff-ctx";
      html += `<div class="${cls}">${mark === " " ? "&nbsp;" : esc(mark)} ${esc(line)}</div>`;
    }
    html += `</pre>`;
    showModal(t("diff.title"), "html", html, true);
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function helpModal() {
  const v = await invoke("get_app_version").catch(() => "");
  showModal(t("help.title"), "pre", `${t("help.text")}\n\n${t("help.version")}: ${v}`, true);
}

function fileSearchDialog() {
  lastFocused = document.activeElement;
  modalTitle.textContent = t("searchFiles.title");
  modalBody.innerHTML = "";
  modalActions.innerHTML = "";
  modalEl.classList.add("search-modal");
  const origClose = closeModal;
  const cleanup = () => modalEl.classList.remove("search-modal");
  const doClose = () => {
    cleanup();
    origClose();
  };

  const activePath = state[activeSide].path || "";

  const mkRow = (labelKey, input) => {
    const row = document.createElement("div");
    row.className = "row";
    const lab = document.createElement("label");
    lab.textContent = t(labelKey);
    lab.style.minWidth = "90px";
    row.appendChild(lab);
    row.appendChild(input);
    return row;
  };

  const baseInput = document.createElement("input");
  baseInput.type = "text";
  baseInput.value = activePath;
  baseInput.style.flex = "1";
  baseInput.autocomplete = "off";
  modalBody.appendChild(mkRow("searchFiles.basePath", baseInput));

  const exclInput = document.createElement("input");
  exclInput.type = "text";
  exclInput.placeholder = t("searchFiles.exclusionsHint");
  exclInput.style.flex = "1";
  exclInput.autocomplete = "off";
  modalBody.appendChild(mkRow("searchFiles.exclusions", exclInput));

  const patternInput = document.createElement("input");
  patternInput.type = "text";
  patternInput.placeholder = t("searchFiles.patternHint");
  patternInput.style.flex = "1";
  patternInput.autocomplete = "off";
  modalBody.appendChild(mkRow("searchFiles.pattern", patternInput));

  const modeRow = document.createElement("div");
  modeRow.className = "row";
  const globRadio = document.createElement("input");
  globRadio.type = "radio";
  globRadio.name = "fileMode";
  globRadio.value = "glob";
  globRadio.checked = true;
  globRadio.id = "file-mode-glob";
  const globLab = document.createElement("label");
  globLab.htmlFor = "file-mode-glob";
  globLab.textContent = t("searchFiles.glob");
  globLab.style.marginRight = "12px";
  const reRadio = document.createElement("input");
  reRadio.type = "radio";
  reRadio.name = "fileMode";
  reRadio.value = "regexp";
  reRadio.id = "file-mode-re";
  const reLab = document.createElement("label");
  reLab.htmlFor = "file-mode-re";
  reLab.textContent = t("searchFiles.regexp");
  modeRow.appendChild(globRadio);
  modeRow.appendChild(globLab);
  modeRow.appendChild(reRadio);
  modeRow.appendChild(reLab);
  modalBody.appendChild(modeRow);

  const optsRow = document.createElement("div");
  optsRow.className = "row";
  const igCase = document.createElement("input");
  igCase.type = "checkbox";
  igCase.checked = true;
  igCase.id = "search-igcase";
  const igLab = document.createElement("label");
  igLab.htmlFor = "search-igcase";
  igLab.textContent = t("searchFiles.ignoreCase");
  igLab.style.marginRight = "12px";
  const rec = document.createElement("input");
  rec.type = "checkbox";
  rec.checked = true;
  rec.id = "search-rec";
  const recLab = document.createElement("label");
  recLab.htmlFor = "search-rec";
  recLab.textContent = t("searchFiles.recursive");
  optsRow.appendChild(igCase);
  optsRow.appendChild(igLab);
  optsRow.appendChild(rec);
  optsRow.appendChild(recLab);
  modalBody.appendChild(optsRow);

  const sep1 = document.createElement("hr");
  modalBody.appendChild(sep1);

  const contentEnable = document.createElement("input");
  contentEnable.type = "checkbox";
  contentEnable.id = "search-content-enable";
  const contentEnableLab = document.createElement("label");
  contentEnableLab.htmlFor = "search-content-enable";
  contentEnableLab.textContent = t("searchFiles.contentEnable");
  const contentEnableRow = document.createElement("div");
  contentEnableRow.className = "row";
  contentEnableRow.appendChild(contentEnable);
  contentEnableRow.appendChild(contentEnableLab);
  modalBody.appendChild(contentEnableRow);

  const contentPanel = document.createElement("div");
  contentPanel.style.opacity = "0.5";
  contentPanel.style.pointerEvents = "none";
  const contentInput = document.createElement("input");
  contentInput.type = "text";
  contentInput.style.flex = "1";
  contentInput.autocomplete = "off";
  const contentRow = mkRow("searchFiles.contentPattern", contentInput);
  contentPanel.appendChild(contentRow);
  const cModeRow = document.createElement("div");
  cModeRow.className = "row";
  const cTextRadio = document.createElement("input");
  cTextRadio.type = "radio";
  cTextRadio.name = "contentMode";
  cTextRadio.value = "text";
  cTextRadio.checked = true;
  cTextRadio.id = "c-mode-text";
  const cTextLab = document.createElement("label");
  cTextLab.htmlFor = "c-mode-text";
  cTextLab.textContent = t("searchFiles.contentText");
  cTextLab.style.marginRight = "12px";
  const cReRadio = document.createElement("input");
  cReRadio.type = "radio";
  cReRadio.name = "contentMode";
  cReRadio.value = "regexp";
  cReRadio.id = "c-mode-re";
  const cReLab = document.createElement("label");
  cReLab.htmlFor = "c-mode-re";
  cReLab.textContent = t("searchFiles.contentRegexp");
  cModeRow.appendChild(cTextRadio);
  cModeRow.appendChild(cTextLab);
  cModeRow.appendChild(cReRadio);
  cModeRow.appendChild(cReLab);
  contentPanel.appendChild(cModeRow);
  const cOptsRow = document.createElement("div");
  cOptsRow.className = "row";
  const cIg = document.createElement("input");
  cIg.type = "checkbox";
  cIg.checked = true;
  cIg.id = "search-c-igcase";
  const cIgLab = document.createElement("label");
  cIgLab.htmlFor = "search-c-igcase";
  cIgLab.textContent = t("searchFiles.ignoreCase");
  cOptsRow.appendChild(cIg);
  cOptsRow.appendChild(cIgLab);
  contentPanel.appendChild(cOptsRow);
  modalBody.appendChild(contentPanel);

  contentEnable.addEventListener("change", () => {
    const en = contentEnable.checked;
    contentPanel.style.opacity = en ? "1" : "0.5";
    contentPanel.style.pointerEvents = en ? "auto" : "none";
  });

  const sep2 = document.createElement("hr");
  modalBody.appendChild(sep2);

  const resultsLabel = document.createElement("div");
  resultsLabel.textContent = t("searchFiles.results");
  resultsLabel.style.fontWeight = "bold";
  modalBody.appendChild(resultsLabel);

  const results = document.createElement("ul");
  results.className = "fav-list search-results";
  results.tabIndex = 0;
  results.style.minWidth = "auto";
  results.style.maxWidth = "none";
  results.style.width = "100%";
  results.style.minHeight = "120px";
  results.style.maxHeight = "220px";
  modalBody.appendChild(results);

  const hint = document.createElement("div");
  hint.className = "hint";
  hint.textContent = "";
  modalBody.appendChild(hint);

  const cancelBtn = document.createElement("button");
  cancelBtn.textContent = t("btn.cancel");
  cancelBtn.addEventListener("mousedown", doClose);
  modalActions.appendChild(cancelBtn);

  const searchBtn = document.createElement("button");
  searchBtn.textContent = t("searchFiles.search");
  searchBtn.className = "primary";
  modalActions.appendChild(searchBtn);

  overlay.classList.add("open");
  patternInput.focus();

  const chooseResult = async (idx) => {
    const path = results.children[idx]?.dataset?.path;
    if (!path) return;
    doClose();
    const isDir = await invoke("path_info", { path }).then((i) => i.isDir).catch(() => false);
    if (isDir) {
      await loadDir(activeSide, path);
    } else {
      const slash = path.includes("\\") ? "\\" : "/";
      const parent = path.substring(0, path.lastIndexOf(slash)) || "/";
      const name = path.substring(path.lastIndexOf(slash) + 1);
      await loadDir(activeSide, parent);
      const rows = state[activeSide].rows || [];
      const sel = rows.findIndex((r) => r.kind === "item" && r.entry.name === name);
      if (sel >= 0) select(activeSide, sel);
    }
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
    if (ev.key === "Enter") { ev.preventDefault(); doSearch(); }
  });

}

function settingsModal() {
  const SIZES = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28];
  const body = document.createElement("div");

  function mkH2(text) {
    const h = document.createElement("h2");
    h.textContent = text;
    h.style.cssText = "font-size:var(--font-size,14px);color:var(--dir);margin:8px 0 4px";
    return h;
  }
  function mkOpt(tag, attrs, text) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) el[k] = v;
    if (text) {
      const sp = document.createElement("span");
      sp.textContent = text;
      const lbl = document.createElement("label");
      lbl.className = "opt";
      lbl.appendChild(el);
      lbl.appendChild(sp);
      return lbl;
    }
    return el;
  }
  function mkRow(labelText, input) {
    const row = document.createElement("div");
    row.className = "row";
    const lbl = document.createElement("label");
    lbl.textContent = labelText;
    row.appendChild(lbl);
    if (input) row.appendChild(input);
    return row;
  }
  function mkHint(text) {
    const d = document.createElement("div");
    d.className = "hint";
    d.textContent = text;
    return d;
  }
  function quotePath(p) {
    return /[\s"']/.test(p) ? `"${p.replace(/"/g, '\\"')}"` : p;
  }

  // Theme
  body.appendChild(mkH2(t("settings.theme")));
  const themes = [
    ["system", t("theme.system")],
    ["light", t("theme.light")],
    ["dark", t("theme.dark")],
  ];
  const themeName = "settings-theme-" + Date.now();
  themes.forEach(([val, label]) => {
    body.appendChild(mkOpt("input", { type: "radio", name: themeName, value: val }, label));
  });
  const themeRadios = body.querySelectorAll(`input[name="${themeName}"]`);

  // Font
  body.appendChild(mkH2(t("settings.font")));
  const fontSelect = document.createElement("select");
  [["default", t("font.default")], ["monospace", t("font.monospace")], ["serif", t("font.serif")], ["sans", t("font.sans")]].forEach(([v, l]) => {
    const o = document.createElement("option");
    o.value = v; o.textContent = l;
    fontSelect.appendChild(o);
  });
  body.appendChild(mkRow(t("settings.fontFamily"), fontSelect));

  // Font size
  body.appendChild(mkH2(t("settings.fontSize")));
  const sizeSelect = document.createElement("select");
  SIZES.forEach((s) => {
    const o = document.createElement("option");
    o.value = String(s);
    o.textContent = t("sizePx", { size: s });
    sizeSelect.appendChild(o);
  });
  body.appendChild(mkRow(t("settings.sizeLabel"), sizeSelect));

  // Show hidden
  body.appendChild(mkH2(t("settings.files")));
  const showHiddenCb = mkOpt("input", { type: "checkbox", id: "set-show-hidden" }, t("settings.showHidden"));
  body.appendChild(showHiddenCb);

  // Diff
  body.appendChild(mkH2(t("settings.diff")));
  const diffInput = document.createElement("input");
  diffInput.type = "text"; diffInput.placeholder = "nvim -d";
  diffInput.autocomplete = "off"; diffInput.autocorrect = "off";
  diffInput.autocapitalize = "off"; diffInput.spellcheck = false;
  const diffRow = mkRow(t("settings.diffCommand"), diffInput);
  const diffBrowse = document.createElement("button");
  diffBrowse.type = "button"; diffBrowse.textContent = t("btn.browse");
  diffBrowse.style.cssText = "flex:0 0 auto;margin-left:8px;padding:4px 10px;border:1px solid var(--modal-border);border-radius:3px;background:var(--modal-input-bg);color:var(--modal-input-text);cursor:pointer;font-family:inherit;font-size:var(--font-size,14px)";
  diffRow.appendChild(diffBrowse);
  body.appendChild(diffRow);
  body.appendChild(mkHint(t("settings.diffHint")));
  const diffTermCb = mkOpt("input", { type: "checkbox", id: "set-diff-terminal" }, t("settings.diffTerminal"));
  body.appendChild(diffTermCb);

  // Edit
  body.appendChild(mkH2(t("settings.edit")));
  const editInput = document.createElement("input");
  editInput.type = "text"; editInput.placeholder = "code -w";
  editInput.autocomplete = "off"; editInput.autocorrect = "off";
  editInput.autocapitalize = "off"; editInput.spellcheck = false;
  const editRow = mkRow(t("settings.editCommand"), editInput);
  const editBrowse = document.createElement("button");
  editBrowse.type = "button"; editBrowse.textContent = t("btn.browse");
  editBrowse.style.cssText = diffBrowse.style.cssText;
  editRow.appendChild(editBrowse);
  body.appendChild(editRow);
  body.appendChild(mkHint(t("settings.editHint")));
  const editTermCb = mkOpt("input", { type: "checkbox", id: "set-edit-terminal" }, t("settings.editTerminal"));
  body.appendChild(editTermCb);

  showModal(t("settings.title"), "append", body, true);

  // Load current values
  invoke("get_appearance").then((a) => {
    if (!a) return;
    const r = body.querySelector(`input[name="${themeName}"][value="${a.theme}"]`);
    if (r) r.checked = true;
    fontSelect.value = a.font;
    sizeSelect.value = String(a.fontSize);
    diffInput.value = a.diffCommand || "";
    diffTermCb.querySelector("input").checked = !!a.diffInTerminal;
    editInput.value = a.editCommand || "";
    editTermCb.querySelector("input").checked = !!a.editInTerminal;
    showHiddenCb.querySelector("input").checked = a.showHidden !== false;
  }).catch(() => {});

  // Bind controls
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
  showHiddenCb.querySelector("input").addEventListener("change", async (ev) => {
    try { await invoke("set_show_hidden", { showHidden: ev.target.checked }); } catch {}
  });

  let diffDebounce = null;
  diffInput.addEventListener("input", () => {
    clearTimeout(diffDebounce);
    diffDebounce = setTimeout(async () => {
      try { await invoke("set_diff_command", { command: diffInput.value }); } catch {}
    }, 400);
  });
  diffTermCb.querySelector("input").addEventListener("change", async (ev) => {
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
  editTermCb.querySelector("input").addEventListener("change", async (ev) => {
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
}

function quickMenu() {
  const items = [
    [t("menu.copy"), () => copyOrMove("copy")],
    [t("menu.move"), () => copyOrMove("move")],
    [t("menu.mkdir"), () => newFolder()],
    [t("menu.delete"), () => deleteSelected()],
    [t("menu.view"), () => viewFile()],
    [t("menu.info"), () => fileInfo()],
  ];
  let html = "";
  items.forEach(([label, fn], i) => {
    html += `<button class="menu-item" data-i="${i}">${label}</button>`;
  });
  showModal(t("menu.title"), "html", html, false);
  modalBody.querySelectorAll(".menu-item").forEach((b) => {
    b.addEventListener("mousedown", () => {
      closeModal();
      items[Number(b.dataset.i)][1]();
    });
  });
}

function quitApp() {
  invoke("quit_app").catch(() => {});
}

/* ---------- Modal helpers ---------- */

let lastFocused = null;
let progressCtrl = null;

function getModalFocusables() {
  const modal = document.querySelector(".modal");
  if (!modal) return [];
  return Array.from(
    modal.querySelectorAll(
      'button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
    )
  ).filter((el) => !el.disabled && el.offsetParent !== null);
}

function showModal(title, kind, content, closable) {
  hideNameTooltip();
  lastFocused = document.activeElement;
  modalTitle.textContent = title;
  modalBody.innerHTML = "";
  if (kind === "pre") {
    const pre = document.createElement("pre");
    pre.textContent = content;
    modalBody.appendChild(pre);
  } else if (kind === "html") {
    modalBody.innerHTML = content;
  } else if (kind === "append") {
    modalBody.appendChild(content);
  }
  modalActions.innerHTML = "";
  if (closable) {
    const closeBtn = document.createElement("button");
    closeBtn.textContent = t("btn.close");
    closeBtn.className = "primary";
    closeBtn.addEventListener("mousedown", closeModal);
    modalActions.appendChild(closeBtn);
  }
  overlay.classList.add("open");
  focusFirstAction();
}

function closeModal() {
  overlay.classList.remove("open");
  modalBody.innerHTML = "";
  modalActions.innerHTML = "";
  modalEl.classList.remove("search-modal");
  modalEl.classList.remove("maximized");
  viewState = null;
  if (lastFocused && document.contains(lastFocused)) {
    lastFocused.focus();
  }
  lastFocused = null;
}

function openProgressModal(title, id) {
  lastFocused = document.activeElement;
  modalTitle.textContent = title;
  modalBody.innerHTML = "";
  const ctrl = { cancelled: false, closed: false };

  const curLabel = document.createElement("div");
  curLabel.className = "prog-label";
  modalBody.appendChild(curLabel);

  const curBar = document.createElement("div");
  curBar.className = "prog-bar";
  const curFill = document.createElement("div");
  curFill.className = "prog-fill";
  curBar.appendChild(curFill);
  modalBody.appendChild(curBar);

  const curStats = document.createElement("div");
  curStats.className = "prog-stats";
  modalBody.appendChild(curStats);

  const totLabel = document.createElement("div");
  totLabel.className = "prog-label";
  modalBody.appendChild(totLabel);

  const totBar = document.createElement("div");
  totBar.className = "prog-bar";
  const totFill = document.createElement("div");
  totFill.className = "prog-fill";
  totBar.appendChild(totFill);
  modalBody.appendChild(totBar);

  ctrl.setCurrent = (label, copied, total, statsText) => {
    curLabel.textContent = label;
    if (total > 0) {
      curFill.classList.remove("indet");
      curFill.style.width = `${Math.min(100, Math.round((copied / total) * 100))}%`;
    } else {
      curFill.classList.add("indet");
    }
    curStats.textContent = statsText || "";
  };

  ctrl.setOverall = (label, done, total) => {
    totLabel.textContent = label;
    if (total > 0) {
      totFill.classList.remove("indet");
      totFill.style.width = `${Math.min(100, Math.round((done / total) * 100))}%`;
    } else {
      totFill.classList.add("indet");
    }
  };

  const cancelBtn = document.createElement("button");
  cancelBtn.textContent = t("btn.cancel");
  ctrl.cancel = () => {
    if (ctrl.cancelled) return;
    ctrl.cancelled = true;
    cancelBtn.disabled = true;
    cancelBtn.textContent = t("copy.cancelling");
    invoke("cancel_copy", { id }).catch(() => {});
  };
  cancelBtn.addEventListener("mousedown", ctrl.cancel);
  modalActions.appendChild(cancelBtn);

  overlay.classList.add("open");
  cancelBtn.focus();
  ctrl.close = () => {
    if (ctrl.closed) return;
    ctrl.closed = true;
    closeModal();
  };
  return ctrl;
}

function confirmModal(title, message, checkboxes) {
  return new Promise((resolve) => {
    lastFocused = document.activeElement;
    modalTitle.textContent = title;
    modalBody.innerHTML = "";
    const pre = document.createElement("pre");
    pre.textContent = message;
    modalBody.appendChild(pre);

    const values = {};
    if (checkboxes && checkboxes.length) {
      const boxGroup = document.createElement("div");
      boxGroup.className = "modal-checks";
      checkboxes.forEach((cb) => {
        const label = document.createElement("label");
        label.className = "opt";
        const input = document.createElement("input");
        input.type = "checkbox";
        input.checked = !!cb.checked;
        values[cb.id] = input.checked;
        input.addEventListener("change", () => {
          values[cb.id] = input.checked;
        });
        label.appendChild(input);
        label.appendChild(document.createTextNode(cb.label));
        boxGroup.appendChild(label);
      });
      modalBody.appendChild(boxGroup);
    }

    modalActions.innerHTML = "";

    const noBtn = document.createElement("button");
    noBtn.textContent = t("btn.no");
    noBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve({ ok: false, values });
    });
    noBtn.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        ev.stopPropagation();
        closeModal();
        resolve({ ok: false, values });
      }
    });
    modalActions.appendChild(noBtn);

    const yesBtn = document.createElement("button");
    yesBtn.textContent = t("btn.yes");
    yesBtn.className = "primary";
    yesBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve({ ok: true, values });
    });
    yesBtn.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        ev.stopPropagation();
        closeModal();
        resolve({ ok: true, values });
      }
    });
    modalActions.appendChild(yesBtn);

    overlay.classList.add("open");
    yesBtn.focus();
  });
}

function promptModal(title, label, initial) {
  return new Promise((resolve) => {
    lastFocused = document.activeElement;
    modalTitle.textContent = title;
    modalBody.innerHTML = "";
    const p = document.createElement("p");
    p.textContent = label;
    modalBody.appendChild(p);
    const input = document.createElement("input");
    input.type = "text";
    input.autocomplete = "off";
    input.autocorrect = "off";
    input.autocapitalize = "off";
    input.spellcheck = false;
    input.value = initial ?? "";
    modalBody.appendChild(input);
    modalActions.innerHTML = "";

    const cancelBtn = document.createElement("button");
    cancelBtn.textContent = t("btn.cancel");
    cancelBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve(null);
    });
    modalActions.appendChild(cancelBtn);

    const okBtn = document.createElement("button");
    okBtn.textContent = t("btn.ok");
    okBtn.className = "primary";
    okBtn.addEventListener("mousedown", () => {
      const value = input.value;
      closeModal();
      resolve(value);
    });
    modalActions.appendChild(okBtn);

    overlay.classList.add("open");
    input.focus();
    input.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        ev.stopPropagation();
        const value = input.value;
        closeModal();
        resolve(value);
      }
      if (ev.key === "Escape") {
        ev.preventDefault();
        ev.stopPropagation();
        closeModal();
        resolve(null);
      }
    });
  });
}

function commandModal() {
  return new Promise((resolve) => {
    lastFocused = document.activeElement;
    modalTitle.textContent = t("cmd.title");
    modalBody.innerHTML = "";
    const p = document.createElement("p");
    p.textContent = t("cmd.prompt");
    modalBody.appendChild(p);
    const input = document.createElement("input");
    input.type = "text";
    input.autocomplete = "off";
    input.autocorrect = "off";
    input.autocapitalize = "off";
    input.spellcheck = false;
    modalBody.appendChild(input);
    const label = document.createElement("label");
    label.className = "opt";
    const term = document.createElement("input");
    term.type = "checkbox";
    term.checked = true;
    label.appendChild(term);
    label.appendChild(document.createTextNode(t("cmd.terminal")));
    modalBody.appendChild(label);
    modalActions.innerHTML = "";

    const cancelBtn = document.createElement("button");
    cancelBtn.textContent = t("btn.cancel");
    cancelBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve(null);
    });
    modalActions.appendChild(cancelBtn);

    const runBtn = document.createElement("button");
    runBtn.textContent = t("cmd.run");
    runBtn.className = "primary";
    runBtn.addEventListener("mousedown", () => {
      const value = input.value;
      closeModal();
      resolve({ command: value, inTerminal: term.checked });
    });
    modalActions.appendChild(runBtn);

    overlay.classList.add("open");
    input.focus();
    input.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        ev.stopPropagation();
        const value = input.value;
        closeModal();
        resolve({ command: value, inTerminal: term.checked });
      }
      if (ev.key === "Escape") {
        ev.preventDefault();
        ev.stopPropagation();
        closeModal();
        resolve(null);
      }
    });
  });
}

let favKeyHandler = null;

function favModal(opts) {
  return new Promise((resolve) => {
    if (favKeyHandler) modalEl.removeEventListener("keydown", favKeyHandler);
    favKeyHandler = (ev) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        ev.stopPropagation();
        closeModal();
        resolve(null);
      }
    };
    modalEl.addEventListener("keydown", favKeyHandler);

    lastFocused = document.activeElement;
    modalTitle.textContent = opts.title;

    let items = [];
    let sel = 0;
    let loading = true;
    let list = null;

    const choose = (i) => {
      if (!items[i]) return;
      closeModal();
      resolve(items[i]);
    };

    const save = async (next) => {
      items = next;
      sel = Math.min(sel, Math.max(0, items.length - 1));
      try {
        await opts.save(items);
      } catch (err) {
        alertModal(t("err.title"), String(err));
      }
    };

    const updateSel = () => {
      const els = list ? list.children : [];
      for (let i = 0; i < els.length; i++) {
        els[i].classList.toggle("selected", i === sel && i < items.length);
      }
    };

    const render = () => {
      modalBody.innerHTML = "";
      list = document.createElement("ul");
      list.className = "fav-list";
      list.setAttribute("tabindex", "-1");
      modalBody.appendChild(list);

      if (!loading) {
        if (sel >= items.length) sel = Math.max(0, items.length - 1);
        items.forEach((path, i) => {
          const li = document.createElement("li");
          li.textContent = path;
          li.addEventListener("mousedown", (ev) => {
            ev.preventDefault();
            sel = i;
            updateSel();
          });
          li.addEventListener("dblclick", (ev) => {
            ev.preventDefault();
            choose(i);
          });
          list.appendChild(li);
        });
        if (items.length === 0) {
          const empty = document.createElement("li");
          empty.className = "empty";
          empty.textContent = opts.empty;
          list.appendChild(empty);
        }
      }

      list.addEventListener("keydown", (ev) => {
        const pageStep = () => {
          const first = list.firstElementChild;
          if (!first) return 1;
          const rowH = first.getBoundingClientRect().height || 20;
          return Math.max(1, Math.floor(list.clientHeight / rowH));
        };
        if (ev.key === "ArrowDown") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = Math.min(items.length - 1, sel + 1);
          updateSel();
        } else if (ev.key === "ArrowUp") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = Math.max(0, sel - 1);
          updateSel();
        } else if (ev.key === "PageDown") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = Math.min(items.length - 1, sel + pageStep());
          updateSel();
        } else if (ev.key === "PageUp") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = Math.max(0, sel - pageStep());
          updateSel();
        } else if (ev.key === "Home") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = 0;
          updateSel();
        } else if (ev.key === "End") {
          ev.preventDefault();
          ev.stopPropagation();
          sel = Math.max(0, items.length - 1);
          updateSel();
        } else if (ev.key === "Enter") {
          ev.preventDefault();
          ev.stopPropagation();
          choose(sel);
        } else if (ev.key === "Escape") {
          ev.preventDefault();
          ev.stopPropagation();
          closeModal();
          resolve(null);
        }
      });

      modalActions.innerHTML = "";
      const mkBtn = (label, cls, handler, noRender) => {
        const b = document.createElement("button");
        b.textContent = label;
        if (cls) b.className = cls;
        let mouse = false;
        const run = async () => {
          await handler();
          if (!noRender) render();
        };
        b.addEventListener("mousedown", (ev) => {
          ev.preventDefault();
          mouse = true;
          run();
        });
        b.addEventListener("click", (ev) => {
          ev.preventDefault();
          if (mouse) {
            mouse = false;
            return;
          }
          run();
        });
        modalActions.appendChild(b);
      };

      mkBtn(t("fav.add"), "", async () => {
        const value = await opts.add();
        if (!value) return;
        if (!items.includes(value)) await save([...items, value]);
      });
      mkBtn(t("fav.remove"), "", async () => {
        if (items.length === 0) return;
        await save(items.filter((_, i) => i !== sel));
      });
      mkBtn(t("fav.up"), "", async () => {
        if (sel <= 0) return;
        const next = [...items];
        [next[sel - 1], next[sel]] = [next[sel], next[sel - 1]];
        sel -= 1;
        await save(next);
      });
      mkBtn(t("fav.down"), "", async () => {
        if (sel < 0 || sel >= items.length - 1) return;
        const next = [...items];
        [next[sel + 1], next[sel]] = [next[sel], next[sel + 1]];
        sel += 1;
        await save(next);
      });
      mkBtn(t("btn.ok"), "primary", () => {
        choose(sel);
      }, true);
      mkBtn(t("btn.close"), "", () => {
        closeModal();
        resolve(null);
      }, true);

      updateSel();
      overlay.classList.add("open");
      list.focus();
    };

    opts.load()
      .then((data) => {
        items = Array.isArray(data) ? data : [];
        loading = false;
        render();
      })
      .catch((err) => {
        loading = false;
        closeModal();
        alertModal(t("err.title"), String(err));
        resolve(null);
      });
  });
}

function favoritesModal() {
  return favModal({
    title: t("fav.title"),
    empty: t("fav.empty"),
    load: () => invoke("get_favorites"),
    save: (list) => invoke("set_favorites", { favorites: list }),
    add: async () => {
      const cur = state[activeSide].path || "";
      const value = await promptModal(t("fav.addTitle"), t("fav.addPrompt"), cur);
      return value === null ? null : value.trim();
    },
  });
}

function favAppsModal() {
  return favModal({
    title: t("fav.apps"),
    empty: t("fav.appsEmpty"),
    load: () => invoke("get_fav_apps"),
    save: (list) => invoke("set_fav_apps", { favApps: list }),
    add: async () => {
      const picked = await openDialog({ multiple: false, title: t("dialog.pickApp") }).catch(() => null);
      return picked ? (Array.isArray(picked) ? picked[0] : picked) : null;
    },
  });
}

async function runCommandModal() {
  const res = await commandModal();
  if (!res || !res.command.trim()) return;
  const trimmed = res.command.trim();
  const cdMatch = trimmed.match(/^cd(?:\s+(.*))?$/);
  if (cdMatch) {
    let target = (cdMatch[1] || "").trim();
    if (target) {
      if (
        (target.startsWith('"') && target.endsWith('"')) ||
        (target.startsWith("'") && target.endsWith("'"))
      ) {
        target = target.slice(1, -1);
      }
      if (target) {
        const base = state[activeSide].path || "";
        const isWindows = base.includes("\\");
        const full = isWindows
          ? /^[A-Za-z]:/.test(target)
            ? target
            : `${base.replace(/\\$/, "")}\\${target}`
          : target.startsWith("/")
            ? target
            : `${base.replace(/\/$/, "")}/${target}`;
        await loadDir(activeSide, full);
      }
    }
    return;
  }
  try {
    await invoke("run_command", {
      command: res.command,
      inTerminal: res.inTerminal,
      cwd: state[activeSide].path,
    });
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

function fuzzyMatch(text, q) {
  if (!q) return true;
  let qi = 0;
  for (let i = 0; i < text.length && qi < q.length; i++) {
    if (text[i] === q[qi]) qi++;
  }
  return qi === q.length;
}

function searchPanel(side, query, fuzzy) {
  const rows = state[side].rows || [];
  const q = query.toLowerCase();
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    if (row.kind === "parent") continue;
    const name = row.entry.name.toLowerCase();
    const hit = fuzzy ? fuzzyMatch(name, q) : name.startsWith(q);
    if (hit) {
      select(side, i);
      return true;
    }
  }
  return false;
}

function openSearch(initial) {
  lastFocused = document.activeElement;
  modalTitle.textContent = t("search.title");
  modalBody.innerHTML = "";
  const input = document.createElement("input");
  input.type = "text";
  input.value = initial;
  input.autocomplete = "off";
  input.autocorrect = "off";
  input.autocapitalize = "off";
  input.spellcheck = false;
  modalBody.appendChild(input);
  const label = document.createElement("label");
  label.className = "opt";
  const fuzzy = document.createElement("input");
  fuzzy.type = "checkbox";
  fuzzy.checked = fuzzyPref;
  label.appendChild(fuzzy);
  label.appendChild(document.createTextNode(t("search.fuzzy")));
  modalBody.appendChild(label);
  modalActions.innerHTML = "";
  const closeBtn = document.createElement("button");
  closeBtn.textContent = t("btn.close");
  closeBtn.className = "primary";
  closeBtn.addEventListener("mousedown", () => {
    closeModal();
  });
  modalActions.appendChild(closeBtn);
  overlay.classList.add("open");

  const doSearch = () => searchPanel(activeSide, input.value, fuzzy.checked);
  input.addEventListener("input", doSearch);
  fuzzy.addEventListener("change", () => {
    fuzzyPref = fuzzy.checked;
    invoke("set_fuzzy_search", { fuzzySearch: fuzzy.checked }).catch(() => {});
    doSearch();
  });
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" || ev.key === "Enter") {
      ev.preventDefault();
      ev.stopPropagation();
      closeModal();
    }
  });
  input.focus();
  doSearch();
}

function alertModal(title, message) {
  showModal(title, "pre", message, true);
}

function focusFirstAction() {
  const btn = modalActions.querySelector("button");
  if (btn) btn.focus();
}

/* ---------- Events ---------- */

overlay.addEventListener("mousedown", (ev) => {
  if (ev.target === overlay) closeModal();
});

document.querySelectorAll(".panel-list").forEach((list) => {
  list.setAttribute("tabindex", "0");
  list.addEventListener("mousedown", () => {
    const side = list.dataset.side;
    setActiveSide(side);
  });
});

document.querySelectorAll(".panel-head").forEach((head) => {
  head.addEventListener("mousedown", () => {
    setActiveSide(head.dataset.side);
  });
});

document.querySelectorAll(".panel-cols").forEach((colrow) => {
  colrow.querySelectorAll("span[data-sort]").forEach((sp) => {
    sp.addEventListener("mousedown", (ev) => {
      ev.preventDefault();
      const side = colrow.dataset.side;
      setActiveSide(side);
      toggleSort(side, sp.dataset.sort);
    });
  });
});

document.querySelector("#btn-up").addEventListener("mousedown", () => goUp(activeSide));
document.querySelector("#btn-home").addEventListener("mousedown", async () => {
  const home = await invoke("home_dir").catch(() => "");
  if (home) loadDir(activeSide, home);
});
document.querySelector("#btn-refresh").addEventListener("mousedown", () => refresh(activeSide));
document.querySelector("#btn-paste").addEventListener("mousedown", () => pasteFromClipboard());
document.querySelector("#btn-command").addEventListener("mousedown", () => runCommandModal());
document.querySelector("#btn-fav").addEventListener("mousedown", () => {
  favoritesModal().then((p) => {
    if (p) loadDir(activeSide, p);
  });
});
document.querySelector("#btn-fav-apps").addEventListener("mousedown", () => {
  favAppsModal().then((appPath) => {
    if (!appPath) return;
    const row = selectedRow(activeSide);
    const file = row && row.kind !== "parent" ? selectedPath(activeSide) : "";
    const q = (p) => (/[\s"']/.test(p) ? `"${p.replace(/"/g, '\\"')}"` : p);
    const command = file ? `${q(appPath)} ${q(file)}` : appPath;
    invoke("run_command", { command, inTerminal: false, cwd: state[activeSide].path || "" }).catch((err) =>
      alertModal(t("err.title"), String(err))
    );
  });
});
document.querySelector("#btn-search").addEventListener("mousedown", () => fileSearchDialog());
document.querySelector("#btn-settings").addEventListener("mousedown", () => settingsModal());
document.querySelector("#btn-quit").addEventListener("mousedown", quitApp);
document.querySelector("#btn-back").addEventListener("mousedown", () => goUp(activeSide));

document.querySelectorAll(".fkey").forEach((btn) => {
  btn.addEventListener("mousedown", () => handleFKey(btn.dataset.fkey));
});

document.addEventListener("keydown", (ev) => {
  if (overlay.classList.contains("open")) {
    const pre = modalBody.querySelector("pre");
    if (pre && pre.scrollHeight > pre.clientHeight) {
      const scrollKeys = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "PageUp", "PageDown", "Home", "End", " "];
      if (scrollKeys.includes(ev.key)) {
        ev.preventDefault();
        const step = 40;
        if (ev.key === "ArrowUp") pre.scrollTop -= step;
        else if (ev.key === "ArrowDown") pre.scrollTop += step;
        else if (ev.key === "ArrowLeft") pre.scrollLeft -= step;
        else if (ev.key === "ArrowRight") pre.scrollLeft += step;
        else if (ev.key === "PageUp") pre.scrollTop -= pre.clientHeight;
        else if (ev.key === "PageDown") pre.scrollTop += pre.clientHeight;
        else if (ev.key === "Home") pre.scrollTop = 0;
        else if (ev.key === "End") pre.scrollTop = pre.scrollHeight;
        else if (ev.key === " ") pre.scrollTop += pre.clientHeight;
        return;
      }
    }
    if (ev.key === "Escape") {
      ev.preventDefault();
      if (progressCtrl) {
        progressCtrl.cancel();
      } else {
        closeModal();
      }
    } else if (ev.key === "Tab") {
      ev.preventDefault();
      const focusables = getModalFocusables();
      if (focusables.length === 0) return;
      const current = document.activeElement;
      let idx = focusables.indexOf(current);
      if (ev.shiftKey) {
        idx = idx <= 0 ? focusables.length - 1 : idx - 1;
      } else {
        idx = idx >= focusables.length - 1 ? 0 : idx + 1;
      }
      focusables[idx].focus();
    }
    return;
  }

  const side = activeSide;
  const mod = isMac ? ev.metaKey : ev.ctrlKey;
  if (mod && ev.key.toLowerCase() === "u") {
    ev.preventDefault();
    swapPanels();
    return;
  }
  if (ev.ctrlKey && ev.key === "ArrowRight") {
    ev.preventDefault();
    openSelectedIn("left", "right");
    return;
  }
  if (ev.ctrlKey && ev.key === "ArrowLeft") {
    ev.preventDefault();
    openSelectedIn("right", "left");
    return;
  }
  if (ev.ctrlKey && ev.key === "ArrowDown") {
    ev.preventDefault();
    runCommandModal();
    return;
  }
  if (mod && ev.key.toLowerCase() === "d") {
    ev.preventDefault();
    favoritesModal().then((p) => {
      if (p) loadDir(activeSide, p);
    });
    return;
  }
  if (mod && ev.key.toLowerCase() === "s") {
    ev.preventDefault();
    favAppsModal().then((appPath) => {
      if (!appPath) return;
      const row = selectedRow(activeSide);
      const file = row && row.kind !== "parent" ? selectedPath(activeSide) : "";
      const q = (p) => (/[\s"']/.test(p) ? `"${p.replace(/"/g, '\\"')}"` : p);
      const command = file ? `${q(appPath)} ${q(file)}` : appPath;
      invoke("run_command", {
        command,
        inTerminal: false,
        cwd: state[activeSide].path || "",
      }).catch((err) => alertModal(t("err.title"), String(err)));
    });
    return;
  }
  if (mod && ev.key.toLowerCase() === "a") {
    ev.preventDefault();
    selectAll(side);
    return;
  }
  if (mod && ev.key.toLowerCase() === "c") {
    ev.preventDefault();
    copyToClipboard(false);
    return;
  }
  if (mod && ev.key.toLowerCase() === "x") {
    ev.preventDefault();
    copyToClipboard(true);
    return;
  }
  if (mod && ev.key.toLowerCase() === "v") {
    ev.preventDefault();
    pasteFromClipboard();
    return;
  }
  if (ev.ctrlKey && ev.key === "PageDown") {
    ev.preventDefault();
    openRow(side, getActive().selected, true);
    return;
  }
  switch (ev.key) {
    case "ArrowDown":
      ev.preventDefault();
      moveSelection(side, 1, ev.shiftKey);
      break;
    case "ArrowUp":
      ev.preventDefault();
      moveSelection(side, -1, ev.shiftKey);
      break;
    case "PageDown":
      ev.preventDefault();
      moveSelection(side, pageStep(side));
      break;
    case "PageUp":
      ev.preventDefault();
      moveSelection(side, -pageStep(side));
      break;
    case "Enter":
      ev.preventDefault();
      openRow(side, getActive().selected);
      break;
    case "Backspace":
      ev.preventDefault();
      goUp(side);
      break;
    case "Tab":
      ev.preventDefault();
      setActiveSide(side === "left" ? "right" : "left");
      break;
    case "Home":
      ev.preventDefault();
      select(side, 0, { shift: ev.shiftKey });
      break;
    case "End":
      ev.preventDefault();
      select(side, (state[side].rows || []).length - 1, { shift: ev.shiftKey });
      break;
    case "Delete":
      ev.preventDefault();
      deleteSelected();
      break;
    case " ":
      ev.preventDefault();
      toggleMark(side, state[side].selected);
      break;
    case "+":
      ev.preventDefault();
      markPatternModal(true);
      break;
    case "-":
      ev.preventDefault();
      markPatternModal(false);
      break;
    case "*":
      ev.preventDefault();
      invertMarks(side);
      break;
    default:
      if (/^F(1[0-2]|[1-9])$/.test(ev.key)) {
        ev.preventDefault();
        handleFKey(ev.key);
      } else if (ev.key.length === 1 && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
        ev.preventDefault();
        openSearch(ev.key);
      }
  }
});

function handleFKey(key) {
  const map = {
    F1: helpModal,
    F2: diffSelected,
    F3: viewFile,
    F4: editFile,
    F5: () => copyOrMove("copy"),
    F6: () => copyOrMove("move"),
    F7: newFolder,
    F8: deleteSelected,
    F9: quickMenu,
    F10: quitApp,
    F11: renameSelected,
    F12: fileInfo,
  };
  const fn = map[key];
  if (fn) fn();
}

/* ---------- Column resize ---------- */

const COL_DEFAULTS = { name: 0, ext: 60, size: 80, date: 120 };
let colWidths = { ...COL_DEFAULTS };

function applyColWidths() {
  rootEl.style.setProperty("--col-ext", colWidths.ext + "px");
  rootEl.style.setProperty("--col-size", colWidths.size + "px");
  rootEl.style.setProperty("--col-date", colWidths.date + "px");
}

function setupColumnResize() {
  document.querySelectorAll(".panel-cols").forEach((cols) => {
    const resizePairs = [
      [cols.querySelector(".col-ext"), "ext"],
      [cols.querySelector(".col-size"), "size"],
      [cols.querySelector(".col-date"), "date"],
    ];
    resizePairs.forEach(([span, key]) => {
      if (!span) return;
      const handle = document.createElement("span");
      handle.className = "col-resize";
      span.parentNode.insertBefore(handle, span);
      let startX, startW;
      const onMove = (ev) => {
        ev.preventDefault();
        const diff = ev.clientX - startX;
        colWidths[key] = Math.max(30, startW + diff);
        applyColWidths();
      };
      const onUp = () => {
        handle.classList.remove("active");
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        invoke("set_column_widths", { columnWidths: colWidths }).catch(() => {});
      };
      handle.addEventListener("mousedown", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        startX = ev.clientX;
        startW = colWidths[key];
        handle.classList.add("active");
        document.body.style.cursor = "col-resize";
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
      });
    });
  });
}

/* ---------- Init ---------- */

async function init() {
  getCurrentWindow()
    .onThemeChanged(({ payload }) => {
      if (themePref === "system") {
        currentOsTheme = payload;
        applyTheme();
      }
    })
    .catch(() => {});
  getCurrentWindow()
    .listen("appearance-changed", () => syncAppearance())
    .catch(() => {});
  syncAppearance();

  const appearance = await invoke("get_appearance").catch(() => null);
  if (appearance && appearance.columnWidths) {
    colWidths = { ...COL_DEFAULTS, ...appearance.columnWidths };
  }
  applyColWidths();
  setupColumnResize();

  const home = await invoke("home_dir").catch(() => null);
  const start = home || (await invoke("list_dir", { path: "/" }).then((l) => l.path).catch(() => "/"));
  await loadDir("left", start);
  await loadDir("right", start);
  setActiveSide("left");
}

init();
