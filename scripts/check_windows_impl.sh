#!/usr/bin/env bash
# 在 macOS 上对 Windows 专有实现做类型检查。
#
# 整个项目无法交叉编译——libsqlite3-sys 的 bundled SQLite 要用 MSVC 编译 C 代码，
# 开发机上没有。但 platform/windows_impl.rs 只依赖 windows crate，把它单独摘到
# 一个最小项目里就能检查：API 导入路径、常量名、返回类型处理。
#
# 这**不能**验证运行时行为。全屏时是否真返回 FullScreenD3D 只有在 Windows 上
# 打开一个全屏游戏才知道，见 MOCKS.md S1 的验证清单。
#
# 用法：bash scripts/check_windows_impl.sh

set -euo pipefail

SRC="wordcraft/src-tauri/src/platform/windows_impl.rs"
WORK="${TMPDIR:-/tmp}/wordcraft_wincheck"

if [ ! -f "$SRC" ]; then
  echo "找不到 $SRC，请在项目根目录运行" >&2
  exit 1
fi

rustup target list --installed | grep -q x86_64-pc-windows-msvc || {
  echo "缺少 Windows target，执行：rustup target add x86_64-pc-windows-msvc" >&2
  exit 1
}

mkdir -p "$WORK/src"
cat > "$WORK/Cargo.toml" <<'TOML'
[package]
name = "wincheck"
version = "0.0.0"
edition = "2021"

[dependencies]
log = "0.4"
windows = { version = "0.61.3", features = ["Win32_UI_Shell", "Win32_Foundation"] }
TOML

# 用桩替换本项目的类型，其余原样摘取——改动越少，检查越接近真实编译
{
  cat <<'RUST'
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusyState { Normal, FullScreenD3D, Busy, Presentation, Unknown }
#[derive(Debug)]
pub struct PlatformError(pub String);
pub trait PlatformIntegration { fn user_busy_state(&self) -> Result<BusyState, PlatformError>; }
RUST
  sed -n '/^use windows/,$p' "$SRC"
} > "$WORK/src/lib.rs"

echo "对 $SRC 做 Windows 目标类型检查…"
(cd "$WORK" && cargo check --target x86_64-pc-windows-msvc 2>&1 | tail -20)
echo "✅ 类型检查通过（运行时行为仍需在 Windows 上实测）"
