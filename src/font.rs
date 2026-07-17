//! Font discovery & loading — build-time font fetching.
//!
//! kou does not ship fonts. Instead, it fetches one font per script into a
//! shared cache (once, then cached across runs) and locates it transparently,
//! so a consumer never has to install fonts by hand. Sources are public
//! (GitHub / jsDelivr CDN); `KOU_FONT_MIRROR` and `KOU_DOWNLOAD_PROXY` route
//! the fetch through a mirror or forward proxy, which is essential behind
//! restrictive networks.
//!
//! A [`FontSet`] pairs a primary monospace face (Latin) with an ordered list
//! of fallback faces (CJK by default). [`FontCache::load`] reads them into
//! `ab_glyph` font vectors and [`FontCache::glyph_for`] picks the first face
//! that actually contains a given codepoint — so a single render can mix
//! Latin and CJK without tofu.
//!
//! Resolution order:
//! 1. Explicit file paths — `KOU_FONT_PATH` (primary) and `KOU_FONT_*_PATH`.
//! 2. A previously-fetched copy in the shared cache.
//! 3. A runtime download (the `font-fetch` feature) from the registry.
//!
//! Knobs: `KOU_FONT_MIRROR`, `KOU_DOWNLOAD_PROXY`, `KOU_DOWNLOAD_TIMEOUT_SECS`,
//! `KOU_SKIP_FONT_FETCH`, `KOU_FONT_PRIMARY`, `KOU_FONT_CJK`.
//! Proxy detection also checks `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY`
//! as fallbacks when `KOU_DOWNLOAD_PROXY` is not set.

use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontVec, Glyph, PxScaleFont, ScaleFont};

/// A curated font family kou knows how to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    /// Fira Code — ligature-rich Latin monospace.
    FiraCode,
    /// JetBrains Mono.
    JetBrainsMono,
    /// Source Han Sans SC (思源黑体) — CJK sans serif.
    SourceHanSansSC,
    /// Sarasa Mono SC (更纱黑体) — CJK monospace, terminal-friendly.
    SarasaMonoSC,
    /// Smiley Sans (得意黑) — CJK display face.
    SmileySans,
}

impl FontFamily {
    /// The family selected for the primary (Latin) slot, or the default.
    pub fn primary_from_env() -> Self {
        match std::env::var("KOU_FONT_PRIMARY")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("jetbrains") | Some("jetbrainsmono") => FontFamily::JetBrainsMono,
            _ => FontFamily::FiraCode,
        }
    }

    /// The family selected for the CJK fallback slot; `None` disables CJK.
    pub fn cjk_from_env() -> Option<Self> {
        match std::env::var("KOU_FONT_CJK")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("sourcehansans") | Some("sourcehan") => Some(FontFamily::SourceHanSansSC),
            Some("sarasa") | Some("sarasa-mono") => Some(FontFamily::SarasaMonoSC),
            Some("smileysans") | Some("smiley") => Some(FontFamily::SmileySans),
            Some("none") | Some("off") => None,
            _ => Some(FontFamily::SourceHanSansSC),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FontFamily::FiraCode => "fira-code",
            FontFamily::JetBrainsMono => "jetbrains-mono",
            FontFamily::SourceHanSansSC => "source-han-sans-sc",
            FontFamily::SarasaMonoSC => "sarasa-mono-sc",
            FontFamily::SmileySans => "smiley-sans",
        }
    }

    /// Filename inside the cache.
    fn cache_name(self) -> &'static str {
        match self {
            FontFamily::FiraCode => "FiraCode-Regular.ttf",
            FontFamily::JetBrainsMono => "JetBrainsMono-Regular.ttf",
            FontFamily::SourceHanSansSC => "SourceHanSansSC-Regular.otf",
            FontFamily::SarasaMonoSC => "SarasaMonoSC-Regular.ttf",
            FontFamily::SmileySans => "SmileySans-Regular.ttf",
        }
    }

    /// Which `KOU_FONT_*_PATH` env var overrides this family's file location.
    fn path_env(self) -> Option<&'static str> {
        match self {
            FontFamily::FiraCode | FontFamily::JetBrainsMono => Some("KOU_FONT_PATH"),
            FontFamily::SourceHanSansSC | FontFamily::SarasaMonoSC | FontFamily::SmileySans => {
                Some("KOU_FONT_CJK_PATH")
            }
        }
    }

    /// Source URL. `KOU_FONT_MIRROR` substitutes the canonical GitHub/jsDelivr
    /// host so a mirror or GFW-friendly host can serve the bytes instead.
    #[cfg(feature = "font-fetch")]
    fn source_url(self) -> String {
        let raw = match self {
            FontFamily::FiraCode => {
                "https://cdn.jsdelivr.net/gh/tonsky/FiraCode@5.2/distr/ttf/FiraCode-Regular.ttf"
            }
            FontFamily::JetBrainsMono => {
                "https://cdn.jsdelivr.net/gh/JetBrains/JetBrainsMono@2.304/fonts/ttf/JetBrainsMono-Regular.ttf"
            }
            FontFamily::SourceHanSansSC => {
                "https://cdn.jsdelivr.net/gh/adobe-fonts/source-han-sans@release/SubsetOTF/CN/SourceHanSansCN-Regular.otf"
            }
            FontFamily::SarasaMonoSC => {
                "https://cdn.jsdelivr.net/gh/be5invis/sarasa-gothic@0.42.5/SarasaMonoSC-Regular.ttf"
            }
            FontFamily::SmileySans => {
                "https://cdn.jsdelivr.net/gh/atelier-anchor/smiley-sans@2.0.1/fonts/SmileySans-Regular.ttf"
            }
        };
        if let Ok(mirror) = std::env::var("KOU_FONT_MIRROR") {
            let mirror = mirror.trim_end_matches('/');
            if !mirror.is_empty() {
                return raw
                    .replace("https://cdn.jsdelivr.net/gh", mirror)
                    .replace("https://github.com", mirror);
            }
        }
        raw.to_string()
    }
}

