//! A bounded floor plan for maps without an official minimap image.
use image::{Rgba, RgbaImage};
use nostalre_formats::gat::GatFile;

const SIZE: u32 = 256;
const OUTSIDE: Rgba<u8> = Rgba([20, 25, 31, 255]);
const WALL: Rgba<u8> = Rgba([48, 55, 63, 255]);
const FLOOR: Rgba<u8> = Rgba([168, 178, 157, 255]);
const WATER: Rgba<u8> = Rgba([69, 122, 159, 255]);

pub(crate) fn floor_plan(gat: &GatFile) -> Option<RgbaImage> {
    let width = usize::try_from(gat.width).ok()?;
    let height = usize::try_from(gat.height).ok()?;
    if width == 0 || height == 0 || gat.cells.len() < width.checked_mul(height)? {
        return None;
    }
    let w = width as f32;
    let h = height as f32;
    let span = w.max(h);
    // Match MinimapWindow::cell_to_uv: north up, uniform scale, centred
    // in a square even when the map itself is rectangular.
    Some(RgbaImage::from_fn(SIZE, SIZE, |px, py| {
        let x = ((px as f32 + 0.5) / SIZE as f32 - 0.5) * span + w / 2.0;
        let y = (0.5 - (py as f32 + 0.5) / SIZE as f32) * span + h / 2.0;
        if x < 0.0 || y < 0.0 || x >= w || y >= h {
            return OUTSIDE;
        }
        let cell = &gat.cells[y as usize * width + x as usize];
        if cell.is_water() {
            WATER
        } else if cell.is_walkable() {
            FLOOR
        } else {
            WALL
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_formats::gat::GatCell;

    fn gat(width: i32, height: i32, types: &[i32]) -> GatFile {
        let mut data = b"GRAT\x01\x02".to_vec();
        data.extend(width.to_le_bytes());
        data.extend(height.to_le_bytes());
        for kind in types {
            data.extend([0; 16]);
            data.extend(kind.to_le_bytes());
        }
        GatFile::parse(&data).unwrap()
    }

    #[test]
    fn north_up_floor_water_and_wall_match_the_player_coordinate_system() {
        let image = floor_plan(&gat(2, 2, &[0, 1, 3, 0])).unwrap();
        assert_eq!(*image.get_pixel(64, 192), FLOOR); // south-west
        assert_eq!(*image.get_pixel(192, 192), WALL); // south-east
        assert_eq!(*image.get_pixel(64, 64), WATER); // north-west
        assert_eq!(*image.get_pixel(192, 64), FLOOR);
    }

    #[test]
    fn rectangular_map_has_centred_padding_and_uniform_scale() {
        let image = floor_plan(&gat(4, 2, &[0; 8])).unwrap();
        assert_eq!(*image.get_pixel(128, 32), OUTSIDE);
        assert_eq!(*image.get_pixel(128, 80), FLOOR);
        assert_eq!(*image.get_pixel(128, 224), OUTSIDE);
        assert_eq!(*image.get_pixel(16, 128), FLOOR);
        let portrait = floor_plan(&gat(2, 4, &[0; 8])).unwrap();
        assert_eq!(*portrait.get_pixel(32, 128), OUTSIDE);
        assert_eq!(*portrait.get_pixel(80, 128), FLOOR);
        assert_eq!(*portrait.get_pixel(128, 16), FLOOR);
    }

    #[test]
    fn malformed_geometry_is_rejected_without_indexing_out_of_bounds() {
        let mut map = gat(1, 1, &[0]);
        map.width = -1;
        assert!(floor_plan(&map).is_none());
        map.width = 0;
        assert!(floor_plan(&map).is_none());
        map.width = 2;
        assert!(floor_plan(&map).is_none());
        map.width = 1;
        map.height = 0;
        assert!(floor_plan(&map).is_none());
    }

    #[test]
    fn texture_memory_stays_bounded_for_large_maps() {
        let map = GatFile {
            version: (1, 2), width: 4096, height: 1,
            cells: (0..4096).map(|_| GatCell {
                height_sw: 0.0, height_se: 0.0, height_nw: 0.0, height_ne: 0.0,
                cell_flags: 1,
            }).collect(),
        };
        let image = floor_plan(&map).unwrap();
        assert_eq!(image.dimensions(), (256, 256));
        assert_eq!(image.as_raw().len(), 256 * 256 * 4);
    }
}
