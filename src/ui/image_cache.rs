//! Image downloading, disk caching, and terminal rendering protocol management.
//!
//! All ArtifactsMMO image assets follow the URL pattern:
//! `https://artifactsmmo.com/images/{category}/{code}.png`
//!
//! Known categories: `characters`, `items`, `monsters`, `maps`, `resources`,
//! `effects`, `npcs`, `badges`.
//!
//! The module provides two complementary types:
//!
//! - [`ImageCache`] — shared (`Arc<Mutex<…>>`) store that handles HTTP
//!   downloads, on-disk caching, and in-memory promotion.  Images are fetched
//!   lazily on first request and a concurrent-download semaphore caps
//!   outstanding HTTP requests at [`MAX_CONCURRENT`].
//!
//! - [`ProtocolCache`] — per-component, non-`Send` store of
//!   [`StatefulProtocol`] objects (terminal-specific pixel rendering handles).
//!   Sits alongside each component that renders images; pairs with
//!   [`ImageCache`] to convert a decoded [`image::DynamicImage`] into
//!   renderable form.
//!
//! Download events are forwarded to the TUI footer log via
//! [`Action::SystemLog`] when a log sender has been wired up with
//! [`ImageCache::set_log_tx`].

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use directories::BaseDirs;
use image::DynamicImage;
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

use crate::core::action::Action;

/// Reference-counted, mutex-wrapped [`ImageCache`].
///
/// All public [`ImageCache`] methods accept `&SharedImageCache` rather than
/// `&mut self` so callers can clone and share the handle freely across
/// component boundaries without holding the lock across await points.
pub type SharedImageCache = Arc<Mutex<ImageCache>>;

/// Default CDN base URL for ArtifactsMMO image assets.
pub const BASE_URL: &str = "https://artifactsmmo.com/images";

/// Alternate CDN base URL served by the web client (used for some skin assets).
pub const PLAY_BASE_URL: &str = "https://play.artifactsmmo.com/images";

/// Maximum number of simultaneous background HTTP downloads.
///
/// Controlled by the internal [`tokio::sync::Semaphore`]; requests beyond this
/// cap are not dropped — they queue inside the spawned task until a permit
/// becomes available.
const MAX_CONCURRENT: usize = 32;

/// How long to wait before retrying a transiently-failed download (e.g. timeout, 5xx).
/// 404 responses are never retried.
const TRANSIENT_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Error returned by the internal [`ImageCache::download`] helper.
enum DownloadError {
    /// Server returned 404 — the asset does not exist; never retry.
    NotFound,
    /// Any other failure (network error, timeout, 5xx, decode error).
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::NotFound => write!(f, "not found (404)"),
            DownloadError::Other(e) => write!(f, "{e}"),
        }
    }
}

// ── ImageCache ────────────────────────────────────────────────────────────────

/// Thread-safe image store.  Wrap in `Arc<Mutex<…>>` via `new_shared()`.
pub struct ImageCache {
    /// Root of the on-disk cache tree.  Images land at `{cache_dir}/{category}/{code}.png`.
    cache_dir: PathBuf,
    /// Shared HTTP client (internally connection-pooled, cheap to clone).
    http: reqwest::Client,
    /// In-memory decoded images, keyed by `"{category}/{code}"`.
    images: HashMap<String, Arc<DynamicImage>>,
    /// Keys currently being fetched (prevents duplicate concurrent requests).
    pending: HashSet<String>,
    /// Semaphore limiting concurrent HTTP requests without dropping them.
    semaphore: Arc<tokio::sync::Semaphore>,
    /// Keys that permanently failed (404) — never retried.
    failed: HashSet<String>,
    /// Keys that failed transiently (timeout, 5xx, I/O) with the time of failure.
    /// Retried after [`TRANSIENT_RETRY_AFTER`].
    transient_failed: HashMap<String, Instant>,
    /// Total items that have been queued for download
    total_queued: usize,
    /// Total items downloaded or failed
    total_completed: usize,
    /// Optional channel for routing download events to the TUI footer log.
    log_tx: Option<UnboundedSender<Action>>,
    /// Base URL override for images, derived from bot_sync_api_url
    sync_base_url: Option<String>,
}

