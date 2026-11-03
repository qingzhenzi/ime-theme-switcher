#!/usr/bin/env bash
# 在本机（Arch/CachyOS）构建 pacman 安装包：packaging/arch/*.pkg.tar.zst
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PKGVER="$(grep -m1 '^pkgver=' "$ROOT/packaging/arch/PKGBUILD" | cut -d= -f2)"
ARCHIVE="ime-theme-switcher-$PKGVER.tar.gz"
echo "==> 生成源码包 $ARCHIVE"
rm -f "$ROOT/packaging/arch/$ARCHIVE"
# 注意：git archive 不带 pathspec 时只归档当前目录，必须在仓库根目录执行
(cd "$ROOT" && git archive --format=tar.gz --prefix="ime-theme-switcher-$PKGVER/" -o "packaging/arch/$ARCHIVE" HEAD)
cd "$ROOT/packaging/arch"
echo "==> makepkg"
# 如果本机 rust/cargo 来自 rustup，pacman 依赖检查会报缺 rust，可传 --nodeps
makepkg -f --noconfirm "${MAKEPKG_ARGS:-}"
ls -la ./*.pkg.tar.zst
