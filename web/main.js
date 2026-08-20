const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const opener = window.__TAURI__.opener;

const rootEl = document.documentElement;

function setThemeClass(theme) {
  rootEl.classList.toggle("theme-dark", theme === "dark");
  rootEl.classList.toggle("theme-light", theme === "light");
}

function syncTheme() {
  getCurrentWindow()
    .theme()
    .then(setThemeClass)
    .catch(() => {
      const dark = window.matchMedia
        ? window.matchMedia("(prefers-color-scheme: dark)").matches
        : false;
      setThemeClass(dark ? "dark" : "light");
    });
}

const state = {
  left: { path: null, parent: null, items: [], selected: 0 },
  right: { path: null, parent: null, items: [], selected: 0 },
};
let activeSide = "left";

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
    alertModal("Грешка", String(err));
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
      date.textContent = "UP-DIR";
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
  foots[side].textContent = `${total} елемента`;
}

function updateStatus() {
  const act = getActive();
  const other = getOther();
  const rows = act.rows || [];
  const row = rows[act.selected];
  let sel = "—";
  if (row) {
    if (row.kind === "parent") {
      sel = `Нагоре → ${act.parent}`;
    } else if (row.entry.isDir) {
      sel = `Папка: ${row.entry.name}/`;
    } else {
      sel = `${row.entry.name} (${fmtSize(row.entry.size)}, ${fmtDate(row.entry.modified)})`;
    }
  }
  statusbar.textContent = `Активен: ${act.path} | Избран: ${sel} | Друг панел: ${other.path}`;
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

function openRow(side, idx) {
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
    loadDir(side, full);
  } else {
    opener
      .openPath(full)
      .catch((err) => alertModal("Грешка", String(err)));
  }
}

async function goUp(side) {
  const s = state[side];
  if (s.parent) {
    loadDir(side, s.parent);
  } else {
    alertModal("Грешка", "Вече сте в най-горната директория.");
  }
}

async function refresh(side) {
  if (state[side].path) loadDir(side, state[side].path);
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

async function copyOrMove(op) {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row) {
    alertModal("Грешка", "Няма избран елемент.");
    return;
  }
  if (row.kind === "parent") {
    alertModal("Грешка", "Изберете файл или папка (не „..“).");
    return;
  }
  const src = selectedPath(side);
  if (!src) return;
  const other = getOther();
  if (!other.path) {
    alertModal("Грешка", "Другият панел няма отворена директория.");
    return;
  }
  const verb = op === "copy" ? "Копиране" : "Преместване";
  const ok = await confirmModal(verb, `${verb} на „${row.kind === "parent" ? ".." : row.entry.name}“\nОт: ${state[side].path}\nВ: ${other.path}`);
  if (!ok) return;
  try {
    await invoke(op === "copy" ? "copy_path" : "move_path", {
      src,
      dstDir: other.path,
    });
    refresh(side);
    refresh(activeSide === "left" ? "right" : "left");
  } catch (err) {
    alertModal("Грешка", String(err));
  }
}

async function deleteSelected() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row) {
    alertModal("Грешка", "Няма избран елемент.");
    return;
  }
  if (row.kind === "parent") return;
  const src = selectedPath(side);
  const name = row.entry.name;
  const ok = await confirmModal(
    "Изтриване",
    `Сигурни ли сте, че искате да изтриете „${name}“?\nТова действие е необратимо.`
  );
  if (!ok) return;
  try {
    await invoke("delete_path", { path: src });
    refresh(side);
  } catch (err) {
    alertModal("Грешка", String(err));
  }
}

async function newFolder() {
  const side = activeSide;
  const name = await promptModal("Нова папка", `Папка в: ${state[side].path}`);
  if (name == null) return;
  try {
    await invoke("make_dir", { parent: state[side].path, name });
    refresh(side);
  } catch (err) {
    alertModal("Грешка", String(err));
  }
}

async function viewFile() {
  const side = activeSide;
  const row = selectedRow(side);
  if (!row || row.kind === "parent") return;
  const src = selectedPath(side);
  try {
    const content = await invoke("read_text_file", { path: src });
    showModal(`Преглед: ${row.entry.name}`, `pre`, content, true);
  } catch (err) {
    alertModal("Грешка", String(err));
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
      ["Име", info.name],
      ["Път", info.path],
      ["Тип", info.isDir ? "Директория" : "Файл"],
      ["Размер", info.isDir ? "—" : fmtSize(info.size)],
      ["Променен", fmtDate(info.modified)],
      ["Създаден", fmtDate(info.created)],
      ["Права", info.permissions],
    ];
    let html = `<table class="info">`;
    for (const [k, v] of rows) html += `<tr><td>${k}</td><td>${v}</td></tr>`;
    html += `</table>`;
    showModal("Информация", "html", html, true);
  } catch (err) {
    alertModal("Грешка", String(err));
  }
}

