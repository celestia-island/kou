<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/kou/master/docs/logo.webp" alt="Kou" width="240" /></p>

<h1 align="center">Kou</h1>

<p align="center"><strong>Motor de terminal virtual</strong></p>

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
**Español** ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## Introducción

kou es un motor de terminal virtual autónomo — gestión de PTY, un emulador de
pantalla VT100/ANSI, y renderizado de pantalla que dibuja los glifos. Es el
núcleo vtty extraído del empaquetador tairitsu, reforzado como librería y CLI
propios.

Tres cosas lo distinguen de un simple envoltorio de PTY:

- **Pantalla VT100.** El flujo de bytes pasa por el analizador [`vte`](https://crates.io/crates/vte),
  por lo que los movimientos CSI del cursor, el borrado, el desplazamiento y la
  paleta SGR de 16 colores se respetan — no es el stub de "tirar ESC al suelo"
  del primer prototipo.
- **Obtención de fuentes en tiempo de compilación.** kou pre-descarga una fuente por
  escritura en una caché compartida en tiempo de compilación. Sobrescribe las
  familias o fija archivos locales mediante variables de entorno; enruta las
  descargas a través de un proxy HTTP(S) cuando estés tras una red restrictiva.
  Consulta [Fuentes y obtención](#fuentes-y-obtención) para la lista completa.
- **Gráficos in-band.** Un fotograma puede rasterizarse a PNG o describirse a
  un terminal compatible mediante el protocolo gráfico kitty (`kitty2`) o
  iTerm2 — así wezterm / kitty / iTerm2 / Ghostty renderizan los píxeles
  reales en línea.

## Inicio rápido

### CLI

```bash
# Launch a command in a virtual terminal and drive it from a REPL.
kou launch bash --cols 80 --rows 24
# > echo hello
# > screen        # prints the current screen text
```

### Librería

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

## Protocolos gráficos

| `KOU_GRAPHICS` | Protocolo | Terminales |
|----------------|-----------|------------|
| `kitty` / `kitty2` | kitty APC graphics | kitty, wezterm, Ghostty |
| `iterm` / `iterm2` | OSC 1337 imagen en línea | iTerm2, wezterm |
| `sixel` | DCS sixel | (marcador de posición — necesita un rasterizador) |
| `off` (predeterminado) | ninguno — renderiza un PNG fuera de banda | todos |

```rust
use kou::{FontCache, FontSet, GraphicsProtocol, VttyManager, render_graphics};
let frame = render_graphics(&screen, &FontCache::load(&FontSet::from_env(), 16.0), 16.0,
                            GraphicsProtocol::from_env());
if let Some(escape) = frame {
    print!("{escape}"); // capable terminals render the pixels inline
}
```

## Fuentes y obtención

kou pre-descarga una fuente por escritura en una caché compartida en tiempo de compilación:

| Escritura | Fuente |
|-----------|--------|
| Latin | [Fira Code](https://github.com/tonsky/FiraCode) |
| CJK (中文 · 日本語 · 한국어) | [Source Han Sans SC](https://github.com/adobe-fonts/source-han-sans) (思源黑体) |

Sobrescribe cualquier familia en tiempo de compilación con `KOU_FONT_PRIMARY` /
`KOU_FONT_CJK`, o fija archivos locales con `KOU_FONT_PATH` / `KOU_FONT_CJK_PATH`.
Las descargas pueden enrutarse a través de un proxy HTTP(S) vía
`KOU_DOWNLOAD_PROXY` (pasado directamente a reqwest).

| Env | Propósito |
|-----|-----------|
| `KOU_FONT_PRIMARY` | Sobrescribe la familia de fuentes latinas. |
| `KOU_FONT_CJK` | Sobrescribe / deshabilita la fuente CJK (`none` para deshabilitar). |
| `KOU_FONT_MIRROR` | Sustituye el host de descarga por un mirror. |
| `KOU_DOWNLOAD_PROXY` | Enruta las descargas a través de un proxy HTTP(S) (reqwest). |
| `KOU_DOWNLOAD_TIMEOUT_SECS` | Tiempo de espera por solicitud (predeterminado 120). |
| `KOU_SKIP_FONT_FETCH` | Deshabilita la obtención. |

## Servidor MCP

Construye kou con la feature `mcp` y ejecuta el servidor stdio — expone el motor de terminal virtual a los asistentes de codificación de IA a través del Model Context Protocol (no se requiere navegador ni demonio):

```bash
kou mcp
```

El servidor anuncia once herramientas — `vtty_launch`, `vtty_kill`, `vtty_send_keys`, `vtty_send_text`, `vtty_screenshot`, `vtty_wait`, `vtty_ready`, `vtty_scrollback`, `vtty_resize`, `vtty_list`, `vtty_ping` — cada una delegando en el proceso al mismo `VttyManager` que expone la biblioteca. Las capturas de pantalla se renderizan a través de la misma pila de fuentes + temas que la biblioteca, por lo que `vtty_screenshot` devuelve un PNG real (o texto con tema) para modelos con capacidad de visión.

Conéctalo a un cliente MCP:

```json
{
  "mcpServers": {
    "kou": { "command": "kou", "args": ["mcp"] }
  }
}
```

Establece `KOU_PROJECT_ROOT` para fijar el directorio de trabajo de las sesiones iniciadas cuando el cliente no anuncia una raíz de proyecto.

## Desarrollo

```bash
cargo check --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Licencia

SySL-1.0 (Synthetic Source License). Consulta [LICENSE](https://sysl.celestia.world).

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

