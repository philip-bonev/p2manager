import { t, tp, applyStatic } from "/i18n.js";

const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

applyStatic();

const rootEl = document.documentElement;
let themePref = "system";
let currentOsTheme = null;

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
  left: { path: null, parent: null, items: [], selected: 0 },
  right: { path: null, parent: null, items: [], selected: 0 },
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
const foots = {
  left: document.querySelector("#panel-left .panel-foot"),
  right: document.querySelector("#panel-right .panel-foot"),
};
const overlay = document.querySelector("#modal-overlay");
const modalTitle = document.querySelector("#modal-title");
const modalBody = document.querySelector("#modal-body");
const modalActions = document.querySelector("#modal-actions");
const statusbar = document.querySelector("#statusbar");

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

function getActive() {
  return state[activeSide];
}

function getOther() {
  return state[activeSide === "left" ? "right" : "left"];
}

function fullList(side) {
  const s = state[side];
  const rows = [];
  if (s.parent) rows.push({ kind: "parent" });
  for (const it of s.items) rows.push({ kind: "item", entry: it });
  return rows;
}

async function loadDir(side, path) {
  try {
    const listing = await invoke("list_dir", { path });
    const s = state[side];
    s.path = listing.path;
    s.parent = listing.parent ?? null;
    s.items = listing.items;
    s.selected = 0;
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
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = "..";
      li.appendChild(name);
      const date = document.createElement("span");
      date.className = "date";
      date.textContent = t("updir");
      li.appendChild(date);
    } else {
      const e = row.entry;
      if (e.isDir) li.classList.add("dir");
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = e.name;
      li.appendChild(name);
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
      select(side, idx);
    });
    li.addEventListener("dblclick", (ev) => {
      ev.preventDefault();
      openRow(side, idx);
    });

    list.appendChild(li);
  });

  select(side, s.selected);
  updateFoot(side);
}

function select(side, idx) {
  const s = state[side];
  const rows = s.rows || [];
  if (rows.length === 0) return;
  if (idx < 0) idx = 0;
  if (idx >= rows.length) idx = rows.length - 1;
  s.selected = idx;

  listItems(side).forEach((li, i) => {
    li.classList.toggle("selected", i === idx);
  });

  const el = listItems(side)[idx];
  if (el) el.scrollIntoView({ block: "nearest" });
  updateStatus();
}

function listItems(side) {
  return Array.from(lists[side].children);
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
    loadDir(side, s.parent);
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
    loadDir(side, s.parent);
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

function moveSelection(side, delta) {
  const s = state[side];
  const count = (s.rows || []).length;
  if (count === 0) return;
  let idx = s.selected + delta;
  if (idx < 0) idx = 0;
  if (idx >= count) idx = count - 1;
  select(side, idx);
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
  const row = selectedRow(side);
  if (!row) {
    alertModal(t("err.title"), t("err.noSelection"));
    return;
  }
  if (row.kind === "parent") {
    alertModal(t("err.title"), t("err.notParent"));
    return;
  }
  const src = selectedPath(side);
  if (!src) return;
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
  const res = await confirmModal(
    verb,
    t("copyMove.confirm", {
      verb,
      name: row.entry.name,
      src: state[side].path,
      dst: other.path,
    }),
    checkboxes
  );
  if (!res.ok) return;
  try {
    if (op === "copy" && (res.values.hard || res.values.soft)) {
      await invoke("link_path", {
        src,
        dstDir: other.path,
        hard: !!res.values.hard,
      });
    } else {
      await invoke(op === "copy" ? "copy_path" : "move_path", {
        src,
        dstDir: other.path,
      });
    }
    refresh(side);
    refresh(activeSide === "left" ? "right" : "left");
  } catch (err) {
    alertModal(t("err.title"), String(err));
  }
}

async function deleteSelected() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row) {
    alertModal(t("err.title"), t("err.noSelection"));
    return;
  }
  if (row.kind === "parent") return;
  const src = selectedPath(side);
  const name = row.entry.name;
  const res = await confirmModal(t("delete.title"), t("delete.confirm", { name }));
  if (!res.ok) return;
  try {
    await invoke("delete_path", { path: src });
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

async function viewFile() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") return;
  const src = selectedPath(side);
  try {
    const content = await invoke("read_text_file", { path: src });
    showModal(t("view.title", { name: row.entry.name }), "pre", content, true);
  } catch (err) {
    alertModal(t("err.title"), String(err));
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

function helpModal() {
  showModal(t("help.title"), "pre", t("help.text"), true);
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
  lastFocused = document.activeElement;
  modalTitle.textContent = title;
  modalBody.innerHTML = "";
  if (kind === "pre") {
    const pre = document.createElement("pre");
    pre.textContent = content;
    modalBody.appendChild(pre);
  } else if (kind === "html") {
    modalBody.innerHTML = content;
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
  if (lastFocused && document.contains(lastFocused)) {
    lastFocused.focus();
  }
  lastFocused = null;
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
    modalActions.appendChild(noBtn);

    const yesBtn = document.createElement("button");
    yesBtn.textContent = t("btn.yes");
    yesBtn.className = "primary";
    yesBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve({ ok: true, values });
    });
    modalActions.appendChild(yesBtn);

    overlay.classList.add("open");
    yesBtn.focus();
  });
}

