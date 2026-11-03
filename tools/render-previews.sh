#!/usr/bin/env bash
# 在隔离的 Xvfb 里用真实的 fcitx5 渲染每个主题的候选框截图（供 dist/previews 使用）。
#
# 依赖：Xvfb（xorg-server-xvfb）、python3-gi（GTK3）、fcitx5、xdotool、ImageMagick
# 用法：tools/render-previews.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${TMPDIR:-/tmp}/ime-theme-preview"
RAW="$WORK/raw"
OUT="$ROOT/dist/previews"
DISPLAY_ID=":99"
mkdir -p "$RAW" "$OUT" "$WORK/config" "$WORK/data/fcitx5" "$WORK/cache"

# 用当前用户的 fcitx5 配置和主题做一个隔离副本，避免影响正在使用的输入法
cp -r "$HOME/.config/fcitx5" "$WORK/config/" 2>/dev/null || true
cp -r "$HOME/.local/share/fcitx5/themes" "$WORK/data/fcitx5/" 2>/dev/null || true

# 预览用的极简 GTK 窗口：深色背景 + 一个输入框，候选窗会出现在输入框下方
cat > "$WORK/preview_app.py" <<'PY'
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk

css = b"""
window { background-color: #1b1f24; }
entry {
  background-color: #232a33; color: #d8dee9;
  border: 1px solid #3b4252; border-radius: 8px;
  font-size: 17px; padding: 6px 10px; caret-color: #88c0d0; min-height: 26px;
}
"""
prov = Gtk.CssProvider()
prov.load_from_data(css)
Gtk.StyleContext.add_provider_for_screen(
    Gdk.Screen.get_default(), prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
win = Gtk.Window(title="ime-preview")
win.set_decorated(False)
win.set_default_size(900, 170)
win.move(0, 0)
box = Gtk.Box()
box.set_valign(Gtk.Align.START)
box.set_margin_start(16)
box.set_margin_top(14)
entry = Gtk.Entry()
entry.set_valign(Gtk.Align.START)
entry.set_size_request(520, -1)
box.pack_start(entry, False, False, 0)
win.add(box)
win.show_all()
entry.grab_focus()
Gtk.main()
PY

render_theme() {
  local theme="$1" out="$2"
  sed -i "s/^Theme=.*/Theme=$theme/" "$WORK/config/fcitx5/conf/classicui.conf"
  timeout 40 dbus-run-session -- bash -c '
    export DISPLAY='$DISPLAY_ID'
    unset WAYLAND_DISPLAY
    export XDG_CONFIG_HOME='$WORK'/config XDG_DATA_HOME='$WORK'/data XDG_CACHE_HOME='$WORK'/cache
    export XMODIFIERS=@im=fcitx GTK_IM_MODULE=fcitx QT_IM_MODULE=fcitx GDK_BACKEND=x11
    export XDG_SESSION_TYPE=x11
    fcitx5 -d >/dev/null 2>&1
    sleep 1.5
    python3 '"$WORK"'/preview_app.py >/dev/null 2>&1 &
    APP=$!
    sleep 1.8
    WID=$(xdotool search --name "ime-preview" 2>/dev/null | head -1)
    [ -n "$WID" ] && xdotool windowfocus --sync "$WID" 2>/dev/null || true
    sleep 0.3
    xdotool type --delay 90 "nihao"
    sleep 1.2
    import -window root "'"$out"'" 2>/dev/null
    kill $APP 2>/dev/null || true
  ' >/dev/null 2>&1
}

rm -f "/tmp/.X11-unix/X99" "/tmp/.X99-lock" 2>/dev/null
Xvfb "$DISPLAY_ID" -screen 0 900x170x24 -nolisten tcp >/dev/null 2>&1 &
XVFB_PID=$!
sleep 1.5
trap 'kill $XVFB_PID 2>/dev/null || true' EXIT

themes=$(for d in "$HOME/.local/share/fcitx5/themes"/*/ /usr/local/share/fcitx5/themes/*/ /usr/share/fcitx5/themes/*/; do
  [ -f "$d/theme.conf" ] && basename "$d"
done | sort -u)

for theme in $themes; do
  echo "rendering $theme ..."
  render_theme "$theme" "$RAW/$theme.png"
  if [ -s "$RAW/$theme.png" ] && [ "$(identify -format '%[fx:mean]' "$RAW/$theme.png" 2>/dev/null | cut -d. -f1)" != "0" ]; then
    # 裁掉输入框上方区域，去掉背景色，再留一点边距
    magick "$RAW/$theme.png" -crop 900x118+0+52 +repage -fuzz 4% -trim +repage \
      -bordercolor '#1b1f24' -border 8 "$OUT/$theme.png"
  else
    echo "  ! 渲染失败，跳过 $theme"
  fi
done
echo "完成，预览图在 $OUT"
