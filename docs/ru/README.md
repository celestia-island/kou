<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/kou/master/docs/logo.webp" alt="Kou" width="240" /></p>

<h1 align="center">Kou</h1>

<p align="center"><strong>Движок виртуального терминала</strong></p>

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
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
**Русский** ·
[العربية](../ar/README.md)

</div>

## Введение

kou — это автономный движок виртуального терминала: управление PTY, эмулятор
экрана VT100/ANSI и отрисовка экрана с прорисовкой глифов. Это ядро vtty,
vtty, выделенное из упаковщика tairitsu и оформленное в виде самостоятельной
библиотеки и CLI.

Три особенности отличают его от простой обёртки над PTY:

- **Экран VT100.** Поток байтов обрабатывается парсером [`vte`](https://crates.io/crates/vte),
  поэтому перемещения курсора CSI, стирание, прокрутка и 16-цветная палитра SGR
  поддерживаются полноценно — а не отбрасываются, как в упрощённой заглушке
  раннего прототипа.
- **Получение шрифтов во время сборки.** kou предварительно загружает по одному
  шрифту на письменность в общий кеш во время сборки. Переопределяйте семейства
  или фиксируйте локальные файлы через переменные окружения; направляйте загрузки
  через HTTP(S) прокси, находясь за сетью с ограничениями. Полный список см. в
  [Шрифты и загрузка](#шрифты-и-загрузка).
- **Встроенная графика.** Кадр может быть растеризован в PNG или передан
  совместимому терминалу через графический протокол kitty (`kitty2`) или
  iTerm2 — таким образом wezterm / kitty / iTerm2 / Ghostty отображают
  пиксели непосредственно в потоке.

## Быстрый старт

### CLI

```bash
# Launch a command in a virtual terminal and drive it from a REPL.
kou launch bash --cols 80 --rows 24
# > echo hello
# > screen        # prints the current screen text
```

### Библиотека

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

## Графические протоколы

| `KOU_GRAPHICS` | Протокол | Терминалы |
|----------------|----------|-----------|
| `kitty` / `kitty2` | kitty APC graphics | kitty, wezterm, Ghostty |
| `iterm` / `iterm2` | OSC 1337 inline image | iTerm2, wezterm |
| `sixel` | DCS sixel | (заглушка — требуется растеризатор) |
| `off` (по умолчанию) | нет — рендер PNG вне потока | все |

```rust
use kou::{FontCache, FontSet, GraphicsProtocol, VttyManager, render_graphics};
let frame = render_graphics(&screen, &FontCache::load(&FontSet::from_env(), 16.0), 16.0,
                            GraphicsProtocol::from_env());
if let Some(escape) = frame {
    print!("{escape}"); // capable terminals render the pixels inline
}
```

## Шрифты и загрузка

kou предварительно загружает по одному шрифту на письменность в общий кеш во время сборки:

| Письменность | Шрифт |
|--------------|-------|
| Latin | [Fira Code](https://github.com/tonsky/FiraCode) |
| CJK (中文 · 日本語 · 한국어) | [Source Han Sans SC](https://github.com/adobe-fonts/source-han-sans) (思源黑体) |

Переопределяйте любое семейство во время сборки с помощью `KOU_FONT_PRIMARY` /
`KOU_FONT_CJK` или фиксируйте локальные файлы через `KOU_FONT_PATH` /
`KOU_FONT_CJK_PATH`. Загрузки можно направлять через HTTP(S) прокси через
`KOU_DOWNLOAD_PROXY` (передаётся напрямую в reqwest).

| Переменная | Назначение |
|------------|------------|
| `KOU_FONT_PRIMARY` | Переопределяет семейство латинских шрифтов. |
| `KOU_FONT_CJK` | Переопределить / отключить шрифт CJK (`none` для отключения). |
| `KOU_FONT_MIRROR` | Заменить хост загрузки на зеркало. |
| `KOU_DOWNLOAD_PROXY` | Направить загрузки через HTTP(S) прокси (reqwest). |
| `KOU_DOWNLOAD_TIMEOUT_SECS` | Таймаут одного запроса (по умолчанию 120). |
| `KOU_SKIP_FONT_FETCH` | Отключить загрузку. |

## MCP-сервер

Соберите kou с feature `mcp` и запустите stdio-сервер — он предоставляет движок виртуального терминала AI-ассистентам программиста по протоколу Model Context Protocol (без браузера или демона):

```bash
kou mcp
```

Сервер предоставляет одиннадцать инструментов — `vtty_launch`, `vtty_kill`, `vtty_send_keys`, `vtty_send_text`, `vtty_screenshot`, `vtty_wait`, `vtty_ready`, `vtty_scrollback`, `vtty_resize`, `vtty_list`, `vtty_ping` — каждый делегирует внутри процесса тому же `VttyManager`, который предоставляет библиотека. Скриншоты рендерятся через тот же стек шрифтов + тем, что и библиотека, поэтому `vtty_screenshot` возвращает реальный PNG (или тематический текст) для моделей с поддержкой зрения.

Подключите его к MCP-клиенту:

```json
{
  "mcpServers": {
    "kou": { "command": "kou", "args": ["mcp"] }
  }
}
```

Установите `KOU_PROJECT_ROOT`, чтобы зафиксировать рабочий каталог запускаемых сеансов, когда клиент не сообщает корень проекта.

## Разработка

```bash
cargo check --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Лицензия

SySL-1.0 (Synthetic Source License). См. [LICENSE](https://sysl.celestia.world).

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
