<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/kou/master/docs/logo.webp" alt="Kou" width="240" /></p>

<h1 align="center">Kou</h1>

<p align="center"><strong>虚拟终端引擎</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![GitHub](https://img.shields.io/badge/github-celestia--island%2Fkou-blue.svg)](https://github.com/celestia-island/kou)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/kou/checks.yml)](https://github.com/celestia-island/kou/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-kou.docs.celestia.world-blue)](https://kou.docs.celestia.world)
[![docs.rs](https://docs.rs/kou/badge.svg)](https://docs.rs/kou)

</div>

<div align="center">

[English](../en/README.md) ·
**简体中文** ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## 简介

kou 是一个独立的虚拟终端引擎——集 PTY 管理、VT100/ANSI 屏幕模拟器以及
会绘制字形的屏幕渲染于一身。它是从 tairitsu 打包器中提取的 vtty 核心，
经过强化后成为独立的库和命令行工具。

让它区别于普通 PTY 封装的三点：

- **VT100 屏幕。** 字节流通过 [`vte`](https://crates.io/crates/vte) 解析器
  处理，因此 CSI 光标移动、擦除、滚动以及 SGR 16 色调色板都能被正确响应——
  而不是早期原型那种"丢弃 ESC 序列"的占位实现。
- **构建时字体预取。** kou 会在构建时为每种文字各预下载一个字体并存入共享
  缓存。可通过环境变量覆盖字体族或指定本地文件；身处受限网络时，可经由
  HTTP(S) 代理来路由下载。完整列表见[字体与拉取](#字体与拉取)。
- **带内图形。** 一帧既可以光栅化为 PNG，也可以通过 kitty（`kitty2`）或 iTerm2
  图形协议描述给支持这些协议的终端——这样 wezterm / kitty / iTerm2 / Ghostty
  就能原位渲染真实像素。

## 快速开始

### CLI

```bash
# Launch a command in a virtual terminal and drive it from a REPL.
kou launch bash --cols 80 --rows 24
# > echo hello
# > screen        # prints the current screen text
```

### 库

```rust
use kou::{FontCache, FontSet, VttyManager, render_png};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mgr = VttyManager::new();
    let id = mgr.launch("bash", None, 80, 24).await?;
    mgr.send_text(&id, "echo hello\n").await?;

    // Plain text.
    println!("{}", mgr.screenshot(&id).await?);

    // A real PNG, rendered with auto-fetched fonts (Latin + CJK fallback).
    let fonts = FontCache::load(&FontSet::from_env(), 16.0);
    let screen = mgr.screen(&id).await?;
    let png = render_png(&screen, &fonts, 16.0)?;
    std::fs::write("screen.png", png)?;
    Ok(())
}
```

## 图形协议

| `KOU_GRAPHICS` | 协议 | 支持的终端 |
|----------------|------|-----------|
| `kitty` / `kitty2` | kitty APC 图形协议 | kitty, wezterm, Ghostty |
| `iterm` / `iterm2` | OSC 1337 内联图像 | iTerm2, wezterm |
| `sixel` | DCS sixel | （占位——需要光栅化器） |
| `off`（默认） | 无——带外渲染为 PNG | 全部 |

```rust
use kou::{FontCache, FontSet, GraphicsProtocol, VttyManager, render_graphics};
let frame = render_graphics(&screen, &FontCache::load(&FontSet::from_env(), 16.0), 16.0,
                            GraphicsProtocol::from_env());
if let Some(escape) = frame {
    print!("{escape}"); // capable terminals render the pixels inline
}
```

## 字体与拉取

kou 会在构建时为每种文字各预下载一个字体到共享缓存中：

| 文字 | 字体 |
|------|------|
| Latin | [Fira Code](https://github.com/tonsky/FiraCode) |
| CJK (中文 · 日本語 · 한국어) | [Source Han Sans SC](https://github.com/adobe-fonts/source-han-sans) (思源黑体) |

在构建时通过 `KOU_FONT_PRIMARY` / `KOU_FONT_CJK` 覆盖任意字体族，或通过
`KOU_FONT_PATH` / `KOU_FONT_CJK_PATH` 指定本地文件。下载可经由
`KOU_DOWNLOAD_PROXY`（直接传给 reqwest）通过 HTTP(S) 代理路由。

| 环境变量 | 用途 |
|---------|------|
| `KOU_FONT_PRIMARY` | 覆盖 Latin 字体族。 |
| `KOU_FONT_CJK` | 覆盖 / 禁用 CJK 字体（`none` 为禁用）。 |
| `KOU_FONT_MIRROR` | 将下载主机替换为镜像源。 |
| `KOU_DOWNLOAD_PROXY` | 通过 HTTP(S) 代理路由下载（reqwest）。 |
| `KOU_DOWNLOAD_TIMEOUT_SECS` | 单次请求超时时间（默认 120）。 |
| `KOU_SKIP_FONT_FETCH` | 禁用字体拉取。 |

## MCP 服务器

使用 `mcp` feature 构建 kou 并运行 stdio 服务器——它通过模型上下文协议（Model Context Protocol）将虚拟终端引擎暴露给 AI 编码助手（无需浏览器或守护进程）：

```bash
kou mcp
```

服务器提供十一个工具——`vtty_launch`、`vtty_kill`、`vtty_send_keys`、`vtty_send_text`、`vtty_screenshot`、`vtty_wait`、`vtty_ready`、`vtty_scrollback`、`vtty_resize`、`vtty_list`、`vtty_ping`——每个工具都在进程内委托给库所暴露的同一个 `VttyManager`。截图通过与库相同的字体 + 主题栈渲染，因此 `vtty_screenshot` 会为具备视觉能力的模型返回真实的 PNG（或主题化文本）。

将其接入 MCP 客户端：

```json
{
  "mcpServers": {
    "kou": { "command": "kou", "args": ["mcp"] }
  }
}
```

当客户端未通告项目根目录时，设置 `KOU_PROJECT_ROOT` 来固定所启动会话的工作目录。

## 开发

```bash
cargo check --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## 许可证

SySL-1.0（Synthetic Source License）。详见 [LICENSE](https://sysl.celestia.world)。

## MCP Server Deployment

> (English section — translation pending)

For production or long-running MCP deployments (e.g. with opencode, Claude Desktop, or other MCP clients), we recommend using an **auto-restart wrapper** to keep the MCP server alive across updates and transient failures without interrupting the client session.

### Recommended launcher script

#!/bin/bash
while true; do
  /path/to/kou mcp
  sleep 0.2
done

### How it works

1. The wrapper runs the MCP server in a `while true` loop.
2. If the server process exits, the wrapper restarts it within 0.2 seconds.
3. The MCP client detects the reconnect and continues without data loss.
4. To restart after updating the binary: `kill $(pgrep -f "kou mcp" | head -1)`

### Integration with malkuth

For fully managed deployment, use [malkuth](https://github.com/celestia-island/malkuth) as a supervisor watching the binary for changes and performing rolling restarts.
