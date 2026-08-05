//! The QuotaPane mark, rasterized in code (M7b).
//!
//! Paints brand mark **1c "Two panes"** — the repo mark in `assets/` — into a
//! plain RGBA8 buffer at any size. Used for both the window/taskbar icon and
//! the system-tray icon, and for the tray it is a *live* miniature: the two
//! bars carry the providers' real usage, so the tray shows the same truth the
//! window does.
//!
//! ## No dependencies, by design
//! There is no image crate here, no SVG parser, and no build script — the mark
//! is described as geometry and sampled per pixel, in the same tradition as the
//! painted disclosure triangle and titlebar glyphs. An icon format is a parser,
//! and a parser in the dependency tree is exactly what this project spends its
//! budget avoiding. If a future mark seems to need a decoder, that is a design
//! question to hand back, not a crate to add.
//!
//! Colours come from the window's own palette consts, so the icon cannot drift
//! away from the UI, and bar fills go through the same [`crate::fraction_color`]
//! mapping as the on-screen bars.

/// Geometry is authored in the mark's own 64×64 viewBox and scaled to the
/// requested size, so every dimension below matches `assets/quotapane-1c.svg`
/// unit for unit.
const VIEW: f32 = 64.0;

/// Corner radius of the tile itself.
const TILE_RADIUS: f32 = 10.0;

/// The inner "window" outline: x, y, width, height, corner radius, stroke.
const WIN_X0: f32 = 8.0;
const WIN_Y0: f32 = 13.0;
const WIN_X1: f32 = 56.0;
const WIN_Y1: f32 = 51.0;
const WIN_RADIUS: f32 = 6.0;
const WIN_STROKE: f32 = 2.0;

/// The two titlebar dots.
const DOT_RADIUS: f32 = 1.9;
const DOT_Y: f32 = 21.0;
const DOT1_X: f32 = 15.0;
const DOT2_X: f32 = 21.5;

/// The two quota bars: top is Claude, bottom is Codex.
const BAR_X: f32 = 14.0;
const BAR_WIDTH: f32 = 36.0;
const BAR_HEIGHT: f32 = 6.0;
const BAR_RADIUS: f32 = 3.0;
const BAR1_Y: f32 = 29.0;
const BAR2_Y: f32 = 39.0;

/// Fully transparent — outside the tile.
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// Render the mark as RGBA8, row-major, `size * size * 4` bytes.
///
/// `claude` and `codex` are used fractions in `0.0..=1.0`. `None` means "not
/// known" and draws that bar as an empty trough — the icon never invents a
/// reading it does not have.
///
/// Pure and deterministic: same inputs, same bytes. That is what lets the tray
/// wiring skip the platform call when nothing changed.
pub fn render_icon(claude: Option<f32>, codex: Option<f32>, size: u32) -> Vec<u8> {
    render_icon_with_alert(claude, codex, size, false)
}

/// The mark, optionally in its **alert** variant: a 1-pixel CARDINAL ring
/// around the tile's edge (M13).
///
/// One pixel at the requested size, not one unit of the 64-unit viewBox, so the
/// ring reads as a hairline outline at 32px in the tray rather than as a thick
/// red border. Painted, like everything else here — no second asset, no
/// decoder, no dependency, and nothing to keep in sync with the base mark.
///
/// The ring is the *only* difference: with `alert` false this is byte-for-byte
/// the mark M7b accepted, which is what lets the tray revert simply by asking
/// for it again.
pub fn render_icon_with_alert(
    claude: Option<f32>,
    codex: Option<f32>,
    size: u32,
    alert: bool,
) -> Vec<u8> {
    let n = size as usize;
    let mut px = vec![0u8; n * n * 4];
    if n == 0 {
        return px;
    }

    let scale = VIEW / size as f32;
    // The ring's thickness in viewBox units: exactly one device pixel.
    let ring = if alert { scale } else { 0.0 };
    for y in 0..n {
        for x in 0..n {
            // Sample at the pixel centre, in the mark's 64-unit space.
            let u = (x as f32 + 0.5) * scale;
            let v = (y as f32 + 0.5) * scale;
            let color = sample(u, v, claude, codex, ring);
            let i = (y * n + x) * 4;
            px[i..i + 4].copy_from_slice(&color);
        }
    }
    px
}

