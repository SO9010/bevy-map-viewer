use bevy::{prelude::*, window::PrimaryWindow};
use camera::CameraPlugin;
use tile_map::TileMapPlugin;
use types::{world_mercator_to_lat_lon, TileMapResources};

mod api;
mod camera;
mod tile_map;
mod types;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CameraPlugin)
        .add_plugins(TileMapPlugin)
        .add_systems(Update, handle_mouse)
        .run();
}

pub fn handle_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    res_manager: Res<TileMapResources>,
) {
    let (camera, camera_transform) = camera.single();
    if buttons.pressed(MouseButton::Left) {
        if let Some(position) = q_windows.single().cursor_position() {
            let world_pos = camera
                .viewport_to_world_2d(camera_transform, position)
                .unwrap();
            info!(
                "{:?}",
                world_mercator_to_lat_lon(
                    world_pos.x,
                    world_pos.y,
                    res_manager.chunk_manager.refrence_long_lat,
                    res_manager.chunk_manager.displacement,
                    14,
                    res_manager.zoom_manager.tile_size
                )
            );
        }
    }
}
