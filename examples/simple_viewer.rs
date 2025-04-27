use bevy::{prelude::*, render::view::RenderLayers, window::PrimaryWindow};
use bevy_egui::EguiPlugin;
use bevy_map_viewer::{Coord, MapViewerMarker, MapViewerPlugin, TileMapResources};
use bevy_pancam::{DirectionKeys, PanCam, PanCamPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: true,
        })
        .add_plugins(PanCamPlugin)
        .add_plugins(MapViewerPlugin {
            starting_location: Coord::new(52.1951, 0.1313),
            starting_zoom: 14,
            tile_quality: 256.0,
            cache_dir: "cache".to_string(),
            starting_url: None,
        })
        .add_systems(Startup, setup_camera)
        .add_systems(Update, handle_mouse)
        .run();
}

fn handle_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    res_manager: Res<TileMapResources>,
) {
    if let Ok((camera, camera_transform)) = camera.single() {
        if buttons.pressed(MouseButton::Left) {
            if let Some(position) = q_windows
                .single()
                .expect("Unable to get cursor position")
                .cursor_position()
            {
                let world_pos = camera
                    .viewport_to_world_2d(camera_transform, position)
                    .unwrap();

                info!(
                    "{:?} | {:?} | {:?}",
                    world_pos,
                    res_manager.point_to_coord(world_pos),
                    res_manager.coord_to_point(res_manager.point_to_coord(world_pos)),
                );
            }
        }
    }
}

fn setup_camera(mut commands: Commands, res_manager: Option<Res<TileMapResources>>) {
    if let Some(res_manager) = res_manager {
        let starting = res_manager
            .location_manager
            .location
            .to_game_coords(res_manager.clone());
        commands.spawn((
            Camera2d,
            MapViewerMarker,
            RenderLayers::from_layers(&[0]),
            Camera {
                order: 0,
                ..default()
            },
            Transform {
                translation: Vec3::new(starting.x, starting.y, 1.0),
                ..Default::default()
            },
            PanCam {
                grab_buttons: vec![MouseButton::Middle],
                move_keys: DirectionKeys {
                    up: vec![KeyCode::ArrowUp],
                    down: vec![KeyCode::ArrowDown],
                    left: vec![KeyCode::ArrowLeft],
                    right: vec![KeyCode::ArrowRight],
                },
                speed: 150.,
                enabled: true,
                zoom_to_cursor: true,
                min_scale: f32::NEG_INFINITY,
                max_scale: f32::INFINITY,
                min_x: f32::NEG_INFINITY,
                max_x: f32::INFINITY,
                min_y: f32::NEG_INFINITY,
                max_y: f32::INFINITY,
            },
        ));
    } else {
        error!("TileMapResources not found. Please add the tilemap addon first.");
    }
}
