#!/usr/bin/env bash
# 输入法候选框样式切换器 —— 构建 + 安装到当前用户
#
# 用法：
#   ./install.sh              构建并安装（二进制 / 图标 / 应用菜单 / 桌面快捷方式）
#   ./install.sh --uninstall  卸载（只删本脚本装的东西）
#
# 依赖：cargo、webkit2gtk-4.1、gtk3、librsvg（构建时）
set -euo pipefail

APP_ID="ime-theme-switcher"
APP_NAME="输入法候选框样式"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${HOME}/.local"
BIN="$PREFIX/bin/$APP_ID"
ICON_DIR="$PREFIX/share/icons/hicolor/256x256/apps"
DESKTOP_DIR="$PREFIX/share/applications"
DESKTOP_FILE="$DESKTOP_DIR/$APP_ID.desktop"
USER_DESKTOP="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/桌面")"
SHORTCUT="$USER_DESKTOP/$APP_NAME.desktop"

uninstall() {
  echo "卸载 $APP_NAME ..."
  rm -f "$BIN" "$DESKTOP_FILE" "$SHORTCUT" "$ICON_DIR/$APP_ID.png"
  rm -f "$USER_DESKTOP/$APP_ID.desktop"
  update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
  command -v kbuildsycoca6 >/dev/null 2>&1 && kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
  echo "已卸载（用户配置 ~/.config/fcitx5/conf/classicui.conf 未改动）"
}

if [[ "${1:-}" == "--uninstall" ]]; then
  uninstall
  exit 0
fi

echo "[1/4] 构建（cargo build --release）..."
(cd "$ROOT/src-tauri" && cargo build --release)

echo "[2/4] 安装二进制：$BIN"
install -Dm755 "$ROOT/src-tauri/target/release/$APP_ID" "$BIN"

echo "[3/4] 安装图标与桌面入口"
install -Dm644 "$ROOT/src-tauri/icons/128x128@2x.png" "$ICON_DIR/$APP_ID.png"
mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Name[zh_CN]=$APP_NAME
Comment=切换 fcitx5 候选框主题（经典界面皮肤）
Comment[zh_CN]=切换 fcitx5 候选框主题（经典界面皮肤）
Exec=$BIN
Icon=$APP_ID
Terminal=false
Categories=Utility;
Keywords=fcitx5;rime;theme;skin;输入法;主题;皮肤;候选框;
StartupWMClass=$APP_ID
EOF
chmod +x "$DESKTOP_FILE"
update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
command -v kbuildsycoca6 >/dev/null 2>&1 && kbuildsycoca6 --noincremental >/dev/null 2>&1 || true

echo "[4/4] 创建桌面快捷方式"
mkdir -p "$USER_DESKTOP"
cp "$DESKTOP_FILE" "$SHORTCUT"
chmod +x "$SHORTCUT"
# KDE Plasma 6 需要在文件管理器里标记为可信才能双击启动
gio set "$SHORTCUT" metadata::trusted true >/dev/null 2>&1 || true

cat <<MSG

完成 ✔
  启动器：应用菜单搜索「$APP_NAME」，或双击桌面图标
  命令行：$BIN
  调试用：$BIN --list-themes / --apply-theme kagami / --current-theme
MSG
