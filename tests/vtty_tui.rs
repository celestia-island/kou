//! PTY-driven TUI snapshot tests.
//!
//! Unlike `tests/snapshots.rs` (which feeds static ANSI into a `Screen`),
//! these tests launch a **real** child process inside a PTY via
//! [`VttyManager`], drive it with keystrokes, and snapshot the rendered grid.
//! This is the pattern downstream TUI projects (scriptum, aoba) should follow
//! for their own screenshot-based CI.
//!
//! # Platform
//!
//! `#![cfg(unix)]` because the harness spawns real shell binaries (`echo`,
//! `printf`) and relies on a Unix PTY. On Windows the ConPTY path works, but
//! the specific commands differ — kept off Windows to avoid maintaining two
//! fixture scripts.
//!
//! # Font determinism
//!
//! Same contract as `tests/snapshots.rs`: pinned fonts via the kou cache,
//! `KOU_SKIP_FONT_FETCH=1`, `KOU_ACCEPT_SNAPSHOTS=1` to bless.

#![cfg(unix)]

mod common;

use common::assert_vtty_screenshot;
use kou::VttyManager;

/// Drive a trivial interactive program (`cat`) through the PTY and snapshot
/// the echoed output. This is the smoke test for the vtty→render pipeline
/// that scriptum's harness will build on top of.
#[tokio::test]
async fn cat_echo_snapshot() {
    let mgr = VttyManager::new();
    let info = mgr
        .launch(
            "cat",
            None,
            &[],
            80,
            24,
            Some("vtty_tui_cat"),
        )
        .await
        .expect("launch cat");

    // Type a line; cat echoes it back.
    mgr.send_text(&info.id, "hello from kou vtty harness")
        .await
        .expect("send_text");
    mgr.send_keys(&info.id, "ENTER").await.expect("send_keys ENTER");

    // Wait for the echo to land in the screen.
    kou_wait_for_text(&mgr, &info.id, "hello from kou", 5.0).await;

    assert_vtty_screenshot(&mgr, &info.id, "campbell", "vtty_cat_echo_campbell")
        .await
        .expect("screenshot matches baseline");

    mgr.kill(&info.id).await;
}

/// Drive a slightly richer session: a coloured banner built with a shell
/// one-liner, then snapshot under the Solarized Dark theme to exercise ANSI
/// colour rendering through the real PTY path (not just hand-fed ANSI).
#[tokio::test]
async fn shell_colour_snapshot() {
    let mgr = VttyManager::new();
    // Use `sh -c` so we can pass a full command line; the command prints a
    // coloured banner then exits, leaving its output in the screen.
    let banner = "printf '\\\\x1b[1;36mkou\\\\x1b[0m \\\\x1b[1;33mvtty\\\\x1b[0m \\\\x1b[2;37mcolour snapshot test\\\\x1b[0m\\\\n'";
    let info = mgr
        .launch(
            "sh",
            None,
            &[],
            80,
            24,
            Some("vtty_tui_shell"),
        )
        .await
        .expect("launch sh");
    // sh with no -c reads stdin; type the banner then exit.
    mgr.send_text(&info.id, banner).await.expect("send banner");
    mgr.send_keys(&info.id, "ENTER").await.expect("send ENTER");

    kou_wait_for_text(&mgr, &info.id, "colour snapshot test", 5.0).await;

    assert_vtty_screenshot(
        &mgr,
        &info.id,
        "solarized-dark",
        "vtty_shell_colour_solarized_dark",
    )
    .await
    .expect("screenshot matches baseline");

    mgr.kill(&info.id).await;
}

/// Poll [`VttyManager::find_text`] until `pattern` shows up or `timeout_secs`
/// elapses. Mirrors the wait helper used by the existing `tests/smoke.rs`.
async fn kou_wait_for_text(mgr: &VttyManager, id: &str, pattern: &str, timeout_secs: f64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
    loop {
        if let Ok(hits) = mgr.find_text(id, pattern).await {
            if !hits.is_empty() {
                return;
            }
        }
        if std::time::Instant::now() >= deadline {
            panic!("timed out waiting for text {pattern:?} in session {id}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
