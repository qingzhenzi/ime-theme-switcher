/* 与 Tauri 后端的类型化接口；浏览器里直接打开时用 mock 数据方便调样式。 */

export interface ThemeInfo {
  name: string;
  display_name: string;
  description: string;
  author: string;
  source: "user" | "system";
  is_current: boolean;
  normal_color: string;
  background_color: string;
  border_color: string;
  highlight_color: string;
  highlight_text_color: string;
  label_color: string;
  has_page_buttons: boolean;
}

export type CheckLevel = "ok" | "warn" | "error" | "info";

export interface CheckItem {
  id: string;
  title: string;
  level: CheckLevel;
  detail: string;
  fixable: boolean;
  fix_label: string;
}

export interface AppInfo {
  name: string;
  version: string;
  tested_on: string;
  shortcut: string;
  tray: boolean;
}

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

function getInvoke(): InvokeFn | null {
  const w = window as unknown as {
    __TAURI__?: { core?: { invoke?: InvokeFn } };
  };
  return w.__TAURI__?.core?.invoke ?? null;
}

export const hasTauri = (): boolean => getInvoke() !== null;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const invoke = getInvoke();
  if (!invoke) {
    throw new Error("当前是浏览器预览模式，切换/修复功能需要在 Tauri 应用内运行");
  }
  return invoke<T>(cmd, args);
}

/* ---------- 浏览器预览用的 mock ---------- */

const MOCK_THEMES: ThemeInfo[] = [
  {
    name: "Hatsune_Miku",
    display_name: "初音未来 (Hatsune Miku)",
    description: "RIME 初音未来主题的 fcitx5 移植版",
    author: "Miku <Kamisatoooooo@gmail.com>",
    source: "user",
    is_current: true,
    normal_color: "#22313DFF",
    background_color: "#EEF8FFFF",
    border_color: "#39C5BB",
    highlight_color: "#8CE6D9",
    highlight_text_color: "#102A32FF",
    label_color: "#D10063FF",
    has_page_buttons: true,
  },
  {
    name: "kagami",
    display_name: "PT's Kagami",
    description: "Fcitx5 Kagami Theme",
    author: "passthem",
    source: "user",
    is_current: false,
    normal_color: "#714576ff",
    background_color: "#f3f5f600",
    border_color: "#ffffff00",
    highlight_color: "#7499a100",
    highlight_text_color: "#ffffff",
    label_color: "",
    has_page_buttons: false,
  },
  {
    name: "mellow-sakura-dark",
    display_name: "Mellow Sakura Dark",
    description: "圆角现代风深色主题",
    author: "sanweiya",
    source: "user",
    is_current: false,
    normal_color: "#E5E9F0",
    background_color: "#1E1E1EEE",
    border_color: "#E8D2D0",
    highlight_color: "#E8D2D0",
    highlight_text_color: "#5B2B34",
    label_color: "",
    has_page_buttons: false,
  },
];

const MOCK_HEALTH: CheckItem[] = [
  {
    id: "fcitx5-running",
    title: "fcitx5 运行状态",
    level: "ok",
    detail: "fcitx5 正在运行",
    fixable: false,
    fix_label: "",
  },
  {
    id: "ime-env",
    title: "XIM 漏字修复（/etc/environment）",
    level: "warn",
    detail:
      "缺少 GTK_IM_MODULE=fcitx / QT_IM_MODULE=fcitx，ONLYOFFICE、WorkBuddy 等 Chromium 应用可能偶发漏字（需要管理员权限，会对所有用户生效）",
    fixable: true,
    fix_label: "写入 /etc/environment（需授权）",
  },
  {
    id: "tested-on",
    title: "测试范围",
    level: "info",
    detail: "v0.3.0：仅在 CachyOS（Arch 系）上测试过；其他发行版请自行确认依赖",
    fixable: false,
    fix_label: "",
  },
];

export async function listThemes(): Promise<ThemeInfo[]> {
  if (!getInvoke()) return MOCK_THEMES;
  return call<ThemeInfo[]>("list_themes");
}

export async function currentTheme(): Promise<string> {
  if (!getInvoke()) return "Hatsune_Miku";
  return call<string>("current_theme");
}

export async function applyTheme(name: string): Promise<string> {
  return call<string>("apply_theme", { name });
}

export async function checkHealth(): Promise<CheckItem[]> {
  if (!getInvoke()) return MOCK_HEALTH;
  return call<CheckItem[]>("check_health");
}

export async function runFix(id: string): Promise<string> {
  return call<string>("run_fix", { id });
}

export async function runAllFixes(): Promise<string> {
  return call<string>("run_all_fixes");
}

export async function getAutostart(): Promise<boolean> {
  if (!getInvoke()) return false;
  return call<boolean>("get_autostart");
}

export async function setAutostart(enabled: boolean): Promise<string> {
  return call<string>("set_autostart", { enabled });
}

export async function registerShortcut(accel: string): Promise<string> {
  return call<string>("register_shortcut", { accel });
}

export async function appInfo(): Promise<AppInfo | null> {
  if (!getInvoke()) return null;
  return call<AppInfo>("app_info");
}

export async function openFcitxConfig(): Promise<void> {
  if (!getInvoke()) throw new Error("浏览器预览模式无法启动 fcitx5 设置");
  await call<void>("open_fcitx_config");
}

export function errorText(err: unknown): string {
  if (!err) return "未知错误";
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