/// A primary face plus an ordered list of per-script fallbacks.
#[derive(Debug, Clone)]
pub struct FontSet {
    pub primary: FontFamily,
    pub fallbacks: Vec<FontFamily>,
}

impl FontSet {
    /// The default set, honouring `KOU_FONT_PRIMARY` / `KOU_FONT_CJK` /
    /// `KOU_FONT_ARABIC` / `KOU_FONT_DEVANAGARI` / `KOU_FONT_THAI`.
    pub fn from_env() -> Self {
        let mut fallbacks = Vec::new();
        if let Some(cjk) = FontFamily::cjk_from_env() {
            fallbacks.push(cjk);
        }
        FontSet {
            primary: FontFamily::primary_from_env(),
            fallbacks,
        }
    }

    /// Iterate primary then fallbacks.
    fn all(&self) -> impl Iterator<Item = FontFamily> + '_ {
        std::iter::once(self.primary).chain(self.fallbacks.iter().copied())
    }
}

/// Shared cache root: `<cache>/kou/fonts`.
pub fn cache_root() -> PathBuf {
    cache_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("kou-cache"))
        .join("kou")
        .join("fonts")
}

fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        None
    }
}

/// Resolve a single family to an existing font file, fetching if necessary.
///
/// Returns `None` when the `font-fetch` feature is disabled and no cached copy
/// or explicit path exists, so callers can fall back to a built-in font.
pub fn resolve_family(family: FontFamily) -> Option<PathBuf> {
    // 1. Explicit override via the per-family path env var.
    if let Some(env) = family.path_env() {
        if let Ok(p) = std::env::var(env) {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // 2. Cached copy.
    let cached = cache_root().join(family.cache_name());
    if cached.exists() {
        return Some(cached);
    }

    // 3. Runtime fetch.
    #[cfg(feature = "font-fetch")]
    {
        if std::env::var_os("KOU_SKIP_FONT_FETCH").is_none() {
            match download(family, &cached) {
                Ok(()) => return Some(cached),
                Err(e) => {
                    eprintln!("[kou] font fetch for {} failed: {e}", family.label());
                }
            }
        }
    }

    None
}

/// Detect HTTP proxy from environment variables.
///
/// Checks `KOU_DOWNLOAD_PROXY`, `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` in
/// order, returning the first non-empty value.
#[cfg(feature = "font-fetch")]
fn detect_proxy() -> Option<String> {
    for var in &[
        "KOU_DOWNLOAD_PROXY",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
    ] {
        if let Ok(val) = std::env::var(var) {
            let val = val.trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

#[cfg(feature = "font-fetch")]
fn download(family: FontFamily, dest: &Path) -> anyhow::Result<()> {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        eprintln!("[kou] TLS provider install failed (font fetch may not work): {e:?}");
    }
    let timeout = std::env::var("KOU_DOWNLOAD_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &u64| n > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(120));
    let mut builder = reqwest::blocking::Client::builder().timeout(timeout);
    if let Some(proxy) = detect_proxy() {
        eprintln!("[kou] using font download proxy {proxy}");
        builder = builder.proxy(
            reqwest::Proxy::all(&proxy)
                .map_err(|e| anyhow::anyhow!("invalid proxy {proxy:?}: {e}"))?,
        );
    }
    let client = builder.build()?;
    let url = family.source_url();
    eprintln!("[kou] downloading {} (once, then cached)", url);
    let bytes = client
        .get(&url)
        .header("User-Agent", "kou")
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Write to a temp file first, then rename, so a partial download never
    // poisons the cache.
    let tmp = dest.with_extension("download");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// In-memory, rasterisation-ready font faces.
pub struct FontCache {
    /// Primary first, then CJK fallback. Ordered so [`glyph_for`] picks Latin
    /// before CJK when both cover a codepoint.
    faces: Vec<PxScaleFont<FontVec>>,
}

impl FontCache {
    /// Load a [`FontSet`] from disk into `ab_glyph`. Faces that fail to parse
    /// are skipped (with a warning), so a missing CJK face degrades gracefully.
    pub fn load(set: &FontSet, px: f32) -> Self {
        let mut faces = Vec::new();
        for family in set.all() {
            if let Some(path) = resolve_family(family) {
                push_face(&mut faces, &path, px);
            }
        }
        FontCache { faces }
    }

    /// Build a cache from explicit file paths (no fetching).
    pub fn from_paths(paths: &[&Path], px: f32) -> Self {
        let mut faces = Vec::new();
        for p in paths {
            push_face(&mut faces, p, px);
        }
        FontCache { faces }
    }

    /// Number of usable faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// `true` if no face loaded.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// An empty cache (no faces) — the renderer degrades to solid blocks.
    pub fn empty() -> Self {
        FontCache { faces: Vec::new() }
    }

    /// Pick the first face that defines a glyph for `ch`, returning a scaled
    /// glyph ready to outline. `ab_glyph` reports a missing glyph as
    /// `GlyphId(0)`, which we treat as "not in this face".
    pub fn glyph_for(&self, ch: char) -> Option<Glyph> {
        for face in &self.faces {
            let glyph_id = face.glyph_id(ch);
            if glyph_id.0 != 0 {
                return Some(face.scaled_glyph(ch));
            }
        }
        None
    }

    /// Rasterise `ch` into a per-pixel coverage callback, using the first face
    /// that covers the codepoint (Latin before CJK). `cell_x`/`cell_y` is the
    /// cell's top-left in pixels; the ascent offset is applied internally. The
    /// callback receives **absolute** pixel coordinates (already offset by the
    /// glyph's positioned bounding box), so the caller must NOT add the cell
    /// origin again. Returns `true` if any face rendered the glyph.
    pub fn draw_char<F: FnMut(u32, u32, f32)>(
        &self,
        ch: char,
        cell_x: f32,
        cell_y: f32,
        mut put: F,
    ) -> bool {
        for face in &self.faces {
            if face.glyph_id(ch).0 == 0 {
                continue;
            }
            let mut glyph = face.scaled_glyph(ch);
            glyph.position = ab_glyph::point(cell_x, cell_y + face.ascent());
            let Some(outlined) = face.outline_glyph(glyph) else {
                continue;
            };
            // `OutlinedGlyph::draw` yields coords relative to the glyph's
            // bounding-box min; offset them back to absolute image space.
            let bounds = outlined.px_bounds();
            let min_x = bounds.min.x.floor();
            let min_y = bounds.min.y.floor();
            outlined.draw(|gx, gy, cov| {
                let x = min_x + gx as f32;
                let y = min_y + gy as f32;
                if x >= 0.0 && y >= 0.0 {
                    put(x as u32, y as u32, cov);
                }
            });
            return true;
        }
        false
    }

    /// Borrow the primary (first) face — used for metrics like line height.
    pub fn primary(&self) -> Option<&PxScaleFont<FontVec>> {
        self.faces.first()
    }
}

fn push_face(out: &mut Vec<PxScaleFont<FontVec>>, path: &Path, px: f32) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[kou] cannot read font {}: {e}", path.display());
            return;
        }
    };
    match FontVec::try_from_vec(bytes) {
        // `into_scaled` consumes the font and yields an owned
        // `PxScaleFont<'static, FontVec>` — no leaking needed.
        Ok(font) => out.push(font.into_scaled(px)),
        Err(e) => {
            eprintln!("[kou] font {} failed to parse: {e}", path.display());
        }
    }
}

// ── system font discovery ─────────────────────────────

/// Paths that may contain system-installed fonts (checked in order).
fn font_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // XDG / traditional Linux paths.
    dirs.push(PathBuf::from("/usr/share/fonts"));
    dirs.push(PathBuf::from("/usr/local/share/fonts"));
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home.as_str()).join(".local/share/fonts"));
        dirs.push(PathBuf::from(home.as_str()).join(".fonts"));
    }
    // macOS.
    if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
    }
    // Windows.
    if cfg!(target_os = "windows") {
        if let Ok(windir) = std::env::var("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
    }
    dirs
}