/// Colour of the mark at one point, resolved topmost-layer-first.
fn sample(u: f32, v: f32, claude: Option<f32>, codex: Option<f32>, ring: f32) -> [u8; 4] {
    // Bars (topmost): trough always, fill up to the fraction.
    for (bar_y, fraction) in [(BAR1_Y, claude), (BAR2_Y, codex)] {
        if in_round_rect(
            u,
            v,
            BAR_X,
            bar_y,
            BAR_X + BAR_WIDTH,
            bar_y + BAR_HEIGHT,
            BAR_RADIUS,
        ) {
            if let Some(f) = fraction {
                let filled_to = BAR_X + BAR_WIDTH * f.clamp(0.0, 1.0);
                if u <= filled_to {
                    return fill_color(f);
                }
            }
            return crate::HAIRLINE.to_array();
        }
    }

    // The two dots.
    for (dot_x, color) in [(DOT1_X, crate::CARDINAL), (DOT2_X, crate::PINE)] {
        let (dx, dy) = (u - dot_x, v - DOT_Y);
        if dx * dx + dy * dy <= DOT_RADIUS * DOT_RADIUS {
            return color.to_array();
        }
    }

    // The window outline: inside the outer rounded rect but not inside the one
    // inset by the stroke width.
    let outer = in_round_rect(u, v, WIN_X0, WIN_Y0, WIN_X1, WIN_Y1, WIN_RADIUS);
    let inner = in_round_rect(
        u,
        v,
        WIN_X0 + WIN_STROKE,
        WIN_Y0 + WIN_STROKE,
        WIN_X1 - WIN_STROKE,
        WIN_Y1 - WIN_STROKE,
        (WIN_RADIUS - WIN_STROKE).max(0.0),
    );
    if outer && !inner {
        return crate::TEXT_DIM.to_array();
    }

    // The tile: GROUND with rounded corners, transparent outside them. Corner
    // pixels are simply dropped rather than antialiased — at 32px and below the
    // difference is invisible, and it keeps this a pure geometry test.
    if in_round_rect(u, v, 0.0, 0.0, VIEW, VIEW, TILE_RADIUS) {
        // The alert ring rides the tile's own edge, so it follows the rounded
        // corners and never squares them off. Checked after the mark's contents
        // (bars, dots, outline), all of which sit well inside it, so the ring
        // can only ever replace tile background.
        if ring > 0.0
            && !in_round_rect(
                u,
                v,
                ring,
                ring,
                VIEW - ring,
                VIEW - ring,
                (TILE_RADIUS - ring).max(0.0),
            )
        {
            return crate::CARDINAL.to_array();
        }
        return crate::GROUND.to_array();
    }

    CLEAR
}

/// Bar fill colour for a fraction, through the window's own severity mapping so
/// a glance at the tray means the same thing as a glance at the window.
fn fill_color(fraction: f32) -> [u8; 4] {
    crate::fraction_color(Some(fraction as f64)).to_array()
}

