use bevy::{
    core_pipeline::core_2d::Camera2d,
    ecs::{
        event::{Event, EventReader, EventWriter}, query::{Changed, With}, system::{Query, ResMut}
    },
    math::{Vec2, Vec3},
    render::camera::{Camera, OrthographicProjection},
    transform::components::{GlobalTransform, Transform},
    window::Window,
};

#[cfg(feature = "ui_blocking")]
use bevy::ecs::system::Resource;

use crate::types::{game_to_coord, Coord, TileMapResources, UpdateChunkEvent};

#[allow(unused)]
pub fn camera_space_to_lat_long_rect(
    transform: &GlobalTransform,
    window: &Window,
    projection: OrthographicProjection,
    zoom: u32,
    quality: f32,
    reference: Coord,
    // This comes from the zoommanager
    displacement: Vec2,
) -> Option<geo::Rect<f32>> {
    let window_width = window.width();
    let window_height = window.height();

    let camera_translation = transform.translation();

    let left = camera_translation.x - ((window_width * projection.scale) / 2.0);
    let right = camera_translation.x + ((window_width * projection.scale) / 2.0);
    let bottom = camera_translation.y + ((window_height * projection.scale) / 2.0);
    let top = camera_translation.y - ((window_height * projection.scale) / 2.0);

    Some(geo::Rect::<f32>::new(
        game_to_coord(
            left,
            bottom,
            reference,
            displacement,
            zoom,
            quality,
        )
        .to_tuple(),
        game_to_coord(
            right,
            top,
            reference,
            displacement,
            zoom,
            quality,
        )
        .to_tuple(),
    ))
}

#[allow(unused)]
pub fn camera_middle_to_lat_long(
    zoom: u32,
    quality: f32,
    reference: Coord,
    camera_query: Query<&mut Transform, With<Camera>>,
    // This comes from the zoommanager
    displacement: Vec2,
) -> Coord {
    let camera_translation = camera_query.get_single().unwrap().translation;
    game_to_coord(
        camera_translation.x,
        camera_translation.y,
        reference,
        displacement,
        zoom,
        quality,
    )
}

#[derive(Event)]
pub struct CameraTrackingEvent {
    pub position: Vec3,
}

pub fn track_camera_position(
    camera_query: Query<&GlobalTransform, (With<Camera2d>, Changed<GlobalTransform>)>,
    mut camera_event_writer: EventWriter<CameraTrackingEvent>,
) {
    if let Ok(transform) = camera_query.get_single() {
        let new_position = transform.translation();
        camera_event_writer.send(CameraTrackingEvent {
            position: new_position,
        });
    }
}

pub fn camera_change(
    mut tile_map_res: ResMut<TileMapResources>,
    mut camera_event_reader: EventReader<CameraTrackingEvent>,
    mut event_writer: EventWriter<UpdateChunkEvent>,
) {
    for position in camera_event_reader.read() {
        let movement = game_to_coord(
            position.position.x,
            position.position.y,
            tile_map_res.chunk_manager.refrence_long_lat,
            tile_map_res.chunk_manager.displacement,
            14,
            tile_map_res.zoom_manager.tile_quality,
        );

        if movement != tile_map_res.location_manager.location {
            tile_map_res.location_manager.location = movement;
            event_writer.send(UpdateChunkEvent);
        }
    }
}

#[cfg(feature = "ui_blocking")]
#[derive(Resource, Default)]
pub struct EguiBlockInputState {
    pub block_input: bool,
}
#[cfg(feature = "ui_blocking")]
pub fn absorb_egui_inputs(
    mut contexts: bevy_egui::EguiContexts,
    mut state: ResMut<EguiBlockInputState>,
) {
    let ctx = contexts.ctx_mut();
    state.block_input = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}
