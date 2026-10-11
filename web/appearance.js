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

const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

export const FONT_MAP = {
  default: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
  monospace: "Menlo, Monaco, Consolas, 'DejaVu Sans Mono', monospace",
  serif: "Georgia, 'Times New Roman', serif",
  sans: "'Segoe UI', 'Helvetica Neue', Arial, sans-serif",
};

export function setThemeClass(theme) {
  const rootEl = document.documentElement;
  rootEl.classList.toggle("theme-dark", theme === "dark");
  rootEl.classList.toggle("theme-light", theme === "light");
}

export function applyFont(appearance) {
  const rootEl = document.documentElement;
  rootEl.style.setProperty("--font", FONT_MAP[appearance.font] || FONT_MAP.default);
  rootEl.style.setProperty("--font-size", `${appearance.fontSize}px`);
}

export function applyThemePref(pref, osTheme) {
  if (pref === "light") {
    setThemeClass("light");
  } else if (pref === "dark") {
    setThemeClass("dark");
  } else {
    setThemeClass(osTheme);
  }
}

export async function resolveOsTheme() {
  let osTheme = null;
  try {
    osTheme = await getCurrentWindow().theme();
  } catch {
    osTheme = null;
  }
  if (!osTheme) {
    osTheme =
      window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
  }
  return osTheme;
}

export async function initAppearance(onChange) {
  let appearance = null;
  let osTheme = null;
  function apply() {
    if (!appearance) return;
    applyFont(appearance);
    applyThemePref(appearance.theme, osTheme);
  }
  async function sync() {
    const next = await invoke("get_appearance").catch(() => null);
    if (next) appearance = next;
    osTheme = await resolveOsTheme();
    apply();
    if (onChange) onChange(appearance);
  }
  const win = getCurrentWindow();
  win
    .onThemeChanged(({ payload }) => {
      osTheme = payload;
      if (appearance && appearance.theme === "system") apply();
    })
    .catch(() => {});
  win
    .listen("appearance-changed", () => sync())
    .catch(() => {});
  await sync();
  return { get: () => appearance, sync };
}
