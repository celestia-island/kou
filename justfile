# kou — virtual terminal automation.

set shell := ["bash", "-c"]
# Windows: PowerShell (the 5.1 floor ships with every Windows; pwsh 7 is
# NOT assumed). Linewise recipes must stay PS-5.1-safe: no `&&` chains,
# `cd X; cmd` instead of `cd X && cmd`. Bash-only recipes use
# [script('bash')] and need Git Bash (or WSL) when actually run.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command", "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $PSDefaultParameterValues['*:Encoding']='utf8';"]
# `set lists` enables which() (used by the imported celestia-devtools.just);
# `set unstable` gates it.
set unstable
set lists

# Repo definitions override the shared template's (imported above).
set allow-duplicate-recipes
set allow-duplicate-variables

# Shared celestia-devtools recipes — NOT in git. Stage with: just fetch.
# `import?` silently skips when absent, so this justfile parses pre-fetch.
import? "./.just/git-bash-interop.just"
import? "./.just/celestia-devtools.just"

# Stage shared celestia-devtools recipes into .just/ (gitignored).
# Source order: explicit URL arg → local pip bundle (offline) → GitHub raw.
# curl honors HTTP_PROXY/HTTPS_PROXY/ALL_PROXY env vars automatically.
fetch URL='':
    {{ if os_family() == "windows" { "python" } else { "python3" } }} -c "import os; os.makedirs('.just', exist_ok=True)"
    {{ if URL != "" { "curl -fsSL " + URL + " -o .just/celestia-devtools.just" } else if which("celestia-devtools") != "" { "celestia-devtools fetch-just" } else { "curl -fsSL https://raw.githubusercontent.com/celestia-island/celestia-devtools/dev/src/celestia_devtools/common.just -o .just/celestia-devtools.just" } }}
default:
    @just --list

# Format all sources.
fmt:
    just fmt-toml
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Type-check all targets and features.
[unix]
check:
    KOU_SKIP_FONT_FETCH=1 cargo check --all-targets --all-features

[windows]
check:
    $env:KOU_SKIP_FONT_FETCH='1'; cargo check --all-targets --all-features

# Clippy with -D warnings.
[unix]
clippy:
    KOU_SKIP_FONT_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings

[windows]
clippy:
    $env:KOU_SKIP_FONT_FETCH='1'; cargo clippy --all-targets --all-features -- -D warnings

# Run the test suite.
[unix]
test:
    KOU_SKIP_FONT_FETCH=1 cargo test --all-features

[windows]
test:
    $env:KOU_SKIP_FONT_FETCH='1'; cargo test --all-features

# Build all features.
[unix]
build:
    KOU_SKIP_FONT_FETCH=1 cargo build --all-features

[windows]
build:
    $env:KOU_SKIP_FONT_FETCH='1'; cargo build --all-features

# One-shot local gate: fmt-check + clippy + test.
ci:
    just fmt-check
    just clippy
    just test

# ── npx distribution (local dry-run) ─────────────────────────────────────────
#
# Wraps the shared recipe from celestia-devtools.just with kou's metadata. CI
# does the actual publish (see .github/workflows/npm-release.yml); locally this
# only stages ./dist and runs `npm pack --dry-run`.
#
#   just npm-dist-local                                       # reassemble root from existing dist/
#   just npm-dist-local 0.1.0 path/to/kou x86_64-pc-windows-msvc
[unix]
npm-dist-local version='' binary='' target='':
    KOU_SKIP_FONT_FETCH=1 just npm-dist kou {{version}} {{binary}} {{target}}

[windows]
npm-dist-local version='' binary='' target='':
    $env:KOU_SKIP_FONT_FETCH='1'; just npm-dist kou {{version}} {{binary}} {{target}}
