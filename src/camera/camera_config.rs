use bevy::ecs::system::Resource;

use crate::types::Coord;

#[derive(Resource, Clone)]
pub struct CameraConfig {
    pub enable_camera: bool,
    pub enable_pancam: bool,
    // The need for this displacement is that i have found that for the tiles to be correctly gotten we need an offset of 0.011deg
    // Change if needed.
    pub displacement: Coord,
    // Default 256, change if you want better resultion.
    pub tile_quality: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            enable_camera: true,
            enable_pancam: true,
            displacement: Coord::new(0.011, 0.011),
            tile_quality: 256.0,
        }
    }
}
