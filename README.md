# Bevy Map Viewer

Bevy Map Viewer is a plugin for the [Bevy game engine](https://bevyengine.org/) that allows you to display and navigate maps using raster or vector tile providers. It supports OpenStreetMap, Google Maps, and other XYZ tile providers.

## Features

- Display raster and vector map tiles.
- Supports multiple tile providers (e.g., OpenStreetMap, Google Maps).
- Smooth zooming and panning.
- Caching of tiles for offline use.
- Event-driven camera tracking and chunk updates.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
bevy_map_viewer = "0.1.0"
```
## !Important! Render layers WIP

This plugin uses Bevy's [RenderLayers](https://docs.rs/bevy/latest/bevy/render/view/struct.RenderLayers.html) system to manage how map tiles and game entities are displayed. Map tiles are rendered on layer 0 by default.

To help ensure your game entities appear above the map tiles:

1. Add your game entities to render layer 1
2. Configure it so you have two cameras, one which lookes at layer 1, and one that looks at layer 0 (this will be the layer that has the map)
3. Add the map marker `MapViewerMarker` to the map
4. Make the camera order so that the layer 1 is on top

```rust
fn setup_camera(mut commands: Commands, res_manager: Option<Res<TileMapResources>>) {
    if let Some(res_manager) = res_manager {
        let starting = res_manager
            .location_manager
            .location
            .to_game_coords(res_manager.clone());
        
        commands.spawn((
            Camera2d,
            DrawCamera,
            RenderLayers::from_layers(&[1]),
            Camera { 
                order: 1,
                ..default() 
            },
            Transform {
                translation: Vec3::new(starting.x, starting.y, 1.0),
                ..Default::default()
            },
        ));
        commands.spawn((
            Camera2d,
            MapViewerMarker,
            RenderLayers::from_layers(&[0]),
            Camera { 
                order: 0,
                ..default() 
            },
            Transform {
                translation: Vec3::new(starting.x, starting.y, 0.0),
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
                speed: 400.,
                enabled: true,
                zoom_to_cursor: true,
                min_scale: 0.01,
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

fn sync_cameras(
    primary_query: Query<(&Transform, &OrthographicProjection), With<MapViewerMarker>>,
    mut secondary_query: Query<(&mut Transform, &mut OrthographicProjection), (With<DrawCamera>, Without<MapViewerMarker>)>,
) {
    if let Ok((primary_transform, primary_projection)) = primary_query.get_single() {
        if let Ok((mut secondary_transform, mut secondary_projection)) = secondary_query.get_single_mut() {
            secondary_transform.translation.x = primary_transform.translation.x;
            secondary_transform.translation.y = primary_transform.translation.y;
            secondary_transform.scale = primary_transform.scale;
            
            secondary_projection.scale = primary_projection.scale;
            secondary_projection.area = primary_projection.area;
            secondary_projection.far = primary_projection.far;
            secondary_projection.near = primary_projection.near;
        }
    }
}
```

Without this layering, your game entities might be hidden behind map tiles when they overlap, causing visual glitches and display issues.

## Usage

### Basic Example

Here is a simple example to get started:

```rust
use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::EguiPlugin;
use bevy_map_viewer::{Coord, MapViewerPlugin, TileMapResources};
use bevy_pancam::{DirectionKeys, PanCam, PanCamPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_plugins(PanCamPlugin)
            .add_plugins(MapViewerPlugin { 
            starting_location: Coord::new(52.1951, 0.1313),
            starting_zoom: 14,
            tile_quality: 256.0,
            cache_dir: "cache".to_string(),
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
    let (camera, camera_transform) = camera.single();
    if buttons.pressed(MouseButton::Left) {
        if let Some(position) = q_windows.single().cursor_position() {
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

fn setup_camera(
    mut commands: Commands,
    res_manager: Option<Res<TileMapResources>>,
) {
    if let Some(res_manager) = res_manager {
        let starting = res_manager.location_manager.location.to_game_coords(res_manager.clone());

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
                    up:    vec![KeyCode::ArrowUp],
                    down:  vec![KeyCode::ArrowDown],
                    left:  vec![KeyCode::ArrowLeft],
                    right: vec![KeyCode::ArrowRight],
                },
                speed: 400.,
                enabled: true,
                zoom_to_cursor: false,
                min_scale: 0.01,
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
```

### Features in Detail

- **Tile Providers**: Easily switch between raster and vector tile providers.
- **Caching**: Tiles are cached locally to improve performance and enable offline usage.
- **Zoom and Pan**: Smooth zooming and panning with configurable zoom levels.
- **Event System**: React to camera movements and zoom changes with events.

## Configuration

The `MapViewerPlugin` can be configured with the following parameters:

- `starting_location`: The initial latitude and longitude of the map.
- `starting_zoom`: The initial zoom level.
- `tile_quality`: The resolution of the tiles, try keep this to 256 to not have any issues.
- `cache_dir`: The directory where tiles are cached.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## Contact

For questions or feedback, contact [Samuel Oldham](mailto:so9010sami@gmail.com).
