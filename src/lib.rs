//! A map viewer plugin for the Bevy game engine.
//!
//! This plugin provides functionality to display and navigate maps using
//! different tile providers (raster or vector).

mod api;
mod camera;
mod tile_map;
mod types;

use bevy::prelude::*;

/// Main plugin that combines all functionality
pub struct MapViewerPlugin {
    pub starting_location: Coord,
    pub starting_zoom: u32,
    pub tile_quality: f32,
    pub cache_dir: String,
}

impl Plugin for MapViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(camera::camera_system::CameraSystemPlugin)
            .add_plugins(tile_map::TileMapPlugin {
                starting_location: self.starting_location,
                starting_zoom: self.starting_zoom,
                tile_quality: self.tile_quality,
                cache_dir: self.cache_dir.clone(),
            });
           
    }
}

// Re-export important types so users don't need to import internal modules
pub use camera::camera_helper::CameraTrackingEvent;
pub use tile_map::TileMapPlugin;
pub use types::{TileType, TileMapResources, Coord, UpdateChunkEvent, ZoomChangedEvent};
pub use api::{get_rasta_data, get_mvt_data, buffer_to_bevy_image};