/// Candidate monospace filenames — tried in each directory until one exists.
const MONO_CANDIDATES: &[&str] = &[
    "DejaVuSansMono.ttf",
    "LiberationMono-Regular.ttf",
    "LiberationMono-R.ttf",
    "NotoSansMono-Regular.ttf",
    "FiraCode-Regular.ttf",
    "FiraMono-Regular.ttf",
    "JetBrainsMono-Regular.ttf",
    "JetBrainsMonoNL-Regular.ttf",
    "Hack-Regular.ttf",
    "SourceCodePro-Regular.ttf",
    "UbuntuMono-R.ttf",
    "UbuntuMono-Regular.ttf",
    "Courier_Prime.ttf",
    "consola.ttf",
    "cour.ttf",
    "lucon.ttf",
    "Menlo.ttc",
];

/// Candidate CJK fallback filenames.
const CJK_CANDIDATES: &[&str] = &[
    "NotoSansCJK-Regular.ttc",
    "NotoSansCJK-Medium.ttc",
    "NotoSansCJK-Light.ttc",
    "NotoSansCJK-Bold.ttc",
    "NotoSansSC-Regular.otf",
    "NotoSansSC-Regular.ttf",
    "NotoSansTC-Regular.otf",
    "NotoSansJP-Regular.otf",
    "SourceHanSansCN-Regular.otf",
    "SourceHanSansSC-Regular.otf",
    "SarasaMonoSC-Regular.ttf",
    "SmileySans-Regular.ttf",
    "WQYMicroHei.ttc",
    "DroidSansFallbackFull.ttf",
    "DroidSansFallback.ttf",
    "msyh.ttc",
    "SimHei.ttf",
];

