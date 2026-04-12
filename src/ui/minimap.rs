/// Minimap rendering as a 3×3 Layout grid of tile sprites.
///
/// Each of the 9 visible positions owns an independent `StatefulProtocol` slot, so
/// ratatui-image renders each tile directly into its own sub-`Rect`.  The grid cells
/// are produced by equal-weight Layout constraints, which means the map **always fills
/// the full widget width** regardless of terminal image protocol or aspect ratio.
///
/// Sprite images are fetched from:
///   `https://artifactsmmo.com/images/maps/{skin}.png`
/// via the shared `ImageCache` (background download + disk cache).  While a download
/// is in progress, a solid-colour fallback block is shown instead.
///
/// The centre cell (character's current tile) always has a "◉" marker overlaid.
use std::collections::HashMap;

use image::imageops::FilterType;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};

use crate::{
    core::game::MapTile,
    ui::image_cache::{ImageCache, SharedImageCache},
};

/// Half-width of the visible square — gives a (2R+1)² grid window.
/// R=1 → 3×3  R=2 → 5×5
const RADIUS: i32 = 1;

// ── Per-position slot ─────────────────────────────────────────────────────────

/// Holds the current skin name and its associated terminal rendering protocol.
///
/// The skin is tracked so we rebuild if the tile visible at this grid position changes
/// (e.g. after the character moves).
struct Slot {
    skin: String,
    /// The cell area (in terminal columns/rows) when this slot was last built.
    /// If the area changes (e.g. terminal resize) we need to rebuild so the
    /// pre-resized image matches the new pixel dimensions.
    cell: Rect,
    protocol: StatefulProtocol,
}

// ── Public cache ──────────────────────────────────────────────────────────────

/// Minimap renderer.  One instance lives in `Sidebar`; keep it across frames.
pub struct MinimapCache {
    picker: Picker,
    /// Keyed by `(dx, dy)` offset from the character in `−RADIUS..=RADIUS`.
    /// Each slot has an independent protocol so the same skin can appear in
    /// multiple cells without state aliasing.
    slots: HashMap<(i32, i32), Slot>,
    /// Cached rendering protocol for the character portrait overlay.
    /// Tuple is `(skin_code, rendered_rect, protocol)` — rebuilt when either changes.
    char_slot: Option<(String, Rect, StatefulProtocol)>,
}

impl Default for MinimapCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimapCache {
    pub fn new() -> Self {
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        Self {
            picker,
            slots: HashMap::with_capacity(9),
            char_slot: None,
        }
    }

    /// The terminal's detected font size in pixels `(width, height)` per character cell.
    /// Used by the sidebar to compute a pixel-square minimap area.
    pub fn font_size(&self) -> (u16, u16) {
        self.picker.font_size()
    }

    /// Draw the 3×3 tile grid centred on `(cx, cy)` into `area`.
    ///
    /// Each tile is rendered as:
    ///  1. The real sprite image if it is already in `image_cache` (or fetched in
    ///     the background — appears on a later frame once downloaded), or
    ///  2. A solid-colour block derived from `tile.content_type`.
    ///
    /// The centre tile always has a "◉" overlay.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        cx: i32,
        cy: i32,
        layer: &str,
        char_skin: &str,
        tiles: &HashMap<(i32, i32, String), MapTile>,
        image_cache: Option<&SharedImageCache>,
        frame: &mut Frame,
        area: Rect,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let dim = (RADIUS * 2 + 1) as u32; // 3

        // Three equal rows.
        let row_areas = Layout::vertical(vec![Constraint::Fill(1); dim as usize]).split(area);

        let mut center_cell = Rect::default();

        for (row_idx, dy) in (-RADIUS..=RADIUS).enumerate() {
            // Three equal columns inside each row.
            let col_areas = Layout::horizontal(vec![Constraint::Fill(1); dim as usize])
                .split(row_areas[row_idx]);

            for (col_idx, dx) in (-RADIUS..=RADIUS).enumerate() {
                let cell = col_areas[col_idx];
                let is_center = dx == 0 && dy == 0;
                if is_center {
                    center_cell = cell;
                }

                // Sprite rendering: use the image when available.
                let sprite_ok =
                    self.draw_sprite(cx, cy, dx, dy, layer, tiles, image_cache, frame, cell);

                // Fallback block (also handles the center ◉ marker regardless of path).
                if !sprite_ok {
                    let tile = tiles.get(&(cx + dx, cy + dy, layer.to_string()));
                    draw_tile_block(tile, is_center, frame, cell);
                } else if is_center {
                    // Overlay ◉ on top of the sprite too.
                    overlay_center_marker(frame, cell);
                }
            }
        }

