<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/kou/master/docs/logo.webp" alt="Kou" width="240" /></p>

<h1 align="center">Kou</h1>

<p align="center"><strong>虛擬終端引擎</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![GitHub](https://img.shields.io/badge/github-celestia--island%2Fkou-blue.svg)](https://github.com/celestia-island/kou)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/kou/checks.yml)](https://github.com/celestia-island/kou/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-kou.docs.celestia.world-blue)](https://kou.docs.celestia.world)
[![docs.rs](https://docs.rs/kou/badge.svg)](https://docs.rs/kou)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
**繁體中文** ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## 簡介

kou 是一個獨立的虛擬終端機引擎——PTY 管理、VT100/ANSI 螢幕模擬器，以及會繪製字形的螢幕渲染器。它是從 tairitsu 打包器中抽出的 vtty 核心，經過強化後獨立成為一個程式庫與命令列工具。

以下三點讓它不同於一個簡陋的 PTY 封裝：

- **VT100 螢幕。** 位元組串流會經過 [`vte`](https://crates.io/crates/vte) 解析器處理，因此 CSI 游標移動、清除、捲動以及 SGR 16 色調色盤都會被正確處理——不再是早期原型中那種「直接丟掉 ESC」的陽春做法。
- **建構時字型預取。** kou 會在建構時為每種書寫系統各預先下載一個字型並存入共用快取。可透過環境變數覆寫字型家族或指定本地檔案；身處受限網路時，可透過 HTTP(S) 代理伺服器路由下載。完整清單見[字型與擷取](#字型與擷取)。
- **帶內圖形。** 畫面可以點陣化為 PNG，或透過 kitty（`kitty2`）或 iTerm2 圖形協定描述給支援的終端機——因此 wezterm / kitty / iTerm2 / Ghostty 可以直接在行內顯示真實像素。

## 快速入門

### 命令列介面

```bash
# Launch a command in a virtual terminal and drive it from a REPL.
kou launch bash --cols 80 --rows 24
# > echo hello
# > screen        # prints the current screen text
```

### 程式庫

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

## 圖形協定

| `KOU_GRAPHICS` | 協定 | 終端機 |
|----------------|----------|-----------|
| `kitty` / `kitty2` | kitty APC 圖形 | kitty、wezterm、Ghostty |
| `iterm` / `iterm2` | OSC 1337 行內圖片 | iTerm2、wezterm |
| `sixel` | DCS sixel | （預留——需要點陣化器） |
| `off`（預設） | 無——在頻外渲染 PNG | 全部 |

```rust
use kou::{FontCache, FontSet, GraphicsProtocol, VttyManager, render_graphics};
let frame = render_graphics(&screen, &FontCache::load(&FontSet::from_env(), 16.0), 16.0,
                            GraphicsProtocol::from_env());
if let Some(escape) = frame {
    print!("{escape}"); // capable terminals render the pixels inline
}
```

## 字型與擷取

kou 會在建構時為每種書寫系統各預先下載一個字型到共用快取中：

| 文字 | 字型 |
|------|------|
| Latin | [Fira Code](https://github.com/tonsky/FiraCode) |
| CJK (中文 · 日本語 · 한국어) | [Source Han Sans SC](https://github.com/adobe-fonts/source-han-sans) (思源黑体) |

在建構時透過 `KOU_FONT_PRIMARY` / `KOU_FONT_CJK` 覆寫任意字型家族，或透過
`KOU_FONT_PATH` / `KOU_FONT_CJK_PATH` 指定本地檔案。下載可透過
`KOU_DOWNLOAD_PROXY`（直接傳遞給 reqwest）經由 HTTP(S) 代理伺服器路由。

| 環境變數 | 用途 |
|-----|---------|
| `KOU_FONT_PRIMARY` | 覆寫 Latin 字型家族。 |
| `KOU_FONT_CJK` | 覆寫 / 停用 CJK 字型（`none` 為停用）。 |
| `KOU_FONT_MIRROR` | 將下載主機取代為鏡像站。 |
| `KOU_DOWNLOAD_PROXY` | 透過 HTTP(S) 代理伺服器路由下載（reqwest）。 |
| `KOU_DOWNLOAD_TIMEOUT_SECS` | 每個請求的逾時時間（預設 120）。 |
| `KOU_SKIP_FONT_FETCH` | 停用字型擷取。 |

## MCP 伺服器

使用 `mcp` feature 建置 kou 並執行 stdio 伺服器——它透過模型上下文協定（Model Context Protocol）將虛擬終端引擎暴露給 AI 編碼助手（無需瀏覽器或常駐程式）：

```bash
kou mcp
```

伺服器提供十一個工具——`vtty_launch`、`vtty_kill`、`vtty_send_keys`、`vtty_send_text`、`vtty_screenshot`、`vtty_wait`、`vtty_ready`、`vtty_scrollback`、`vtty_resize`、`vtty_list`、`vtty_ping`——每個工具都在行程內委託給函式庫所暴露的同一個 `VttyManager`。截圖透過與函式庫相同的字型 + 主題棧渲染，因此 `vtty_screenshot` 會為具備視覺能力的模型回傳真實的 PNG（或主題化文字）。

將其接入 MCP 客戶端：

```json
{
  "mcpServers": {
    "kou": { "command": "kou", "args": ["mcp"] }
  }
}
```

當客戶端未通告專案根目錄時，設定 `KOU_PROJECT_ROOT` 來固定所啟動會話的工作目錄。

## 開發

```bash
cargo check --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## 授權條款

SySL-1.0（Synthetic Source License）。詳見 [LICENSE](https://sysl.celestia.world)。

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
