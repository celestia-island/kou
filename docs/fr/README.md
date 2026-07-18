<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/kou/master/docs/logo.webp" alt="Kou" width="240" /></p>

<h1 align="center">Kou</h1>

<p align="center"><strong>Moteur de terminal virtuel</strong></p>

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
**Français** ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## Introduction

kou est un moteur de terminal virtuel autonome — gestion PTY, un émulateur
d'écran VT100/ANSI, et un rendu d'écran qui dessine les glyphes. Il s'agit du
cœur vtty extrait de l'empaqueteur tairitsu, durci en une bibliothèque et un
CLI à part entière.

Trois choses le distinguent d'un simple wrapper PTY :

- **Écran VT100.** Le flux d'octets est traité par l'analyseur
  [`vte`](https://crates.io/crates/vte), donc les déplacements de curseur CSI,
  l'effacement, le défilement et la palette SGR 16 couleurs sont respectés —
  pas le bouchon « jeter ESC au sol » du premier prototype.
- **Récupération des polices à la compilation.** kou pré-télécharge une police par
  écriture dans un cache partagé à la compilation. Surchargez les familles ou
  épinglez des fichiers locaux via des variables d'environnement ; acheminez les
  téléchargements via un proxy HTTP(S) derrière un réseau restrictif. Voir
  [Polices et récupération](#polices-et-récupération) pour la liste complète.
- **Graphiques in-band.** Une trame peut être rastérisée en PNG, ou décrite à un
  terminal compatible via le protocole graphique kitty (`kitty2`) ou iTerm2 —
  ainsi wezterm / kitty / iTerm2 / Ghostty affichent les vrais pixels en ligne.

## Démarrage rapide

### CLI

```bash
# Launch a command in a virtual terminal and drive it from a REPL.
kou launch bash --cols 80 --rows 24
# > echo hello
# > screen        # prints the current screen text
```

### Bibliothèque

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

## Protocoles graphiques

| `KOU_GRAPHICS` | Protocole | Terminaux |
|----------------|-----------|-----------|
| `kitty` / `kitty2` | graphiques APC kitty | kitty, wezterm, Ghostty |
| `iterm` / `iterm2` | image inline OSC 1337 | iTerm2, wezterm |
| `sixel` | DCS sixel | (réservé — nécessite un rastériseur) |
| `off` (par défaut) | aucun — rend un PNG hors bande | tous |

```rust
use kou::{FontCache, FontSet, GraphicsProtocol, VttyManager, render_graphics};
let frame = render_graphics(&screen, &FontCache::load(&FontSet::from_env(), 16.0), 16.0,
                            GraphicsProtocol::from_env());
if let Some(escape) = frame {
    print!("{escape}"); // capable terminals render the pixels inline
}
```

## Polices et récupération

kou pré-télécharge une police par écriture dans un cache partagé à la compilation :

| Écriture | Police |
|----------|--------|
| Latin | [Fira Code](https://github.com/tonsky/FiraCode) |
| CJK (中文 · 日本語 · 한국어) | [Source Han Sans SC](https://github.com/adobe-fonts/source-han-sans) (思源黑体) |

Surchargez n'importe quelle famille à la compilation avec `KOU_FONT_PRIMARY` /
`KOU_FONT_CJK`, ou épinglez des fichiers locaux avec `KOU_FONT_PATH` /
`KOU_FONT_CJK_PATH`. Les téléchargements peuvent être acheminés via un proxy
HTTP(S) via `KOU_DOWNLOAD_PROXY` (passé directement à reqwest).

| Env | Rôle |
|-----|------|
| `KOU_FONT_PRIMARY` | Surcharge la famille de polices latines. |
| `KOU_FONT_CJK` | Surcharge / désactive la police CJK (`none` pour désactiver). |
| `KOU_FONT_MIRROR` | Remplace l'hôte de téléchargement par un miroir. |
| `KOU_DOWNLOAD_PROXY` | Achemine les téléchargements via un proxy HTTP(S) (reqwest). |
| `KOU_DOWNLOAD_TIMEOUT_SECS` | Délai d'expiration par requête (par défaut 120). |
| `KOU_SKIP_FONT_FETCH` | Désactive la récupération. |

## Serveur MCP

Construisez kou avec la feature `mcp` et lancez le serveur stdio — il expose le moteur de terminal virtuel aux assistants de codage IA via le Model Context Protocol (aucun navigateur ni démon requis) :

```bash
kou mcp
```

Le serveur annonce onze outils — `vtty_launch`, `vtty_kill`, `vtty_send_keys`, `vtty_send_text`, `vtty_screenshot`, `vtty_wait`, `vtty_ready`, `vtty_scrollback`, `vtty_resize`, `vtty_list`, `vtty_ping` — chacun déléguant en intra-processus au même `VttyManager` que la bibliothèque expose. Les captures d'écran sont rendues via la même pile de polices + thèmes que la bibliothèque, donc `vtty_screenshot` renvoie un vrai PNG (ou du texte thématisé) pour les modèles capables de vision.

Branchez-le dans un client MCP :

```json
{
  "mcpServers": {
    "kou": { "command": "kou", "args": ["mcp"] }
  }
}
```

Définissez `KOU_PROJECT_ROOT` pour fixer le répertoire de travail des sessions lancées lorsque le client n'annonce pas de racine de projet.

## Développement

```bash
cargo check --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Licence

SySL-1.0 (Synthetic Source License). Voir [LICENSE](https://sysl.celestia.world).

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

