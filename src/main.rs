use bevy::prelude::*;
use camera::CameraPlugin;
use tile_map::TileMapPlugin;

mod camera;
mod types;
mod api;
mod tile_map;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CameraPlugin)
        .add_plugins(TileMapPlugin)
        .run();
}