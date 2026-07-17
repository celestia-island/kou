//! Shared test helpers for kou's screenshot-based regression tests.
//!
//! This module factors the snapshot primitives out of `tests/snapshots.rs` so
//! that other test binaries (and downstream crates using kou as a dev-dep) can
//! reuse the same pinned-font + pixel-diff workflow without duplicating the
//! logic.
//!
//! # Font determinism
//!
//! Every helper here uses [`fonts()`], which loads a **fixed** `FontSet`
//! (FiraCode primary + Source Han Sans SC fallback) — never system fonts.
//! Cross-machine drift would otherwise cause spurious diffs. Tests run with
//! `KOU_SKIP_FONT_FETCH=1`; the cache must be pre-populated:
//!   - locally: run any kou example once, or set `KOU_FONT_PATH` /
//!     `KOU_FONT_CJK_PATH`;
//!   - in CI: the workflow downloads a pinned `fonts` Release into the kou
//!     cache before `cargo test`.
//!
//! # Accepting a new baseline
//!
//! ```bash
//! KOU_ACCEPT_SNAPSHOTS=1 cargo test --test snapshots <name>
//! ```
//! then commit the updated `res/*.png`.

use image::RgbaImage;
use kou::{FontCache, FontSet, Screen, VttyManager, theme_by_name};

/// Maximum fraction of pixels allowed to differ before a snapshot is flagged
/// as a regression. 0.1% tolerates PNG re-encoding noise while still catching
/// any meaningful glyph/layout change.
pub const DIFF_THRESHOLD: f64 = 0.001;

/// Per-channel tolerance below which a pixel is not counted as differing.
/// Absorbs ±2 anti-aliasing jitter.
const PIXEL_JITTER_TOLERANCE: u8 = 2;

/// Load the **fixed** font set used by every snapshot. See the module docs.
pub fn fonts() -> FontCache {
    FontCache::load(&FontSet::from_env(), 32.0 * 3.0)
}

/// Render `screen` in `theme` and compare against the baseline `res/{name}.png`.
///
/// - With `KOU_ACCEPT_SNAPSHOTS` set, the rendered image replaces the baseline
///   (use this to bless an intentional rendering change).
/// - Otherwise the two are diffed pixel-by-pixel; differing beyond
///   [`DIFF_THRESHOLD`] fails the test with guidance on how to accept.
pub fn assert_matches(screen: &Screen, theme: &str, name: &str) {
    let png = kou::render::render_png_supersampled(screen, &fonts(), 32.0, 3, theme_by_name(theme))
        .expect("render");
    compare_against_baseline(&png, name);
}

/// Compare an already-rendered PNG (bytes) against `res/{name}.png`.
///
/// Factored out of [`assert_matches`] so that vtty-driven tests — which obtain
/// their PNG via `VttyManager::screen` + `render_png_supersampled` rather than
/// a hand-fed `Screen` — can share the exact same diff/bless path.
pub fn compare_against_baseline(png_bytes: &[u8], name: &str) {
    let baseline = std::path::Path::new("res").join(format!("{name}.png"));

    if std::env::var_os("KOU_ACCEPT_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(baseline.parent().unwrap()).unwrap();
        std::fs::write(&baseline, png_bytes).unwrap();
        eprintln!("  accepted {baseline:?}");
        return;
    }

    let new = image::load_from_memory(png_bytes)
        .expect("decode rendered png")
        .to_rgba8();
    let base_bytes = std::fs::read(&baseline).unwrap_or_else(|e| {
        panic!(
            "baseline {baseline:?} missing ({e}); generate it with \
             `KOU_ACCEPT_SNAPSHOTS=1 cargo test --test snapshots {name}`"
        )
    });
    let base = image::load_from_memory(&base_bytes)
        .expect("decode baseline png")
        .to_rgba8();

    assert_eq!(
        base.dimensions(),
        new.dimensions(),
        "{name}: dimensions changed ({}x{} -> {}x{}) — screen geometry or font metrics drifted",
        base.width(),
        base.height(),
        new.width(),
        new.height(),
    );

    let (ratio, max_delta) = pixel_diff(&base, &new);
    let pct = format!("{:.3}%", ratio * 100.0);
    assert!(
        ratio < DIFF_THRESHOLD,
        "{name}: {pct} of pixels differ (max channel delta {max_delta}) — \
         rendering regression? If this change is intended, bless it with \
         `KOU_ACCEPT_SNAPSHOTS=1 cargo test --test snapshots {name}`",
    );
}

/// Drive a real TUI running inside a [`VttyManager`] session and snapshot its
/// rendered PNG against `res/{name}.png`.
///
/// This is the end-to-end counterpart to [`assert_matches`]: instead of
/// hand-feeding ANSI into a `Screen`, it pulls the live grid out of the
/// managed PTY session and renders that. Useful for verifying that a real
/// TUI program (scriptum, aoba, etc.) actually paints what we expect.
///
/// `#[allow(dead_code)]` because the only caller (`tests/vtty_tui.rs`) is
/// `#![cfg(unix)]` — on Windows this compiles but is unreferenced.
#[allow(dead_code)]
pub async fn assert_vtty_screenshot(
    mgr: &VttyManager,
    session_id: &str,
    theme: &str,
    name: &str,
) -> anyhow::Result<()> {
    let screen = mgr.screen(session_id).await?;
    let png = kou::render::render_png_supersampled(&screen, &fonts(), 32.0, 3, theme_by_name(theme))?;
    compare_against_baseline(&png, name);
    Ok(())
}

/// Fraction of differing pixels and the largest single-channel delta between
/// two equally-sized RGBA images. A pixel "differs" when any channel differs
/// by more than [`PIXEL_JITTER_TOLERANCE`] (anti-aliasing edges can shift).
pub fn pixel_diff(base: &RgbaImage, other: &RgbaImage) -> (f64, u8) {
    debug_assert_eq!(base.dimensions(), other.dimensions());
    let (w, h) = base.dimensions();
    let total = (w as u64) * (h as u64);
    let mut differing = 0u64;
    let mut max_delta = 0u8;
    for (a, b) in base.pixels().zip(other.pixels()) {
        let dr = a[0].abs_diff(b[0]);
        let dg = a[1].abs_diff(b[1]);
        let db = a[2].abs_diff(b[2]);
        let da = a[3].abs_diff(b[3]);
        let local = dr.max(dg).max(db).max(da);
        max_delta = max_delta.max(local);
        if local > PIXEL_JITTER_TOLERANCE {
            differing += 1;
        }
    }
    (differing as f64 / total as f64, max_delta)
}
