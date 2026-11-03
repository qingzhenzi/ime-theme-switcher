#!/usr/bin/env bash
# 在本机（Arch/CachyOS）构建 pacman 安装包：packaging/arch/*.pkg.tar.zst
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/packaging/arch"
PKGVER="$(grep -m1 '^pkgver=' PKGBUILD | cut -d= -f2)"
ARCHIVE="ime-theme-switcher-$PKGVER.tar.gz"
echo "==> 生成源码包 $ARCHIVE"
rm -f "$ARCHIVE"
git archive --format=tar.gz --prefix="ime-theme-switcher-$PKGVER/" -o "$ARCHIVE" HEAD
echo "==> makepkg"
makepkg -f --noconfirm
ls -la ./*.pkg.tar.zst
