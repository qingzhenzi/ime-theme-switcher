#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const APP_NAME: &str = "输入法候选框样式";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+I";

/// 推荐主题（初始化时检查/安装）
const RECOMMENDED_THEMES: &[&str] = &[
    "Hatsune_Miku",
    "Hatsune_Miku_Dark",
    "kagami",
    "konata",
    "minami",
    "misao",
    "mellow-sakura",
    "mellow-sakura-dark",
];

/// 受 XIM 漏字影响的 Chromium/Electron/CEF 应用
const CEF_APPS: &[&str] = &["onlyoffice-desktopeditors", "workbuddy", "steam"];

static TRAY_READY: AtomicBool = AtomicBool::new(false);
static SHORTCUT_STATUS: Mutex<String> = Mutex::new(String::new());

/* ============================ 路径 / 主题 ============================ */

fn home_dir() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn xdg_config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(".config"))
}

fn xdg_data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

fn classicui_config_path() -> PathBuf {
    xdg_config_home().join("fcitx5/conf/classicui.conf")
}

fn theme_roots() -> Vec<(PathBuf, &'static str)> {
    vec![
        (xdg_data_home().join("fcitx5/themes"), "user"),
        (PathBuf::from("/usr/local/share/fcitx5/themes"), "system"),
        (PathBuf::from("/usr/share/fcitx5/themes"), "system"),
    ]
}

fn user_theme_dir() -> PathBuf {
    xdg_data_home().join("fcitx5/themes")
}

#[derive(Serialize, Clone, Debug)]
struct ThemeInfo {
    name: String,
    display_name: String,
    description: String,
    author: String,
    source: String,
    is_current: bool,
    normal_color: String,
    background_color: String,
    border_color: String,
    highlight_color: String,
    highlight_text_color: String,
    label_color: String,
    has_page_buttons: bool,
}

#[derive(Default)]
struct ThemeConf {
    sections: HashMap<String, HashMap<String, String>>,
}

impl ThemeConf {
    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(section)
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
    }
}

fn parse_theme_conf(path: &Path) -> ThemeConf {
    let mut conf = ThemeConf::default();
    let Ok(content) = fs::read_to_string(path) else {
        return conf;
    };
    let mut current_section = String::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let mut value = value.trim().to_string();
            if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                value = value[1..value.len() - 1].to_string();
            }
            conf.sections
                .entry(current_section.clone())
                .or_default()
                .insert(key, value);
        }
    }
    conf
}

fn find_theme_dir(name: &str) -> Option<PathBuf> {
    for (root, _) in theme_roots() {
        let dir = root.join(name);
        if dir.join("theme.conf").is_file() {
            return Some(dir);
        }
    }
    None
}

fn theme_has_page_buttons(name: &str) -> bool {
    let Some(dir) = find_theme_dir(name) else {
        return false;
    };
    let conf = parse_theme_conf(&dir.join("theme.conf"));
    let non_empty = |section: &str| {
        conf.get(section, "Image")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    };
    non_empty("InputPanel/PrevPage") && non_empty("InputPanel/NextPage")
}

fn current_theme_name() -> String {
    let path = classicui_config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Theme=") {
                let value = rest.trim().trim_matches('"').trim();
                if !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }
    "default".to_string()
}