function promptModal(title, label) {
  return new Promise((resolve) => {
    lastFocused = document.activeElement;
    modalTitle.textContent = title;
    modalBody.innerHTML = "";
    const p = document.createElement("p");
    p.textContent = label;
    modalBody.appendChild(p);
    const input = document.createElement("input");
    input.type = "text";
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
        const value = input.value;
        closeModal();
        resolve(value);
      }
      if (ev.key === "Escape") {
        ev.preventDefault();
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
        const value = input.value;
        closeModal();
        resolve({ command: value, inTerminal: term.checked });
      }
      if (ev.key === "Escape") {
        ev.preventDefault();
        closeModal();
        resolve(null);
      }
    });
  });
}

async function runCommandModal() {
  const res = await commandModal();
  if (!res || !res.command.trim()) return;
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
    doSearch();
  });
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" || ev.key === "Enter") {
      ev.preventDefault();
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

document.querySelector("#btn-up").addEventListener("mousedown", () => goUp(activeSide));
document.querySelector("#btn-home").addEventListener("mousedown", async () => {
  const home = await invoke("home_dir").catch(() => "");
  if (home) loadDir(activeSide, home);
});
document.querySelector("#btn-refresh").addEventListener("mousedown", () => refresh(activeSide));
document.querySelector("#btn-settings").addEventListener("mousedown", () => {
  invoke("open_settings").catch((e) => alertModal(t("err.title"), String(e)));
});
document.querySelector("#btn-quit").addEventListener("mousedown", quitApp);
document.querySelector("#btn-back").addEventListener("mousedown", () => goUp(activeSide));

document.querySelectorAll(".fkey").forEach((btn) => {
  btn.addEventListener("mousedown", () => handleFKey(btn.dataset.fkey));
});

document.addEventListener("keydown", (ev) => {
  if (overlay.classList.contains("open")) {
    if (ev.key === "Escape") {
      ev.preventDefault();
      closeModal();
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
  if (ev.ctrlKey && ev.key.toLowerCase() === "u") {
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
  if (ev.ctrlKey && ev.key === "PageDown") {
    ev.preventDefault();
    openRow(side, getActive().selected, true);
    return;
  }
  switch (ev.key) {
    case "ArrowDown":
      ev.preventDefault();
      moveSelection(side, 1);
      break;
    case "ArrowUp":
      ev.preventDefault();
      moveSelection(side, -1);
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
      select(side, 0);
      break;
    case "End":
      ev.preventDefault();
      select(side, (state[side].rows || []).length - 1);
      break;
    case "Delete":
      ev.preventDefault();
      deleteSelected();
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
    F11: () => refresh(activeSide),
    F12: fileInfo,
  };
  const fn = map[key];
  if (fn) fn();
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

  const home = await invoke("home_dir").catch(() => null);
  const start = home || (await invoke("list_dir", { path: "/" }).then((l) => l.path).catch(() => "/"));
  await loadDir("left", start);
  await loadDir("right", start);
  setActiveSide("left");
}

init();