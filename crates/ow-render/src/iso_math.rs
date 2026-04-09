//! Isometric coordinate math: screen ↔ tile conversions.

use tracing::trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TilePos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenPos {
    pub x: f32,
    pub y: f32,
}

pub struct IsoConfig {
    pub tile_width: f32,
    pub tile_height: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl IsoConfig {
    pub fn tile_to_screen(&self, tile: TilePos) -> ScreenPos {
        let sx = self.origin_x + (tile.x - tile.y) as f32 * (self.tile_width / 2.0);
        let sy = self.origin_y + (tile.x + tile.y) as f32 * (self.tile_height / 2.0);
        trace!(tx = tile.x, ty = tile.y, sx, sy, "tile->screen");
        ScreenPos { x: sx, y: sy }
    }

    pub fn screen_to_tile(&self, screen: ScreenPos) -> TilePos {
        let rx = (screen.x - self.origin_x) / (self.tile_width / 2.0);
        let ry = (screen.y - self.origin_y) / (self.tile_height / 2.0);
        let tx = ((rx + ry) / 2.0).floor() as i32;
        let ty = ((ry - rx) / 2.0).floor() as i32;
        trace!(sx = screen.x, sy = screen.y, tx, ty, "screen->tile");
        TilePos { x: tx, y: ty }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let cfg = IsoConfig {
            tile_width: 64.0,
            tile_height: 32.0,
            origin_x: 0.0,
            origin_y: 0.0,
        };
        let tile = TilePos { x: 5, y: 3 };
        let screen = cfg.tile_to_screen(tile);
        let back = cfg.screen_to_tile(ScreenPos {
            x: screen.x + 1.0,
            y: screen.y + 1.0,
        });
        assert_eq!(back, tile);
    }
}