fn collect_themes() -> Vec<ThemeInfo> {
    let current = current_theme_name();
    let mut seen: HashMap<String, ThemeInfo> = HashMap::new();

    for (root, source) in theme_roots() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let conf_path = dir.join("theme.conf");
            if !conf_path.is_file() {
                continue;
            }
            let name = match dir.file_name().and_then(|n| n.to_str()) {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => continue,
            };
            if seen.contains_key(&name) {
                continue;
            }
            let conf = parse_theme_conf(&conf_path);
            let display_name = conf
                .get("Metadata", "Name")
                .map(|s| s.to_string())
                .unwrap_or_else(|| name.clone());
            let description = conf
                .get("Metadata", "Description")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let author = conf
                .get("Metadata", "Author")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let normal_color = conf
                .get("InputPanel", "NormalColor")
                .unwrap_or("")
                .to_string();
            let background_color = conf
                .get("InputPanel/Background", "Color")
                .unwrap_or("")
                .to_string();
            let border_color = conf
                .get("InputPanel/Background", "BorderColor")
                .unwrap_or("")
                .to_string();
            let highlight_color = conf
                .get("InputPanel/Highlight", "Color")
                .unwrap_or("")
                .to_string();
            let highlight_text_color = conf
                .get("InputPanel", "HighlightCandidateColor")
                .unwrap_or("")
                .to_string();
            let label_color = conf
                .get("InputPanel/CandidateLabelColor", "Value")
                .unwrap_or("")
                .to_string();
            let is_current = current == name;
            let has_page_buttons = {
                let non_empty = |section: &str| {
                    conf.get(section, "Image")
                        .map(|v| !v.trim().is_empty())
                        .unwrap_or(false)
                };
                non_empty("InputPanel/PrevPage") && non_empty("InputPanel/NextPage")
            };

            seen.insert(
                name.clone(),
                ThemeInfo {
                    name,
                    display_name,
                    description,
                    author,
                    source: source.to_string(),
                    is_current,
                    normal_color,
                    background_color,
                    border_color,
                    highlight_color,
                    highlight_text_color,
                    label_color,
                    has_page_buttons,
                },
            );
        }
    }

    let mut themes: Vec<ThemeInfo> = seen.into_values().collect();
    themes.sort_by_key(|t| t.display_name.to_lowercase());
    themes
}