impl ImageCache {
    /// Create a new shared instance.  Call once at startup and clone the Arc.
    pub fn new_shared() -> SharedImageCache {
        let cache_dir = BaseDirs::new()
            .map(|bd| {
                bd.cache_dir()
                    .join("sentient-tui")
                    .join("images")
            })
            .unwrap_or_else(|| PathBuf::from(".cache/images"));

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("sentient-tui/0.1")
            .build()
            .unwrap_or_default();

        Arc::new(Mutex::new(ImageCache {
            cache_dir,
            http,
            images: HashMap::new(),
            pending: HashSet::new(),
            semaphore: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT)),
            failed: HashSet::new(),
            transient_failed: HashMap::new(),
            total_queued: 0,
            total_completed: 0,
            log_tx: None,
            sync_base_url: std::env::var("BOT_SYNC_API_URL")
                .ok()
                .map(|url| format!("{}/images", url.trim_end_matches('/'))),
        }))
    }

    /// Wire up the TUI action channel so download events appear in the footer log.
    /// Call this once in `App::run()` after the channel is set up.
    pub fn set_log_tx(cache: &SharedImageCache, tx: UnboundedSender<Action>) {
        cache.lock().unwrap().log_tx = Some(tx);
    }

    /// Return the image for `(category, code)` if available; otherwise schedule a download.
    ///
    /// Resolution order:
    /// 1. **Memory** — returns immediately if the image was previously decoded.
    /// 2. **Disk** — loads from `{cache_dir}/{category}/{code}.png` and promotes
    ///    to memory on a cache hit.
    /// 3. **Network** — spawns a background Tokio task to download the image from
    ///    the configured base URL, write it to disk, and insert it into memory.
    ///    Returns `None` on this first call; subsequent calls return `Some` once
    ///    the download task completes.
    ///
    /// Safe to call every frame — duplicate in-flight requests are suppressed via
    /// the `pending` set; permanently failed keys are never retried.
    pub fn get_or_fetch(
        cache: &SharedImageCache,
        category: &str,
        code: &str,
    ) -> Option<Arc<DynamicImage>> {
        let base_url = {
            let c = cache.lock().unwrap();
            c.sync_base_url.clone().unwrap_or_else(|| BASE_URL.to_string())
        };
        Self::get_or_fetch_from(cache, &base_url, category, code)
    }

    /// Like `get_or_fetch` but uses a custom base URL (e.g. `PLAY_BASE_URL`).
    pub fn get_or_fetch_from(
        cache: &SharedImageCache,
        base_url: &str,
        category: &str,
        code: &str,
    ) -> Option<Arc<DynamicImage>> {
        if code.is_empty() {
            return None;
        }
        let key = format!("{category}/{code}");

        // ── Phase 1: check memory / disk (while holding lock) ────────────
        let (disk_path, http_clone, log_tx, semaphore, should_spawn) = {
            let mut c = cache.lock().unwrap();

            // Memory hit
            if let Some(img) = c.images.get(&key) {
                return Some(img.clone());
            }

            // Disk hit — load and promote to memory
            let disk_path = c
                .cache_dir
                .join(category)
                .join(format!("{code}.png"));
            if disk_path.exists() {
                match image::open(&disk_path) {
                    Ok(img) => {
                        let arc = Arc::new(img);
                        c.images
                            .insert(key.clone(), arc.clone());
                        // Log disk-cache hit
                        Self::send_log(
                            &c.log_tx,
                            "[IMG◈]",
                            format!("{category}/{code} loaded from disk"),
                        );
                        return Some(arc);
                    }
                    Err(e) => {
                        warn!("image_cache: open disk failed {}: {e}", disk_path.display());
                    }
                }
            }

            // Already failed permanently (404) — don't retry
            if c.failed.contains(&key) {
                return None;
            }

            // Transient failure still within cooldown — wait
            if let Some(&fail_time) = c.transient_failed.get(&key) {
                if fail_time.elapsed() < TRANSIENT_RETRY_AFTER {
                    return None;
                }
                // Cooldown expired — clear and re-queue
                c.transient_failed.remove(&key);
            }

            // Already fetching
            if c.pending.contains(&key) {
                return None;
            }

            c.pending.insert(key.clone());
            c.total_queued += 1;
            // Log that we're starting a download
            Self::send_log(&c.log_tx, "[IMG↓]", format!("{category}/{code} downloading…"));
            (disk_path, c.http.clone(), c.log_tx.clone(), Arc::clone(&c.semaphore), true)
        };

        // ── Phase 2: spawn download (lock dropped) ────────────────────────
        if should_spawn {
            // Strip any trailing slash from base_url, or handle if base_url includes /api/v1
            let base = base_url.trim_end_matches('/');
            let url = format!("{base}/{category}/{code}.png");
            let cache_clone = Arc::clone(cache);
            tokio::spawn(async move {
                let _permit = semaphore.acquire().await;
                match Self::download(&http_clone, &url, &disk_path).await {
                    Ok(img) => {
                        let size_kb = img.width() * img.height() * 4 / 1024;
                        {
                            let mut c = cache_clone.lock().unwrap();
                            c.images
                                .insert(key.clone(), Arc::new(img));
                            c.pending.remove(&key);
                            c.total_completed += 1;
                        }
                        Self::send_log(
                            &log_tx,
                            "[IMG✓]",
                            format!(
                                "{key} cached ({size_kb}KB, {}×{})",
                                {
                                    // Re-read dims from the now-cached image for the log
                                    let c = cache_clone.lock().unwrap();
                                    c.images
                                        .get(&key)
                                        .map(|i| i.width())
                                        .unwrap_or(0)
                                },
                                {
                                    let c = cache_clone.lock().unwrap();
                                    c.images
                                        .get(&key)
                                        .map(|i| i.height())
                                        .unwrap_or(0)
                                }
                            ),
                        );
                        debug!("image_cache: cached {key}");
                    }
                    Err(e) => {
                        let is_permanent = matches!(e, DownloadError::NotFound);
                        {
                            let mut c = cache_clone.lock().unwrap();
                            c.pending.remove(&key);
                            if is_permanent {
                                c.failed.insert(key.clone());
                            } else {
                                c.transient_failed.insert(key.clone(), Instant::now());
                            }
                            c.total_completed += 1;
                        }
                        let msg = if is_permanent {
                            format!("{key} not found (404)")
                        } else {
                            format!("{key} failed (retry in {TRANSIENT_RETRY_AFTER:?}): {e}")
                        };
                        Self::send_log(&log_tx, "[IMG✗]", msg.clone());
                        warn!("image_cache: {msg}");
                    }
                }
            });
        }
        None
    }

    /// Fire-and-forget prefetch.  Call during startup / when new data arrives
    /// so images are ready when the UI needs them.
    pub fn prefetch(cache: &SharedImageCache, category: &str, code: &str) {
        let _ = Self::get_or_fetch(cache, category, code);
    }

    /// Check whether a key is already in memory (no disk/network fallback).
    pub fn is_ready(cache: &SharedImageCache, category: &str, code: &str) -> bool {
        if code.is_empty() {
            return false;
        }
        let key = format!("{category}/{code}");
        cache
            .lock()
            .unwrap()
            .images
            .contains_key(&key)
    }

    /// True if the asset no longer needs to be waited on: it is either in memory,
    /// loaded from disk, permanently failed (404), or transiently failed (will retry
    /// later).  Used to gate the loading screen.
    pub fn is_settled(cache: &SharedImageCache, category: &str, code: &str) -> bool {
        if code.is_empty() {
            return true;
        }
        let key = format!("{category}/{code}");
        let c = cache.lock().unwrap();
        c.images.contains_key(&key)
            || c.failed.contains(&key)
            || c.transient_failed.contains_key(&key)
    }

    /// Return cumulative download progress as `(completed, queued)`.
    ///
    /// `completed` counts both successful downloads and permanent failures.
    /// `queued` counts every asset that has ever been requested via
    /// [`get_or_fetch`][Self::get_or_fetch] or [`prefetch`][Self::prefetch].
    /// The loading screen uses this ratio to drive its progress bar.
    pub fn get_stats(cache: &SharedImageCache) -> (usize, usize) {
        let c = cache.lock().unwrap();
        (c.total_completed, c.total_queued)
    }

    // ── Internals ─────────────────────────────────────────────────────────

    /// Send a [`Action::SystemLog`] message over the optional action channel.
    ///
    /// Silently discards the message when no sender has been registered.
    fn send_log(tx: &Option<UnboundedSender<Action>>, tag: &str, message: String) {
        if let Some(tx) = tx {
            let _ = tx.send(Action::SystemLog {
                tag: tag.to_string(),
                message,
            });
        }
    }

    /// Download `url`, write raw bytes to `disk_path`, and decode the image.
    ///
    /// Parent directories of `disk_path` are created if they do not exist.
    ///
    /// Returns [`DownloadError::NotFound`] for 404 responses (permanent failure)
    /// and [`DownloadError::Other`] for everything else (transient, may retry).
    async fn download(
        http: &reqwest::Client,
        url: &str,
        disk_path: &std::path::Path,
    ) -> Result<DynamicImage, DownloadError> {
        let response = http
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadError::Other(e.into()))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(DownloadError::NotFound);
        }

        let bytes = response
            .error_for_status()
            .map_err(|e| DownloadError::Other(e.into()))?
            .bytes()
            .await
            .map_err(|e| DownloadError::Other(e.into()))?;

        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DownloadError::Other(e.into()))?;
        }
        std::fs::write(disk_path, &bytes).map_err(|e| DownloadError::Other(e.into()))?;

        image::load_from_memory(&bytes).map_err(|e| DownloadError::Other(e.into()))
    }
}

