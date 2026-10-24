//! A colour per layer (feature 007, added on request): each layer name always
//! maps to the same colour from a fixed palette, with nothing to configure.
//! Used by the tray icons and the board's state dot.

/// Well separated hues, all dark enough for white text on top.
const PALETTE: [(u8, u8, u8); 8] = [
    (0x3b, 0x82, 0xf6), // blue
    (0x16, 0xa3, 0x4a), // green
    (0xd9, 0x77, 0x06), // amber
    (0xdc, 0x26, 0x26), // red
    (0x93, 0x33, 0xea), // purple
    (0x08, 0x91, 0xb2), // cyan
    (0xdb, 0x27, 0x77), // pink
    (0x65, 0x73, 0x0d), // olive
];

/// The colour of a layer: stable across runs and machines (FNV-1a of the name).
pub fn layer_color(layer: &str) -> (u8, u8, u8) {
    let hash = layer.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    PALETTE[hash as usize % PALETTE.len()]
}

/// `#rrggbb`, for QML.
pub fn layer_color_hex(layer: &str) -> String {
    let (r, g, b) = layer_color(layer);
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_from_the_palette() {
        assert_eq!(layer_color("nav"), layer_color("nav"));
        assert!(PALETTE.contains(&layer_color("base")));
        assert_eq!(layer_color_hex("base").len(), 7);
        // Typical layer names don't all collapse onto one colour.
        let names = ["base", "nav", "game", "symbols", "num", "media"];
        let distinct: std::collections::HashSet<_> = names.iter().map(|n| layer_color(n)).collect();
        assert!(distinct.len() >= 4, "{distinct:?}");
    }
}
