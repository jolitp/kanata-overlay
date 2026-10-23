//! Where the overlay goes on a screen, and how far it may scale.

use crate::{Align, Edge, Rect, Size};

/// The overlay's window geometry on one screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Top-left corner (used by the fallback window; layer-shell uses anchors).
    pub x: i32,
    pub y: i32,
    pub size: Size,
    /// Effective scale factor (1.0 = 100%).
    pub scale: f64,
    /// The configured scale did not fit and was reduced.
    pub capped: bool,
}

/// Put `base × scale` centred along `edge` of `screen` (or in a corner),
/// `margin` away from each edge it touches.
///
/// The scale is reduced to the largest value that fits `screen − 2·margin` on
/// both axes; the result always lies inside the screen.
pub fn place(screen: Rect, base: Size, edge: Edge, margin: u32, scale_percent: u16) -> Placement {
    let wanted = f64::from(scale_percent) / 100.0;
    let m = f64::from(margin);
    let avail_w = (f64::from(screen.width) - 2.0 * m).max(1.0);
    let avail_h = (f64::from(screen.height) - 2.0 * m).max(1.0);
    let fit = if base.width > 0 && base.height > 0 {
        (avail_w / f64::from(base.width)).min(avail_h / f64::from(base.height))
    } else {
        wanted
    };
    let (scale, capped) = if wanted > fit {
        (fit, true)
    } else {
        (wanted, false)
    };

    let size = Size {
        width: ((f64::from(base.width) * scale).round() as i32).max(1),
        height: ((f64::from(base.height) * scale).round() as i32).max(1),
    };
    let margin = margin as i32;
    let along = |align, start: i32, extent: i32, len: i32| match align {
        Align::Start => start + margin,
        Align::Centre => start + (extent - len) / 2,
        Align::End => start + extent - margin - len,
    };
    let x = along(edge.horizontal(), screen.x, screen.width, size.width);
    let y = along(edge.vertical(), screen.y, screen.height, size.height);

    Placement {
        x: clamp_into(x, screen.x, screen.width, size.width),
        y: clamp_into(y, screen.y, screen.height, size.height),
        size,
        scale,
        capped,
    }
}

/// Keep `[pos, pos + len)` inside `[start, start + extent)` where possible.
fn clamp_into(pos: i32, start: i32, extent: i32, len: i32) -> i32 {
    let max = start + (extent - len).max(0);
    pos.clamp(start, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A second monitor to the right of a first 1920-wide one.
    const SCREEN: Rect = Rect {
        x: 1920,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const BASE: Size = Size {
        width: 800,
        height: 300,
    };

    #[test]
    fn places_on_every_edge() {
        let at = |edge| {
            let p = place(SCREEN, BASE, edge, 16, 100);
            (p.x, p.y)
        };
        assert_eq!(at(Edge::Top), (1920 + 560, 16));
        assert_eq!(at(Edge::Bottom), (1920 + 560, 1080 - 16 - 300));
        assert_eq!(at(Edge::Left), (1920 + 16, 390));
        assert_eq!(at(Edge::Right), (1920 + 1920 - 16 - 800, 390));
    }

    #[test]
    fn places_in_every_corner() {
        let at = |edge| {
            let p = place(SCREEN, BASE, edge, 16, 100);
            (p.x, p.y)
        };
        assert_eq!(at(Edge::TopLeft), (1920 + 16, 16));
        assert_eq!(at(Edge::TopRight), (1920 + 1920 - 16 - 800, 16));
        assert_eq!(at(Edge::BottomLeft), (1920 + 16, 1080 - 16 - 300));
        assert_eq!(
            at(Edge::BottomRight),
            (1920 + 1920 - 16 - 800, 1080 - 16 - 300)
        );
    }

    #[test]
    fn zero_margin_touches_the_edge() {
        let p = place(SCREEN, BASE, Edge::Bottom, 0, 100);
        assert_eq!(p.y + p.size.height, 1080);
    }

    #[test]
    fn scales_down() {
        let p = place(SCREEN, BASE, Edge::Bottom, 16, 50);
        assert_eq!(
            p.size,
            Size {
                width: 400,
                height: 150
            }
        );
        assert_eq!(p.scale, 0.5);
        assert!(!p.capped);
    }

    #[test]
    fn caps_scale_that_does_not_fit() {
        let small = Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        };
        let p = place(small, BASE, Edge::Bottom, 16, 300);
        assert!(p.capped);
        assert!(p.scale < 3.0);
        assert!(p.size.width <= 1280 - 32);
        assert!(p.size.height <= 720 - 32);
        assert!(p.x >= 0 && p.y >= 0);
    }

    #[test]
    fn base_larger_than_screen_is_capped_at_100_percent() {
        let huge = Size {
            width: 2500,
            height: 400,
        };
        let p = place(SCREEN, huge, Edge::Top, 16, 100);
        assert!(p.capped);
        assert!(p.size.width <= 1920 - 32);
        assert!(p.x >= SCREEN.x);
    }
}