        // Overlay the character portrait on the centre tile.
        if !char_skin.is_empty()
            && center_cell.width > 0
            && let Some(cache) = image_cache
            && let Some(img) = ImageCache::get_or_fetch(cache, "characters", char_skin)
        {
            self.render_char_overlay(char_skin, &img, center_cell, frame);
        }
    }

    // ── Internals ─────────────────────────────────────────────────────────────

    /// Attempt to render the sprite for the tile at grid offset `(dx, dy)` from `(cx, cy)`.
    /// Returns `true` if a sprite was actually drawn.
    #[allow(clippy::too_many_arguments)]
    fn draw_sprite(
        &mut self,
        cx: i32,
        cy: i32,
        dx: i32,
        dy: i32,
        layer: &str,
        tiles: &HashMap<(i32, i32, String), MapTile>,
        image_cache: Option<&SharedImageCache>,
        frame: &mut Frame,
        area: Rect,
    ) -> bool {
        let Some(cache) = image_cache else {
            return false;
        };
        let Some(tile) = tiles.get(&(cx + dx, cy + dy, layer.to_string())) else {
            return false;
        };
        if tile.skin.is_empty() {
            return false;
        }

        // Returns Some if in memory/disk cache; fires background download otherwise.
        let Some(img) = ImageCache::get_or_fetch(cache, "maps", &tile.skin) else {
            return false;
        };

        let key = (dx, dy);
        let skin = tile.skin.as_str();

        // Rebuild when the skin or rendered cell area changes.
        let needs_rebuild = self
            .slots
            .get(&key)
            .is_none_or(|s| s.skin != skin || s.cell != area);

        if needs_rebuild {
            // Pre-resize the sprite to exactly fill the cell area in pixel space.
            // This avoids letterboxing: without this, ratatui-image proportionally
            // fits the image (maintaining aspect ratio), leaving dark bars where
            // terminal character height > width (the usual ~2:1 ratio).
            let (fw, fh) = self.picker.font_size();
            let fw = fw.max(1) as u32;
            let fh = fh.max(1) as u32;
            let pwidth = (area.width as u32 * fw).max(1);
            let pheight = (area.height as u32 * fh).max(1);
            let filled = img.resize_exact(pwidth, pheight, FilterType::Nearest);

            let proto = self.picker.new_resize_protocol(filled);
            self.slots.insert(
                key,
                Slot {
                    skin: skin.to_string(),
                    cell: area,
                    protocol: proto,
                },
            );
        }

        if let Some(slot) = self.slots.get_mut(&key) {
            frame.render_stateful_widget(
                // Image is already pixel-exact; Fit simply confirms it fits without
                // further modification.
                StatefulImage::<StatefulProtocol>::default(),
                area,
                &mut slot.protocol,
            );
            true
        } else {
            false
        }
    }

    /// Overlay the character portrait on the centre tile.
    ///
    /// The portrait is half the cell width, centred horizontally, with its
    /// bottom edge sitting 1/5 of the cell height from the bottom of the tile.
    fn render_char_overlay(
        &mut self,
        skin: &str,
        img: &image::DynamicImage,
        cell: Rect,
        frame: &mut Frame,
    ) {
        if cell.width == 0 || cell.height == 0 {
            return;
        }

        let sprite_w = (cell.width / 2).max(1);
        let sprite_h = (cell.height / 2).max(1);
        let sprite_x = cell.x + (cell.width - sprite_w) / 2;
        // Bottom of sprite is 1/5 of the cell height from the tile bottom.
        let bottom = cell.y + cell.height * 4 / 5;
        let sprite_y = bottom.saturating_sub(sprite_h);
        let overlay = Rect::new(sprite_x, sprite_y, sprite_w, sprite_h);

        let needs_rebuild = self
            .char_slot
            .as_ref()
            .is_none_or(|(s, r, _)| s != skin || *r != overlay);

        if needs_rebuild {
            let (fw, fh) = self.picker.font_size();
            let pw = (overlay.width as u32 * fw.max(1) as u32).max(1);
            let ph = (overlay.height as u32 * fh.max(1) as u32).max(1);
            let resized = img.resize_exact(pw, ph, FilterType::Nearest);
            let proto = self.picker.new_resize_protocol(resized);
            self.char_slot = Some((skin.to_string(), overlay, proto));
        }

        if let Some((_, _, proto)) = &mut self.char_slot {
            frame.render_stateful_widget(
                StatefulImage::<StatefulProtocol>::default(),
                overlay,
                proto,
            );
        }
    }
}