fn reload_classicui() -> bool {
    let via_qdbus = Command::new("qdbus6")
        .args([
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1.ReloadAddonConfig",
            "classicui",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if via_qdbus {
        return true;
    }
    Command::new("fcitx5-remote")
        .arg("-r")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn apply_theme_impl(name: &str) -> Result<String, String> {
    if find_theme_dir(name).is_none() {
        return Err(format!("主题不存在：{name}"));
    }

    let path = classicui_config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut replaced = false;
    let mut lines: Vec<String> = Vec::new();
    for line in content.lines() {
        if !replaced && line.trim_start().starts_with("Theme=") {
            lines.push(format!("Theme={name}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        lines.push(format!("Theme={name}"));
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    fs::write(&path, out).map_err(|e| format!("写入配置失败：{e}"))?;

    if reload_classicui() {
        Ok(format!("已切换到 {name}"))
    } else {
        Ok(format!("已写入 {name}，但 fcitx5 未响应，重开输入法后生效"))
    }
}

/* ============================ 自启动 ============================ */

fn autostart_file() -> PathBuf {
    xdg_config_home().join("autostart/ime-theme-switcher.desktop")
}

fn autostart_enabled() -> bool {
    autostart_file().is_file()
}

fn set_autostart_impl(enabled: bool) -> Result<String, String> {
    let path = autostart_file();
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("删除自启动项失败：{e}"))?;
        }
        return Ok("已关闭开机自启动".into());
    }
    let exe = env::current_exe().map_err(|e| format!("获取程序路径失败：{e}"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=ime-theme-switcher\n\
         Comment=输入法候选框样式（开机静默启动到托盘）\n\
         Exec=\"{}\" --minimized\n\
         Icon=ime-theme-switcher\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n\
         NoDisplay=true\n",
        exe.display()
    );
    fs::write(&path, content).map_err(|e| format!("写入自启动项失败：{e}"))?;
    Ok("已开启开机自启动（启动后最小化到托盘）".into())
}

/* ============================ 体检 / 初始化 ============================ */

#[derive(Serialize, Clone)]
struct CheckItem {
    id: String,
    title: String,
    level: String,
    detail: String,
    fixable: bool,
    fix_label: String,
}

fn pgrep(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn read_etc_environment() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(text) = fs::read_to_string("/etc/environment") {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    map
}

fn cef_app_installed(app: &str) -> bool {
    Path::new("/usr/share/applications")
        .join(format!("{app}.desktop"))
        .is_file()
}

fn cef_override_ok(app: &str) -> bool {
    let path = xdg_data_home()
        .join("applications")
        .join(format!("{app}.desktop"));
    fs::read_to_string(path)
        .map(|text| text.contains("GTK_IM_MODULE=fcitx"))
        .unwrap_or(false)
}

fn missing_cef_apps() -> Vec<String> {
    CEF_APPS
        .iter()
        .filter(|app| cef_app_installed(app) && !cef_override_ok(app))
        .map(|app| app.to_string())
        .collect()
}

fn missing_recommended_themes() -> Vec<String> {
    RECOMMENDED_THEMES
        .iter()
        .filter(|name| find_theme_dir(name).is_none())
        .map(|name| name.to_string())
        .collect()
}

fn health_checks() -> Vec<CheckItem> {
    let mut items = Vec::new();

    // 1. fcitx5 是否运行
    let running = pgrep("fcitx5");
    items.push(CheckItem {
        id: "fcitx5-running".into(),
        title: "fcitx5 运行状态".into(),
        level: if running { "ok".into() } else { "warn".into() },
        detail: if running {
            "fcitx5 正在运行".into()
        } else {
            "没有检测到 fcitx5 进程，中文输入法不可用".into()
        },
        fixable: !running,
        fix_label: "启动 fcitx5".into(),
    });

    // 2. 当前主题是否存在
    let current = current_theme_name();
    let theme_exists = find_theme_dir(&current).is_some();
    items.push(CheckItem {
        id: "theme-exists".into(),
        title: "当前候选框主题".into(),
        level: if theme_exists { "ok".into() } else { "error".into() },
        detail: if theme_exists {
            format!("当前主题：{current}")
        } else {
            format!("配置里的主题「{current}」不存在，候选框会退回默认样式")
        },
        fixable: !theme_exists,
        fix_label: "修复为主题默认".into(),
    });

    // 3. 当前主题是否带翻页按钮
    let has_buttons = theme_exists && theme_has_page_buttons(&current);
    items.push(CheckItem {
        id: "page-buttons".into(),
        title: "候选框翻页按钮".into(),
        level: if has_buttons { "ok".into() } else { "warn".into() },
        detail: if has_buttons {
            format!("当前主题「{current}」有 ◀ / ▶ 翻页按钮")
        } else if theme_exists {
            format!(
                "当前主题「{current}」没有定义翻页按钮，候选词超过一页时看不到 ◀ / ▶（不影响键盘翻页）"
            )
        } else {
            "当前主题不可用，翻页按钮状态未知".into()
        },
        fixable: !has_buttons,
        fix_label: "换成带翻页按钮的主题".into(),
    });

    // 4. 系统级输入法模块环境变量（XIM 漏字修复）
    let env_map = read_etc_environment();
    let gtk_ok = env_map
        .get("GTK_IM_MODULE")
        .map(|v| v == "fcitx")
        .unwrap_or(false);
    let qt_ok = env_map
        .get("QT_IM_MODULE")
        .map(|v| v == "fcitx")
        .unwrap_or(false);
    items.push(CheckItem {
        id: "ime-env".into(),
        title: "XIM 漏字修复（/etc/environment）".into(),
        level: if gtk_ok && qt_ok { "ok".into() } else { "warn".into() },
        detail: if gtk_ok && qt_ok {
            "GTK_IM_MODULE / QT_IM_MODULE 已设为 fcitx，Chromium/Electron 应用不会再走 XIM 漏字"
                .into()
        } else {
            "缺少 GTK_IM_MODULE=fcitx / QT_IM_MODULE=fcitx，ONLYOFFICE、WorkBuddy 等 Chromium 应用可能偶发漏字（需要管理员权限，会对所有用户生效）".into()
        },
        fixable: !(gtk_ok && qt_ok),
        fix_label: "写入 /etc/environment（需授权）".into(),
    });

    // 5. Chromium/Electron 应用的启动环境（用户级）
    let missing_cef = missing_cef_apps();
    items.push(CheckItem {
        id: "cef-launchers".into(),
        title: "Chromium 应用启动环境".into(),
        level: if missing_cef.is_empty() { "ok".into() } else { "warn".into() },
        detail: if missing_cef.is_empty() {
            "已安装的 Chromium/Electron 应用都加了 GTK_IM_MODULE=fcitx 启动环境".into()
        } else {
            format!(
                "这些应用还没加 GTK_IM_MODULE=fcitx：{}（重启应用后生效）",
                missing_cef.join("、")
            )
        },
        fixable: !missing_cef.is_empty(),
        fix_label: "添加用户级启动覆盖".into(),
    });

    // 6. 推荐主题是否安装
    let missing_themes = missing_recommended_themes();
    items.push(CheckItem {
        id: "recommended-themes".into(),
        title: "推荐主题包".into(),
        level: if missing_themes.is_empty() { "ok".into() } else { "warn".into() },
        detail: if missing_themes.is_empty() {
            format!("{} 个推荐主题都已安装", RECOMMENDED_THEMES.len())
        } else {
            format!(
                "缺少 {} 个推荐主题：{}（需要联网从 GitHub 下载）",
                missing_themes.len(),
                missing_themes.join("、")
            )
        },
        fixable: !missing_themes.is_empty(),
        fix_label: "下载并安装推荐主题".into(),
    });

    // 7. 开机自启（信息项，用设置开关）
    items.push(CheckItem {
        id: "autostart".into(),
        title: "开机自启动".into(),
        level: "info".into(),
        detail: if autostart_enabled() {
            "已设置开机自启动（静默启动到托盘）".into()
        } else {
            "未设置开机自启动，可在上方开关里开启".into()
        },
        fixable: false,
        fix_label: String::new(),
    });

    // 8. 全局快捷键（信息项）
    let shortcut_status = SHORTCUT_STATUS.lock().map(|s| s.clone()).unwrap_or_default();
    items.push(CheckItem {
        id: "shortcut".into(),
        title: "全局快捷键".into(),
        level: if shortcut_status.contains("已注册") { "ok".into() } else { "info".into() },
        detail: if shortcut_status.is_empty() {
            format!(
                "默认尝试注册 {DEFAULT_SHORTCUT}；Wayland 下通常不可用，建议在 KDE「系统设置 → 快捷键」里给本应用绑定"
            )
        } else {
            shortcut_status
        },
        fixable: false,
        fix_label: String::new(),
    });

    // 9. 测试范围提示
    items.push(CheckItem {
        id: "tested-on".into(),
        title: "测试范围".into(),
        level: "info".into(),
        detail: format!(
            "v{APP_VERSION}：仅在 CachyOS（Arch 系）上测试过；其他发行版请自行确认依赖（webkit2gtk-4.1、gtk3、librsvg、libayatana-appindicator）"
        ),
        fixable: false,
        fix_label: String::new(),
    });

    items
}

/* ============================ 修复动作 ============================ */

fn ensure_cef_overrides_impl() -> Result<String, String> {
    let system_dir = Path::new("/usr/share/applications");
    let user_dir = xdg_data_home().join("applications");
    fs::create_dir_all(&user_dir).map_err(|e| format!("创建目录失败：{e}"))?;
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    for app in CEF_APPS {
        let src = system_dir.join(format!("{app}.desktop"));
        if !src.is_file() {
            continue;
        }
        let dst = user_dir.join(format!("{app}.desktop"));
        if dst.is_file() && cef_override_ok(app) {
            skipped.push(app.to_string());
            continue;
        }
        let content = fs::read_to_string(&src)
            .map_err(|e| format!("读取 {app}.desktop 失败：{e}"))?;
        // 保留原文件所有内容（含注释），只在 Exec= 行前加环境变量
        let patched: String = content
            .lines()
            .map(|line| {
                if line.starts_with("Exec=") {
                    format!(
                        "Exec=env GTK_IM_MODULE=fcitx QT_IM_MODULE=fcitx XMODIFIERS=@im=fcitx {}",
                        &line[5..]
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(&dst, patched).map_err(|e| format!("写入 {app}.desktop 失败：{e}"))?;
        created.push(app.to_string());
    }

    let mut msg = String::new();
    if !created.is_empty() {
        msg.push_str(&format!("已为以下应用添加启动环境：{}\n", created.join("、")));
    }
    if !skipped.is_empty() {
        msg.push_str(&format!("已存在、跳过：{}\n", skipped.join("、")));
    }
    if created.is_empty() && skipped.is_empty() {
        msg.push_str("没有检测到需要处理的 Chromium/Electron 应用\n");
    }
    msg.push_str("提示：需要完全退出并重新打开对应应用才会生效。");
    Ok(msg)
}

fn fix_ime_env_impl() -> Result<String, String> {
    let env_map = read_etc_environment();
    let gtk_ok = env_map
        .get("GTK_IM_MODULE")
        .map(|v| v == "fcitx")
        .unwrap_or(false);
    let qt_ok = env_map
        .get("QT_IM_MODULE")
        .map(|v| v == "fcitx")
        .unwrap_or(false);
    if gtk_ok && qt_ok {
        return Ok("/etc/environment 里已经有 fcitx 的输入法模块设置".into());
    }
    if !Path::new("/usr/bin/pkexec").is_file() && !Path::new("/bin/pkexec").is_file() {
        return Err("没有找到 pkexec，请手动把 GTK_IM_MODULE=fcitx、QT_IM_MODULE=fcitx 加进 /etc/environment".into());
    }

    let script = r#"#!/bin/sh
set -e
ENV_FILE="/etc/environment"
key_ok() { grep -q "^$1=fcitx" "$ENV_FILE" 2>/dev/null; }
if ! key_ok GTK_IM_MODULE || ! key_ok QT_IM_MODULE; then
cat >> "$ENV_FILE" <<'IMEEOF'

#
# ime-theme-switcher 添加：让 Chromium/Electron/CEF 应用（ONLYOFFICE、WorkBuddy、Steam 等）
# 改用 fcitx5 的 GTK/Qt 输入法模块，避免走 XIM 时偶发漏字。
# 对全机所有用户的新登录会话生效。
GTK_IM_MODULE=fcitx
QT_IM_MODULE=fcitx
IMEEOF
fi
"#;
    let script_path = env::temp_dir().join("ime-theme-switcher-fix-env.sh");
    fs::write(&script_path, script).map_err(|e| format!("写入临时脚本失败：{e}"))?;
    let status = Command::new("pkexec")
        .arg("sh")
        .arg(&script_path)
        .status()
        .map_err(|e| format!("调用 pkexec 失败：{e}"))?;
    let _ = fs::remove_file(&script_path);
    if !status.success() {
        return Err("授权被取消或写入失败，未修改 /etc/environment".into());
    }
    let env_map = read_etc_environment();
    let gtk_ok = env_map
        .get("GTK_IM_MODULE")
        .map(|v| v == "fcitx")
        .unwrap_or(false);
    if gtk_ok {
        Ok("已写入 /etc/environment（GTK_IM_MODULE / QT_IM_MODULE=fcitx），所有用户下次登录生效".into())
    } else {
        Err("写入后校验失败，请手动检查 /etc/environment".into())
    }
}

fn fix_page_buttons_impl() -> Result<String, String> {
    let current = current_theme_name();
    if theme_has_page_buttons(&current) {
        return Ok(format!("当前主题「{current}」已经有翻页按钮"));
    }
    let fallback = ["Hatsune_Miku", "default", "default-dark"]
        .iter()
        .find(|name| find_theme_dir(name).is_some() && theme_has_page_buttons(name))
        .map(|s| s.to_string());
    match fallback {
        Some(name) => {
            apply_theme_impl(&name)?;
            Ok(format!(
                "当前主题「{current}」没有翻页按钮，已切换到带按钮的「{name}」"
            ))
        }
        None => Err("没找到带翻页按钮的主题，请先安装推荐主题".into()),
    }
}

fn fix_theme_config_impl() -> Result<String, String> {
    let current = current_theme_name();
    if find_theme_dir(&current).is_some() {
        return Ok(format!("当前主题「{current}」有效"));
    }
    let fallback = ["Hatsune_Miku", "default"]
        .iter()
        .find(|name| find_theme_dir(name).is_some())
        .map(|s| s.to_string());
    match fallback {
        Some(name) => {
            apply_theme_impl(&name)?;
            Ok(format!("主题「{current}」不存在，已回退到「{name}」"))
        }
        None => Err("连 default 主题都找不到，请检查 fcitx5 安装".into()),
    }
}

fn start_fcitx5_impl() -> Result<String, String> {
    if pgrep("fcitx5") {
        return Ok("fcitx5 已经在运行".into());
    }
    Command::new("fcitx5")
        .arg("-d")
        .status()
        .map_err(|e| format!("启动 fcitx5 失败：{e}"))?;
    std::thread::sleep(std::time::Duration::from_millis(800));
    if pgrep("fcitx5") {
        Ok("已启动 fcitx5".into())
    } else {
        Err("fcitx5 启动失败，请手动运行 fcitx5 -d 看看报错".into())
    }
}

/* ---------- 内置的初音未来·暗色主题 ---------- */
const MIKU_DARK_FILES: &[(&str, &str)] = &[
    (
        "theme.conf",
        include_str!("../themes/Hatsune_Miku_Dark/theme.conf"),
    ),
    (
        "panel.svg",
        include_str!("../themes/Hatsune_Miku_Dark/panel.svg"),
    ),
    (
        "highlight.svg",
        include_str!("../themes/Hatsune_Miku_Dark/highlight.svg"),
    ),
    (
        "prev.svg",
        include_str!("../themes/Hatsune_Miku_Dark/prev.svg"),
    ),
    (
        "next.svg",
        include_str!("../themes/Hatsune_Miku_Dark/next.svg"),
    ),
    (
        "radio.svg",
        include_str!("../themes/Hatsune_Miku_Dark/radio.svg"),
    ),
    (
        "arrow.svg",
        include_str!("../themes/Hatsune_Miku_Dark/arrow.svg"),
    ),
];

fn install_embedded_miku_dark() -> Result<(), String> {
    let dir = user_theme_dir().join("Hatsune_Miku_Dark");
    fs::create_dir_all(&dir).map_err(|e| format!("创建主题目录失败：{e}"))?;
    for (name, content) in MIKU_DARK_FILES {
        fs::write(dir.join(name), content).map_err(|e| format!("写入 {name} 失败：{e}"))?;
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目录 {dst:?} 失败：{e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取目录 {src:?} 失败：{e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("复制 {from:?} 失败：{e}"))?;
        }
    }
    Ok(())
}

fn download_repo(urls: &[&str], workdir: &Path) -> Result<PathBuf, String> {
    for url in urls {
        let archive = workdir.join("download.tar.gz");
        let ok = Command::new("curl")
            .args(["-fsSL", "--retry", "2", "--connect-timeout", "15"])
            .arg("-o")
            .arg(&archive)
            .arg(url)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !ok || archive.metadata().map(|m| m.len() < 1024).unwrap_or(true) {
            continue;
        }
        let extract = workdir.join("extract");
        let _ = fs::remove_dir_all(&extract);
        fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
        let ok = Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&extract)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !ok {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&extract) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    return Ok(path);
                }
            }
        }
    }
    Err(format!("下载失败：{}", urls.join(" / ")))
}

fn install_themes_impl() -> Result<String, String> {
    let workdir = env::temp_dir().join("ime-theme-switcher-install");
    let _ = fs::remove_dir_all(&workdir);
    fs::create_dir_all(&workdir).map_err(|e| format!("创建临时目录失败：{e}"))?;
    let themes_dir = user_theme_dir();
    fs::create_dir_all(&themes_dir).map_err(|e| format!("创建主题目录失败：{e}"))?;

    let mut installed: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // 1. 初音未来主题（原版）
    if find_theme_dir("Hatsune_Miku").is_some() {
        skipped.push("Hatsune_Miku".into());
    } else {
        let miku_urls = [
            "https://codeload.github.com/FragrantHaze/fcitx5-rime-miku-theme/tar.gz/refs/heads/main",
            "https://codeload.github.com/FragrantHaze/fcitx5-rime-miku-theme/tar.gz/refs/heads/master",
        ];
        match download_repo(&miku_urls, &workdir) {
            Ok(root) => {
                let target = themes_dir.join("Hatsune_Miku");
                fs::create_dir_all(&target).map_err(|e| e.to_string())?;
                for entry in fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name == "theme.conf" || name.ends_with(".svg") || name.ends_with(".png") {
                        let _ = fs::copy(&path, target.join(&name));
                    }
                }
                installed.push("Hatsune_Miku".into());
            }
            Err(err) => errors.push(format!("Hatsune_Miku：{err}")),
        }
    }

    // 2. PT's cute light（kagami / konata / minami / misao）
    let pt_names = ["kagami", "konata", "minami", "misao"];
    let pt_missing: Vec<&str> = pt_names
        .iter()
        .filter(|name| find_theme_dir(name).is_none())
        .copied()
        .collect();
    if pt_missing.is_empty() {
        skipped.extend(pt_names.iter().map(|s| s.to_string()));
    } else {
        let pt_urls = [
            "https://codeload.github.com/Passthem-desu/fcitx5-theme-pt-cute-light/tar.gz/refs/heads/master",
            "https://codeload.github.com/Passthem-desu/fcitx5-theme-pt-cute-light/tar.gz/refs/heads/main",
        ];
        match download_repo(&pt_urls, &workdir) {
            Ok(root) => {
                for name in &pt_missing {
                    let src = root.join(name);
                    if src.join("theme.conf").is_file() {
                        match copy_dir_all(&src, &themes_dir.join(name)) {
                            Ok(_) => installed.push(name.to_string()),
                            Err(err) => errors.push(format!("{name}：{err}")),
                        }
                    } else {
                        errors.push(format!("{name}：仓库里没有这个目录"));
                    }
                }
            }
            Err(err) => errors.push(format!("PT cute light：{err}")),
        }
    }

    // 3. Mellow 圆角主题
    let mellow_missing: Vec<&str> = RECOMMENDED_THEMES
        .iter()
        .filter(|name| name.starts_with("mellow-") && find_theme_dir(name).is_none())
        .copied()
        .collect();
    if mellow_missing.is_empty() {
        skipped.extend(
            RECOMMENDED_THEMES
                .iter()
                .filter(|name| name.starts_with("mellow-"))
                .map(|s| s.to_string()),
        );
    } else {
        let mellow_urls = [
            "https://codeload.github.com/sanweiya/fcitx5-mellow-themes/tar.gz/refs/heads/master",
            "https://codeload.github.com/sanweiya/fcitx5-mellow-themes/tar.gz/refs/heads/main",
        ];
        match download_repo(&mellow_urls, &workdir) {
            Ok(root) => {
                for entry in fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !path.is_dir() || !name.starts_with("mellow-") || !path.join("theme.conf").is_file() {
                        continue;
                    }
                    if mellow_missing.iter().any(|wanted| *wanted == name) {
                        match copy_dir_all(&path, &themes_dir.join(&name)) {
                            Ok(_) => installed.push(name.clone()),
                            Err(err) => errors.push(format!("{name}：{err}")),
                        }
                    }
                }
            }
            Err(err) => errors.push(format!("mellow：{err}")),
        }
    }

    // 4. 内置的初音未来·暗色（本地生成，不依赖网络）
    if find_theme_dir("Hatsune_Miku_Dark").is_some() {
        skipped.push("Hatsune_Miku_Dark".into());
    } else {
        match install_embedded_miku_dark() {
            Ok(_) => installed.push("Hatsune_Miku_Dark".into()),
            Err(err) => errors.push(format!("Hatsune_Miku_Dark：{err}")),
        }
    }

    let _ = fs::remove_dir_all(&workdir);

    let mut msg = String::new();
    if !installed.is_empty() {
        msg.push_str(&format!("已安装：{}\n", installed.join("、")));
    }
    if !skipped.is_empty() {
        msg.push_str(&format!("已存在、跳过：{}\n", skipped.join("、")));
    }
    if !errors.is_empty() {
        msg.push_str(&format!("失败：{}\n", errors.join("；")));
    }
    msg.push_str("安装完成后可点「刷新」重新扫描主题。");
    Ok(msg)
}

fn run_fix_impl(id: &str) -> Result<String, String> {
    match id {
        "fcitx5-running" => start_fcitx5_impl(),
        "theme-exists" => fix_theme_config_impl(),
        "page-buttons" => fix_page_buttons_impl(),
        "ime-env" => fix_ime_env_impl(),
        "cef-launchers" => ensure_cef_overrides_impl(),
        "recommended-themes" => install_themes_impl(),
        other => Err(format!("未知的修复项：{other}")),
    }
}

fn run_all_fixes_impl() -> Result<String, String> {
    let order = [
        "recommended-themes",
        "cef-launchers",
        "theme-exists",
        "page-buttons",
        "ime-env",
        "fcitx5-running",
    ];
    let current_ids: Vec<String> = health_checks().into_iter().map(|item| item.id).collect();
    let mut logs = Vec::new();
    for id in order {
        if !current_ids.iter().any(|current| current == id) {
            continue;
        }
        match run_fix_impl(id) {
            Ok(message) => logs.push(format!("✔ {message}")),
            Err(err) => logs.push(format!("✘ {err}")),
        }
    }
    Ok(logs.join("\n"))
}

/* ============================ 托盘 / 窗口 ============================ */

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        let focused = window.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    }
}

fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    match builder.build(app) {
        Ok(_) => {
            TRAY_READY.store(true, Ordering::SeqCst);
            Ok(())
        }
        Err(err) => {
            eprintln!("托盘图标初始化失败：{err}");
            Err(err)
        }
    }
}

/* ============================ Tauri commands ============================ */

#[tauri::command]
fn list_themes() -> Vec<ThemeInfo> {
    collect_themes()
}

#[tauri::command]
fn current_theme() -> String {
    current_theme_name()
}

#[tauri::command]
fn apply_theme(name: String) -> Result<String, String> {
    apply_theme_impl(&name)
}

#[tauri::command]
fn open_fcitx_config() -> Result<(), String> {
    Command::new("fcitx5-configtool")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动 fcitx5-configtool 失败：{e}"))
}

#[tauri::command]
fn check_health() -> Vec<CheckItem> {
    health_checks()
}

#[tauri::command]
fn run_fix(id: String) -> Result<String, String> {
    let result = run_fix_impl(&id);
    if result.is_ok() {
        let _ = reload_classicui();
    }
    result
}

#[tauri::command]
fn run_all_fixes() -> Result<String, String> {
    let result = run_all_fixes_impl();
    let _ = reload_classicui();
    result
}

#[tauri::command]
fn get_autostart() -> bool {
    autostart_enabled()
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<String, String> {
    set_autostart_impl(enabled)
}

#[tauri::command]
fn register_shortcut(accel: String, app: AppHandle) -> Result<String, String> {
    let accel = accel.trim().to_string();
    if accel.is_empty() {
        return Err("快捷键不能为空".into());
    }
    let _ = app.global_shortcut().unregister_all();
    match app.global_shortcut().register(accel.as_str()) {
        Ok(_) => {
            let message = format!("{accel} 已注册，按它呼出/隐藏窗口");
            if let Ok(mut status) = SHORTCUT_STATUS.lock() {
                *status = message.clone();
            }
            Ok(message)
        }
        Err(err) => {
            let message = format!(
                "注册 {accel} 失败：{err}。Wayland 下全局快捷键一般不可用，建议在 KDE「系统设置 → 快捷键」里给本应用绑定"
            );
            if let Ok(mut status) = SHORTCUT_STATUS.lock() {
                *status = message.clone();
            }
            Err(message)
        }
    }
}

#[tauri::command]
fn shortcut_status() -> String {
    SHORTCUT_STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({
        "name": APP_NAME,
        "version": APP_VERSION,
        "tested_on": "CachyOS (Arch 系)",
        "shortcut": DEFAULT_SHORTCUT,
        "tray": TRAY_READY.load(Ordering::SeqCst),
    })
}

/* ============================ 入口 ============================ */

fn print_theme_list() {
    match serde_json::to_string_pretty(&collect_themes()) {
        Ok(json) => println!("{json}"),
        Err(err) => eprintln!("{err}"),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--list-themes") {
        print_theme_list();
        return;
    }
    if args.iter().any(|arg| arg == "--current-theme") {
        println!("{}", current_theme_name());
        return;
    }
    if args.iter().any(|arg| arg == "--health") {
        match serde_json::to_string_pretty(&health_checks()) {
            Ok(json) => println!("{json}"),
            Err(err) => eprintln!("{err}"),
        }
        return;
    }
    if let Some(index) = args.iter().position(|arg| arg == "--apply-theme") {
        if let Some(name) = args.get(index + 1) {
            match apply_theme_impl(name) {
                Ok(message) => println!("{message}"),
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            }
            return;
        }
    }
    if let Some(index) = args.iter().position(|arg| arg == "--fix") {
        if let Some(id) = args.get(index + 1) {
            match run_fix_impl(id) {
                Ok(message) => println!("{message}"),
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            }
            return;
        }
    }
    if args.iter().any(|arg| arg == "--fix-all") {
        match run_all_fixes_impl() {
            Ok(message) => println!("{message}"),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("ime-theme-switcher {APP_VERSION}（仅在 CachyOS 测试过）");
        return;
    }

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = init_tray(&handle);
            match handle.global_shortcut().register(DEFAULT_SHORTCUT) {
                Ok(_) => {
                    if let Ok(mut status) = SHORTCUT_STATUS.lock() {
                        *status = format!("{DEFAULT_SHORTCUT} 已注册，按它呼出/隐藏窗口");
                    }
                }
                Err(err) => {
                    if let Ok(mut status) = SHORTCUT_STATUS.lock() {
                        *status = format!(
                            "注册 {DEFAULT_SHORTCUT} 失败：{err}。Wayland 下全局快捷键一般不可用，建议在 KDE「系统设置 → 快捷键」里绑定"
                        );
                    }
                }
            }
            // --minimized：开机自启时静默到托盘
            if env::args().any(|arg| arg == "--minimized")
                && TRAY_READY.load(Ordering::SeqCst)
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if TRAY_READY.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_themes,
            current_theme,
            apply_theme,
            open_fcitx_config,
            check_health,
            run_fix,
            run_all_fixes,
            get_autostart,
            set_autostart,
            register_shortcut,
            shortcut_status,
            app_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
