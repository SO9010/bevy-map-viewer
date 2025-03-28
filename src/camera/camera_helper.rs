use bevy::{
    core_pipeline::core_2d::Camera2d,
    ecs::{
        query::{Changed, With},
        system::{Query, Res, ResMut, Resource},
    },
    log::info,
    math::Vec2,
    math::Vec3,
    render::camera::{Camera, OrthographicProjection},
    transform::components::{GlobalTransform, Transform},
    window::Window,
};

use crate::types::{world_mercator_to_lat_lon, Coord, TileMapResources};

#[derive(Resource, Default)]
pub struct CameraPosition {
    pub position: Vec3,
    pub changed: bool,
}

pub fn camera_rect(window: &Window, projection: OrthographicProjection) -> (f32, f32) {
    let window_width = window.width() * projection.scale;
    let window_height = window.height() * projection.scale;
    (window_width, window_height)
}

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
        world_mercator_to_lat_lon(
            left.into(),
            bottom.into(),
            reference,
            displacement,
            zoom,
            quality,
        )
        .to_tuple(),
        world_mercator_to_lat_lon(
            right.into(),
            top.into(),
            reference,
            displacement,
            zoom,
            quality,
        )
        .to_tuple(),
    ))
}

pub fn camera_middle_to_lat_long(
    zoom: u32,
    quality: f32,
    reference: Coord,
    camera_query: Query<&mut Transform, With<Camera>>,
    // This comes from the zoommanager
    displacement: Vec2,
) -> Coord {
    let camera_translation = camera_query.get_single().unwrap().translation;
    world_mercator_to_lat_lon(
        camera_translation.x.into(),
        camera_translation.y.into(),
        reference,
        displacement,
        zoom,
        quality,
    )
}

pub fn track_camera_position(
    camera_query: Query<&GlobalTransform, (With<Camera2d>, Changed<GlobalTransform>)>,
    mut camera_position: ResMut<CameraPosition>,
) {
    camera_position.changed = false;

    if let Ok(transform) = camera_query.get_single() {
        let new_position = transform.translation();
        // Check if position has changed
        if new_position != camera_position.position {
            camera_position.position = new_position;
            camera_position.changed = true;
        }
    }
}

// Rewrite this to use event reader.
pub fn camera_change(
    camera_position: Res<CameraPosition>,
    mut tile_map_res: ResMut<TileMapResources>,
) {
    if camera_position.changed {
        let movement = world_mercator_to_lat_lon(
            camera_position.position.x,
            camera_position.position.y,
            tile_map_res.chunk_manager.refrence_long_lat,
            tile_map_res.chunk_manager.displacement,
            14,
            tile_map_res.zoom_manager.tile_size,
        );
        info!("Camera moved to: {:?}", movement);

        if movement != tile_map_res.location_manager.location {
            tile_map_res.location_manager.location = movement;
            tile_map_res.chunk_manager.update = true;
        }
    }
}

#[derive(Resource, Default)]
pub struct EguiBlockInputState {
    pub block_input: bool,
}
pub fn absorb_egui_inputs(
    mut contexts: bevy_egui::EguiContexts,
    mut state: ResMut<EguiBlockInputState>,
) {
    let ctx = contexts.ctx_mut();
    state.block_input = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}