/// Find the first existing path for `candidate` by recursively walking
/// `search_dirs` (up to 3 levels deep). System fonts are often nested in
/// subdirectories like `/usr/share/fonts/opentype/noto/` or
/// `/usr/share/fonts/truetype/dejavu/`, so a flat `dir.join(name)` check
/// misses them.
fn find_in_dirs(candidates: &[&str], dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        if let Some(p) = find_recursive(candidates, dir, 3) {
            return Some(p);
        }
    }
    None
}

fn find_recursive(candidates: &[&str], dir: &Path, depth: u32) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(p) = find_recursive(candidates, &path, depth - 1) {
                return Some(p);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if candidates.contains(&name) {
                return Some(path);
            }
        }
    }
    None
}

/// Discover a local monospace face + an optional CJK fallback from the operating
/// system's font directories (no network). Suitable as a zero-config fallback
/// when `KOU_FONT_PATH` isn't set and no kou cache exists. Pair with
/// [`FontCache::from_paths`].
pub fn locate_system_fonts() -> Vec<PathBuf> {
    let dirs = font_search_dirs();
    let mut out = Vec::new();
    if let Some(path) = find_in_dirs(MONO_CANDIDATES, &dirs) {
        out.push(path);
    }
    if let Some(path) = find_in_dirs(CJK_CANDIDATES, &dirs) {
        out.push(path);
    }
    out
}