function helpModal() {
  const text = [
    "PANEL MANAGER — Помощ",
    "",
    "Навигация:",
    "  ↑/↓           преместване на курсора",
    "  Enter/2x click влизане в директория / отваряне на файл",
    "  Backspace     нагоре (родителска директория)",
    "  Tab           превключване между панелите",
    "  Home/End      първи/последен елемент",
    "  Del           изтриване",
    "",
    "Клавиши F1-F12:",
    "  F1 Помощ      този прозорец",
    "  F2 Меню       бързо меню с действия",
    "  F3 Преглед    преглед на текстов файл",
    "  F4 Отвори     отваряне с приложението по подразбиране",
    "  F5 Копирай    копиране в другия панел",
    "  F6 Премести   преместване в другия панел",
    "  F7 Папка      създаване на нова папка",
    "  F8 Изтрий     изтриване на избрания елемент",
    "  F9 Меню       бързо меню с действия",
    "  F10 Изход     изход от приложението",
    "  F11 Обнови    опресняване на активния панел",
    "  F12 Инфо      информация за избрания елемент",
  ].join("\n");
  showModal("Помощ", "pre", text, true);
}

function quickMenu() {
  const items = [
    ["Копирай в другия панел (F5)", () => copyOrMove("copy")],
    ["Премести в другия панел (F6)", () => copyOrMove("move")],
    ["Нова папка (F7)", () => newFolder()],
    ["Изтрий (F8)", () => deleteSelected()],
    ["Преглед (F3)", () => viewFile()],
    ["Информация (F12)", () => fileInfo()],
  ];
  let html = "";
  items.forEach(([label, fn], i) => {
    html += `<button class="menu-item" data-i="${i}">${label}</button>`;
  });
  showModal("Меню", "html", html, false);
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

function showModal(title, kind, content, closable) {
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
    closeBtn.textContent = "Затвори";
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
}

function confirmModal(title, message) {
  return new Promise((resolve) => {
    modalTitle.textContent = title;
    modalBody.innerHTML = "";
    const pre = document.createElement("pre");
    pre.textContent = message;
    modalBody.appendChild(pre);
    modalActions.innerHTML = "";

    const noBtn = document.createElement("button");
    noBtn.textContent = "Не";
    noBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve(false);
    });
    modalActions.appendChild(noBtn);

    const yesBtn = document.createElement("button");
    yesBtn.textContent = "Да";
    yesBtn.className = "primary";
    yesBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve(true);
    });
    modalActions.appendChild(yesBtn);

    overlay.classList.add("open");
    yesBtn.focus();
  });
}

function promptModal(title, label) {
  return new Promise((resolve) => {
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
    cancelBtn.textContent = "Отказ";
    cancelBtn.addEventListener("mousedown", () => {
      closeModal();
      resolve(null);
    });
    modalActions.appendChild(cancelBtn);

    const okBtn = document.createElement("button");
    okBtn.textContent = "ОК";
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
document.querySelector("#btn-mkdir").addEventListener("mousedown", newFolder);
document.querySelector("#btn-copy").addEventListener("mousedown", () => copyOrMove("copy"));
document.querySelector("#btn-move").addEventListener("mousedown", () => copyOrMove("move"));
document.querySelector("#btn-delete").addEventListener("mousedown", deleteSelected);
document.querySelector("#btn-info").addEventListener("mousedown", fileInfo);
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
    }
    return;
  }

  const side = activeSide;
  switch (ev.key) {
    case "ArrowDown":
      ev.preventDefault();
      moveSelection(side, 1);
      break;
    case "ArrowUp":
      ev.preventDefault();
      moveSelection(side, -1);
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
      }
  }
});

function handleFKey(key) {
  const map = {
    F1: helpModal,
    F2: quickMenu,
    F3: viewFile,
    F4: () => openRow(activeSide, getActive().selected),
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
    .onThemeChanged(({ payload }) => setThemeClass(payload))
    .catch(() => {});
  syncTheme();

  const home = await invoke("home_dir").catch(() => null);
  const start = home || (await invoke("list_dir", { path: "/" }).then((l) => l.path).catch(() => "/"));
  await loadDir("left", start);
  await loadDir("right", start);
  setActiveSide("left");
}

init();