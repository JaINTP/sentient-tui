//! Loading screen component — splash screen with asset download progress.
//!
//! Shows an animated banner and progress gauge while images are being downloaded and cached.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};

use crate::ui::components::Component;
use crate::ui::image_cache::{ImageCache, SharedImageCache};

/// ASCII-art banner rendered at the top of the loading screen.
const BANNER: &[&str] = &[
    r#"  ██████ ▓█████  ███▄    █ ▄▄▄█████▓ ██▓ ▓█████  ███▄    █ ▄▄▄█████▓"#,
    r#"▒██    ▒ ▓█   ▀  ██ ▀█   █ ▓  ██▒ ▓▒▓██▒ ▓█   ▀  ██ ▀█   █ ▓  ██▒ ▓▒"#,
    r#"░ ▓██▄   ▒███   ▓██  ▀█ ██▒▒ ▓██░ ▒░▒██▒ ▒███   ▓██  ▀█ ██▒▒ ▓██░ ▒░"#,
    r#"  ▒   ██▒▒▓█  ▄ ▓██▒  ▐▌██▒░ ▓██▓ ░ ░██░ ▒▓█  ▄ ▓██▒  ▐▌██▒░ ▓██▓ ░ "#,
    r#"▒██████▒▒░▒████▒▒██░   ▓██░  ▒██▒ ░ ░██░ ░▒████▒▒██░   ▓██░  ▒██▒ ░ "#,
    r#"▒ ▒▓▒ ▒ ░░░ ▒░ ░░ ▒░   ▒ ▒   ▒ ░░   ░▓   ░░ ▒░ ░░ ▒░   ▒ ▒   ▒ ░░   "#,
    r#"░ ░▒  ░ ░ ░ ░  ░░ ░░   ░ ▒░    ░     ▒ ░  ░ ░  ░░ ░░   ░ ▒░    ░     "#,
    r#"░  ░  ░     ░      ░   ░ ░   ░       ▒ ░    ░      ░   ░ ░   ░       "#,
    r#"      ░     ░  ░         ░           ░      ░  ░         ░           "#,
    r#"                                                                    "#,
    r#"                   ▄▄▄█████▓ █    ██  ██▓               "#,
    r#"                   ▓  ██▒ ▓▒ ██  ▓██▒▓██▒               "#,
    r#"                   ▒ ▓██░ ▒░▓██  ▒██░▒██▒               "#,
    r#"                   ░ ▓██▓ ░ ▓▓█  ░██░░██░               "#,
    r#"                     ▒██▒ ░ ▒▒█████▓ ░██░               "#,
    r#"                     ▒ ░░   ░▒▓▒ ▒ ▒ ░▓                 "#,
    r#"                       ░    ░░▒░ ░ ░  ▒ ░               "#,
    r#"                     ░       ░░░ ░ ░  ▒ ░               "#,
    r#"                               ░      ░                 "#,
];

/// Braille spinner frames — cycled at one frame per three render ticks.
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];

/// Loading screen component shown while assets download.
///
/// Displays a title banner, animated spinner, and progress gauge indicating
/// how many images have been downloaded vs. queued for download.
pub struct LoadingScreen {
    /// Shared image cache to check download progress.
    image_cache: SharedImageCache,
    /// Frame counter for spinner animation.
    tick: usize,
}

impl LoadingScreen {
    /// Create a new loading screen component.
    pub fn new(image_cache: SharedImageCache) -> Self {
        Self {
            image_cache,
            tick: 0,
        }
    }
}

impl Component for LoadingScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        self.tick = self.tick.wrapping_add(1);

        let (completed, total) = ImageCache::get_stats(&self.image_cache);

        // While nothing has been queued yet show an indeterminate 0%.
        let ratio = if total == 0 {
            0.0_f64
        } else {
            (completed as f64 / total as f64).min(1.0)
        };
        let percent = (ratio * 100.0) as u16;

        // ── Vertical split: filler / banner / version / progress bar / filler ──────
        let banner_h = BANNER.len() as u16;
        let [
            _,
            banner_area,
            version_area,
            progress_area,
            _,
        ] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(banner_h),
            Constraint::Length(1),
            Constraint::Length(5), // 2 padding + 3 gauge
            Constraint::Fill(1),
        ])
        .areas(area);

        // ── Banner ────────────────────────────────────────────────────────
        let lines: Vec<Line> = BANNER
            .iter()
            .map(|s| {
                Line::from(Span::styled(
                    *s,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), banner_area);

        // ── Version ───────────────────────────────────────────────────────
        let version = env!("CARGO_PKG_VERSION");
        let version_line = Line::from(vec![
            Span::styled("v", Style::default().fg(Color::DarkGray)),
            Span::styled(
                version,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        frame
            .render_widget(Paragraph::new(version_line).alignment(Alignment::Center), version_area);

        // ── Progress gauge ────────────────────────────────────────────────
        // Centre a 60-col gauge for readability on wide terminals.
        let gauge_w = area.width.clamp(20, 60);
        let [_, gauge_col, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(gauge_w),
            Constraint::Fill(1),
        ])
        .areas(progress_area);

        let [_, gauge_row, _] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(gauge_col);

        let spinner = SPINNER[(self.tick / 3) % SPINNER.len()];
        let title = if total == 0 {
            format!(" {spinner} Waiting for asset list… ")
        } else {
            format!(" {spinner} Sprites  {completed}/{total} ")
        };

        let ratio_f64 = ratio;
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Rgb(20, 20, 30)),
            )
            .ratio(ratio_f64);

        frame.render_widget(gauge, gauge_row);

        // Label the percent underneath the gauge.
        if gauge_row.y + 3 < area.height {
            let pct_area = Rect::new(gauge_row.x, gauge_row.y + 3, gauge_row.width, 1);
            let pct_line = Line::from(Span::styled(
                format!("{percent}%"),
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(Paragraph::new(pct_line).alignment(Alignment::Center), pct_area);
        }

        Ok(())
    }
}