// ── Tile fallback rendering ───────────────────────────────────────────────────

/// Draw a rich fallback block for a tile that has no downloadable sprite.
///
/// Shows:
///  - Background fill keyed on `content_type`
///  - Content-type icon centered in the cell
///  - Abbreviated `content_code` on the row below the icon
///  - ◉ marker if `is_center` (character's current position)
fn draw_tile_block(tile: Option<&MapTile>, is_center: bool, frame: &mut Frame, cell: Rect) {
    if cell.width == 0 || cell.height == 0 {
        return;
    }

    let (bg, icon, code_str): (Color, &str, &str) = match tile {
        None => (Color::Rgb(10, 10, 20), "", ""),
        Some(t) => (tile_color(t), content_icon(&t.content_type), t.content_code.as_str()),
    };

    // 1. Background fill.
    frame.render_widget(Block::default().style(Style::default().bg(bg)), cell);

    // 2. Content icon + code (only if area is tall enough to show something).
    if !icon.is_empty() && cell.height >= 2 {
        let mid_y = cell.y + cell.height / 2;

        // Icon row.
        let icon_area = Rect::new(cell.x, mid_y.saturating_sub(1), cell.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                icon,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            icon_area,
        );

        // Abbreviated code row.
        if !code_str.is_empty() && cell.height >= 3 {
            let label: String = if code_str.len() as u16 > cell.width {
                code_str
                    .chars()
                    .take(cell.width as usize)
                    .collect()
            } else {
                code_str.to_owned()
            };
            let code_area = Rect::new(cell.x, mid_y, cell.width, 1);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    label,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::DIM),
                )))
                .alignment(Alignment::Center),
                code_area,
            );
        }
    }

    // 3. Center-position ◉ marker on top.
    if is_center {
        overlay_center_marker(frame, cell);
    }
}

/// Overlay the character position marker in the centre of a cell.
fn overlay_center_marker(frame: &mut Frame, cell: Rect) {
    if cell.width == 0 || cell.height == 0 {
        return;
    }
    let mx = cell.x + cell.width / 2;
    let my = cell.y + cell.height / 2;
    frame.render_widget(
        Paragraph::new(Span::styled(
            "◉",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Rect::new(mx, my, 1, 1),
    );
}

// ── Tile colour / icon helpers ────────────────────────────────────────────────

fn tile_color(tile: &MapTile) -> Color {
    match tile.content_type.as_str() {
        "monster" => Color::Rgb(110, 25, 25),        // dark crimson
        "resource" => Color::Rgb(20, 80, 35),        // forest green
        "bank" => Color::Rgb(150, 120, 15),          // dark gold
        "workshop" => Color::Rgb(100, 55, 20),       // burnt orange
        "tasks_master" => Color::Rgb(65, 25, 130),   // deep purple
        "grand_exchange" => Color::Rgb(20, 90, 140), // ocean blue
        "cooking" => Color::Rgb(130, 70, 20),        // warm amber
        _ => Color::Rgb(35, 40, 55),                 // slate (open terrain)
    }
}

/// Single-character icon for a tile content type.
fn content_icon(content_type: &str) -> &'static str {
    match content_type {
        "monster" => "☠",
        "resource" => "◈",
        "bank" => "$",
        "workshop" => "⚙",
        "tasks_master" => "!",
        "grand_exchange" => "≡",
        "cooking" => "~",
        _ => "",
    }
}