// ── ProtocolCache ─────────────────────────────────────────────────────────────

/// Per-component cache of `StatefulProtocol` objects.
///
/// `StatefulProtocol` is NOT `Send`, so this lives *inside* the component and
/// is never shared across threads.  It pairs with `SharedImageCache`: when
/// `ImageCache::get_or_fetch` returns `Some(image)`, call `ensure()` to build
/// the terminal-specific rendering protocol, then `render()` to draw it.
pub struct ProtocolCache {
    picker: Picker,
    /// Keyed by the same `"{category}/{code}"` string used in `ImageCache`.
    protocols: HashMap<String, StatefulProtocol>,
}

impl Default for ProtocolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolCache {
    /// Create a new, empty [`ProtocolCache`].
    ///
    /// Probes the terminal for its pixel-rendering capability via
    /// `Picker::from_query_stdio`; falls back to Unicode half-block characters
    /// if the capability cannot be determined (e.g. when stdout is not a TTY).
    pub fn new() -> Self {
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        Self {
            picker,
            protocols: HashMap::new(),
        }
    }

    /// Build a rendering protocol for `key` if one doesn't exist yet.
    pub fn ensure(&mut self, key: &str, img: &DynamicImage) {
        if !self.protocols.contains_key(key) {
            let proto = self
                .picker
                .new_resize_protocol(img.clone());
            self.protocols
                .insert(key.to_string(), proto);
        }
    }

    /// Drop the cached protocol for `key` (call when the underlying image changes).
    pub fn invalidate(&mut self, key: &str) {
        self.protocols.remove(key);
    }

    /// Render the image for `key` into `frame` at `area`.
    /// Does nothing if no protocol exists (image still loading or key unknown).
    pub fn render(&mut self, key: &str, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if let Some(proto) = self.protocols.get_mut(key) {
            frame.render_stateful_widget(StatefulImage::<StatefulProtocol>::default(), area, proto);
        }
    }

    /// True if a protocol exists for `key` (image is ready to render).
    pub fn has(&self, key: &str) -> bool {
        self.protocols.contains_key(key)
    }
}
