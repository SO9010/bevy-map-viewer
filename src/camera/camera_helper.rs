use bevy::{core_pipeline::core_2d::Camera2d, ecs::{query::{Changed, With}, system::{Query, Res, ResMut, Resource}}, math::Vec3, render::camera::{Camera, OrthographicProjection}, transform::components::GlobalTransform, window::Window};

use crate::{tile_map::TileMapResources, types::{world_mercator_to_lat_lon, Coord}};



#[derive(Resource, Default)]
pub struct CameraPosition {
    pub position: Vec3,
    pub changed: bool,
}

pub fn camera_rect(
    window: &Window,
    projection: OrthographicProjection,
) -> (f32, f32) {
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
) -> Option<geo::Rect<f32>> {
    let window_width = window.width(); 
    let window_height = window.height();

    let camera_translation = transform.translation();

    let left = camera_translation.x - ((window_width * projection.scale) / 2.0);
    let right = camera_translation.x  + ((window_width * projection.scale) / 2.0);
    let bottom = camera_translation.y + ((window_height * projection.scale) / 2.0);
    let top = camera_translation.y  - ((window_height * projection.scale) / 2.0);
    
    Some(geo::Rect::<f32>::new(
        world_mercator_to_lat_lon(left.into(), bottom.into(), reference, zoom, quality).to_tuple(),
        world_mercator_to_lat_lon(right.into(), top.into(), reference, zoom, quality).to_tuple(),
    ))
}

pub fn camera_middle_to_lat_long(
    transform: &GlobalTransform,
    zoom: u32,
    quality: f32,
    reference: Coord,
) -> Coord {
    let camera_translation = transform.translation();
    world_mercator_to_lat_lon(camera_translation.x.into(), camera_translation.y.into(), reference, zoom, quality)
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
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    camera_position: Res<CameraPosition>,
    mut tile_map_res: ResMut<TileMapResources>,
) {
    let (_, camera_transform) = camera.single();
    if camera_position.changed {
        let movement = camera_middle_to_lat_long(camera_transform, tile_map_res.zoom_manager.zoom_level, tile_map_res.zoom_manager.tile_size, tile_map_res.chunk_manager.refrence_long_lat);
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