// ── async font fetch (for tokio runtimes) ─────────────

#[cfg(feature = "font-fetch")]
async fn download_async(family: FontFamily, dest: &Path) -> anyhow::Result<()> {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        eprintln!("[kou] TLS provider install failed (font fetch may not work): {e:?}");
    }
    let timeout = std::env::var("KOU_DOWNLOAD_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &u64| n > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(120));
    let mut builder = reqwest::Client::builder().timeout(timeout);
    if let Some(proxy) = detect_proxy() {
        eprintln!("[kou] using font download proxy {proxy}");
        builder = builder.proxy(
            reqwest::Proxy::all(&proxy)
                .map_err(|e| anyhow::anyhow!("invalid proxy {proxy:?}: {e}"))?,
        );
    }
    let client = builder.build()?;
    let url = family.source_url();
    eprintln!("[kou] downloading {} (once, then cached)", url);
    let bytes = client
        .get(&url)
        .header("User-Agent", "kou")
        .send()
        .await
        .and_then(|r| r.error_for_status())?
        .bytes()
        .await?;
    let dest = dest.to_path_buf();
    let parent = dest.parent().map(|p| p.to_path_buf());
    // Offload the sync file I/O so we don't stall the async runtime.
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if let Some(p) = &parent {
            std::fs::create_dir_all(p)?;
        }
        let tmp = dest.with_extension("download");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &dest)?;
        Ok(())
    })
    .await??;
    Ok(())
}

#[cfg(feature = "font-fetch")]
async fn resolve_family_async(family: FontFamily) -> Option<PathBuf> {
    // 1. Explicit path override via the per-family path env var.
    if let Some(env) = family.path_env() {
        if let Ok(p) = std::env::var(env) {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // 2. Previously cached.
    let cached = cache_root().join(family.cache_name());
    if cached.exists() {
        return Some(cached);
    }

    // 3. Async runtime fetch.
    if std::env::var_os("KOU_SKIP_FONT_FETCH").is_none() {
        match download_async(family, &cached).await {
            Ok(()) => return Some(cached),
            Err(e) => {
                eprintln!("[kou] font fetch for {} failed: {e}", family.label());
            }
        }
    }
    None
}

impl FontCache {
    /// Async variant of [`load`](FontCache::load) — uses `reqwest`'s non-blocking
    /// client so it is safe to call inside a Tokio runtime. Falls back through
    /// explicit paths, the kou cache, and an HTTP download just like `load`.
    #[cfg(feature = "font-fetch")]
    pub async fn load_async(set: &FontSet, px: f32) -> Self {
        let mut faces = Vec::new();
        for family in set.all() {
            if let Some(path) = resolve_family_async(family).await {
                push_face(&mut faces, &path, px);
            }
        }
        FontCache { faces }
    }

    /// Build from system-installed fonts discovered under standard OS font
    /// directories (monospace + optional CJK). Zero-config, zero-network.
    /// Pair with [`locate_system_fonts`].
    pub fn from_system_fonts(px: f32) -> Self {
        let paths = locate_system_fonts();
        let refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        FontCache::from_paths(&refs, px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_name_is_unique_per_family() {
        let names: Vec<&str> = [
            FontFamily::FiraCode,
            FontFamily::JetBrainsMono,
            FontFamily::SourceHanSansSC,
            FontFamily::SarasaMonoSC,
            FontFamily::SmileySans,
        ]
        .iter()
        .map(|f| f.cache_name())
        .collect();
        let dedup: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), dedup.len(), "duplicate cache names: {names:?}");
    }

    #[cfg(feature = "font-fetch")]
    #[test]
    #[serial_test::serial]
    fn mirror_substitution_replaces_host() {
        let restore = std::env::var_os("KOU_FONT_MIRROR");
        // SAFETY: serialized via the `serial` attribute — no concurrent reader.
        unsafe { std::env::set_var("KOU_FONT_MIRROR", "https://mirror.example.com") };
        let url = FontFamily::FiraCode.source_url();
        assert!(
            url.starts_with("https://mirror.example.com/"),
            "mirror not applied: {url}"
        );
        unsafe {
            match restore {
                Some(v) => std::env::set_var("KOU_FONT_MIRROR", v),
                None => std::env::remove_var("KOU_FONT_MIRROR"),
            }
        }
    }
}