/// Whether a point lies inside an axis-aligned rounded rectangle.
fn in_round_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if u < x0 || u > x1 || v < y0 || v > y1 {
        return false;
    }
    // Clamp to the rectangle's "straight" core; the leftover offset is the
    // distance into a corner, which must fall within the radius.
    let cx = u.clamp(x0 + r, x1 - r);
    let cy = v.clamp(y0 + r, y1 - r);
    let (dx, dy) = (u - cx, v - cy);
    dx * dx + dy * dy <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample at the mark's native scale so coordinates map 1:1 to the SVG.
    const N: u32 = 64;

    fn pixel(px: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    /// A point well inside the top (Claude) bar, just past its left edge — so
    /// it is filled for any fraction above ~0.06.
    const BAR_PROBE: (u32, u32) = (16, 32);
    /// The same for the bottom (Codex) bar.
    const BAR2_PROBE: (u32, u32) = (16, 42);

    #[test]
    fn buffer_is_rgba_of_the_requested_size() {
        for size in [16u32, 32, 64, 256] {
            let px = render_icon(Some(0.5), Some(0.5), size);
            assert_eq!(px.len(), (size * size * 4) as usize, "size {size}");
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        let a = render_icon(Some(0.31), None, N);
        let b = render_icon(Some(0.31), None, N);
        assert_eq!(a, b);
    }

    #[test]
    fn bar_fill_follows_the_severity_mapping() {
        // The same thresholds the window's bars use: pine / amber / cardinal.
        for (fraction, expected, name) in [
            (0.2f32, crate::PINE, "pine"),
            (0.6, crate::AMBER, "amber"),
            (0.9, crate::CARDINAL, "cardinal"),
        ] {
            let px = render_icon(Some(fraction), None, N);
            assert_eq!(
                pixel(&px, N, BAR_PROBE.0, BAR_PROBE.1),
                expected.to_array(),
                "{fraction} should paint {name}"
            );
        }
    }

    #[test]
    fn unknown_fraction_leaves_the_trough() {
        let px = render_icon(None, None, N);
        assert_eq!(
            pixel(&px, N, BAR_PROBE.0, BAR_PROBE.1),
            crate::HAIRLINE.to_array()
        );
        assert_eq!(
            pixel(&px, N, BAR2_PROBE.0, BAR2_PROBE.1),
            crate::HAIRLINE.to_array()
        );
    }

    #[test]
    fn empty_and_full_render_differently() {
        assert_ne!(
            render_icon(Some(0.0), Some(0.0), N),
            render_icon(Some(1.0), Some(1.0), N)
        );
    }

    #[test]
    fn zero_fraction_paints_no_fill() {
        // 0.0 is a known reading, but nothing is used yet, so the probe well
        // inside the bar stays trough — distinct from `None` only in intent.
        let px = render_icon(Some(0.0), None, N);
        assert_eq!(
            pixel(&px, N, BAR_PROBE.0, BAR_PROBE.1),
            crate::HAIRLINE.to_array()
        );
    }

    #[test]
    fn the_two_bars_are_independent() {
        // Top bar is Claude, bottom is Codex — a swap would show up here.
        let px = render_icon(Some(0.9), Some(0.2), N);
        assert_eq!(
            pixel(&px, N, BAR_PROBE.0, BAR_PROBE.1),
            crate::CARDINAL.to_array(),
            "top bar should carry Claude"
        );
        assert_eq!(
            pixel(&px, N, BAR2_PROBE.0, BAR2_PROBE.1),
            crate::PINE.to_array(),
            "bottom bar should carry Codex"
        );
    }

    #[test]
    fn corners_are_transparent_and_the_centre_is_opaque() {
        let px = render_icon(None, None, N);
        assert_eq!(pixel(&px, N, 0, 0), CLEAR, "tile corner should be clipped");
        assert_eq!(pixel(&px, N, N - 1, N - 1), CLEAR);
        // Dead centre sits between the bars, on the tile.
        assert_eq!(pixel(&px, N, 32, 24)[3], 255);
    }

    #[test]
    fn the_status_dots_are_painted() {
        let px = render_icon(None, None, N);
        assert_eq!(
            pixel(&px, N, DOT1_X as u32, DOT_Y as u32),
            crate::CARDINAL.to_array()
        );
        assert_eq!(
            pixel(&px, N, DOT2_X as u32, DOT_Y as u32),
            crate::PINE.to_array()
        );
    }

    #[test]
    fn a_zero_size_request_does_not_panic() {
        assert!(render_icon(Some(0.5), Some(0.5), 0).is_empty());
    }
}
