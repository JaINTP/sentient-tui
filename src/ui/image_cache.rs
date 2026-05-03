/// Image downloading, disk caching, and terminal rendering protocol management.
///
/// All ArtifactsMMO image assets follow the pattern:
///   `https://artifactsmmo.com/images/{category}/{code}.png`
///
/// Categories: characters, items, monsters, maps, resources, effects, npcs, badges
///
/// Download events are routed to the TUI footer log via a `SystemLog` action sent
/// over the app's action channel (set with `ImageCache::set_log_tx`).
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use directories::BaseDirs;
use image::DynamicImage;
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

use crate::core::action::Action;

pub type SharedImageCache = Arc<Mutex<ImageCache>>;

pub const BASE_URL: &str = "https://artifactsmmo.com/images";
pub const PLAY_BASE_URL: &str = "https://play.artifactsmmo.com/images";

/// Maximum number of simultaneous background downloads.
const MAX_CONCURRENT: usize = 32;

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
    /// Keys that permanently failed (404 / decode error) — never retried.
    failed: HashSet<String>,
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
            total_queued: 0,
            total_completed: 0,
            log_tx: None,
            sync_base_url: std::env::var("BOT_SYNC_API_URL").ok(),
        }))
    }

    /// Wire up the TUI action channel so download events appear in the footer log.
    /// Call this once in `App::run()` after the channel is set up.
    pub fn set_log_tx(cache: &SharedImageCache, tx: UnboundedSender<Action>) {
        cache.lock().unwrap().log_tx = Some(tx);
    }

    /// Return `Some(image)` if the asset is already cached (memory or disk).
    /// Otherwise, schedule a background download and return `None` until it arrives.
    ///
    /// Safe to call every frame — duplicate requests are suppressed via `pending`.
    /// Safe to call every frame — duplicate requests are suppressed via `pending`.
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

            // Already failed permanently — don't retry
            if c.failed.contains(&key) {
                return None;
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
                        {
                            let mut c = cache_clone.lock().unwrap();
                            c.pending.remove(&key);
                            c.failed.insert(key.clone());
                            c.total_completed += 1;
                        }
                        Self::send_log(&log_tx, "[IMG✗]", format!("{key} failed: {e}"));
                        warn!("image_cache: download failed {key}: {e}");
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
    /// loaded from disk, or permanently failed.  Used to gate the loading screen.
    pub fn is_settled(cache: &SharedImageCache, category: &str, code: &str) -> bool {
        if code.is_empty() {
            return true;
        }
        let key = format!("{category}/{code}");
        let c = cache.lock().unwrap();
        c.images.contains_key(&key) || c.failed.contains(&key)
    }

    /// Returns (total_completed, total_queued)
    pub fn get_stats(cache: &SharedImageCache) -> (usize, usize) {
        let c = cache.lock().unwrap();
        (c.total_completed, c.total_queued)
    }

    // ── Internals ─────────────────────────────────────────────────────────

    fn send_log(tx: &Option<UnboundedSender<Action>>, tag: &str, message: String) {
        if let Some(tx) = tx {
            let _ = tx.send(Action::SystemLog {
                tag: tag.to_string(),
                message,
            });
        }
    }

    async fn download(
        http: &reqwest::Client,
        url: &str,
        disk_path: &std::path::Path,
    ) -> Result<DynamicImage, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(disk_path, &bytes)?;

        let img = image::load_from_memory(&bytes)?;
        Ok(img)
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
