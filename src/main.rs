use bevy::{prelude::*, window::PrimaryWindow};
use camera::CameraPlugin;
use tile_map::TileMapPlugin;
use types::TileMapResources;

mod api;
mod camera;
mod tile_map;
mod types;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TileMapPlugin)
        .add_plugins(CameraPlugin)
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
                res_manager.point_to_coord(world_pos),
            );
        }
    }
}
