use bevy::{
    input::mouse::MouseWheel, prelude::*, render::view::RenderLayers, window::PrimaryWindow,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;

#[cfg(feature = "ui_blocking")]
use crate::camera::camera_helper::EguiBlockInputState;
use crate::{
    api::buffer_to_bevy_image,
    types::{
        game_to_coord, Coord, InitTileMapPlugin, TileMapResources, UpdateChunkEvent,
        ZoomChangedEvent,
    },
    MapViewerMarker,
};

//------------------------------------------------------------------------------
// Plugin
//------------------------------------------------------------------------------
pub struct TileMapPlugin {
    pub starting_location: Coord,
    pub starting_zoom: u32,
    pub starting_url: Option<String>,
    pub tile_quality: f32,
    pub cache_dir: String,
}

impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx): (ChunkSenderType, ChunkReceiverType) = bounded(10);
        app.insert_resource(ChunkReceiver(rx))
            .insert_resource(ChunkSender(tx))
            .add_plugins(InitTileMapPlugin {
                starting_location: self.starting_location,
                starting_zoom: self.starting_zoom,
                tile_quality: self.tile_quality,
                cache_dir: self.cache_dir.clone(),
                starting_url: self.starting_url.clone(),
            })
            .insert_resource(Clean::default())
            .add_systems(Update, detect_zoom_level)
            .add_systems(
                FixedUpdate,
                (
                    despawn_outofrange_chunks,
                    read_tile_map_receiver,
                    clean_tile_map,
                    spawn_chunks_around_middle,
                    spawn_to_needed_chunks,
                )
                    .chain(),
            )
            .insert_resource(ZoomCooldown(Timer::from_seconds(0.2, TimerMode::Repeating)))
            .insert_resource(MoveCooldown(Timer::from_seconds(0.2, TimerMode::Repeating)));
    }
}

fn spawn_chunks_around_middle(
    chunk_sender: Res<ChunkSender>,
    mut res_manager: ResMut<TileMapResources>,
    mut camera_event_reader: EventReader<UpdateChunkEvent>,
    mut cooldown: ResMut<MoveCooldown>,
    time: Res<Time>,
    #[cfg(feature = "ui_blocking")] state: Res<EguiBlockInputState>,
) {
    #[cfg(feature = "ui_blocking")]
    if state.block_input {
        return;
    }
    if cooldown.0.tick(time.delta()).finished() && !camera_event_reader.is_empty() {
        camera_event_reader.clear();
        let chunk_pos = camera_pos_to_chunk_pos(
            &res_manager.location_manager_to_point(),
            res_manager.zoom_manager.tile_quality,
        );
        let range = 4;

        for y in (chunk_pos.y - range)..=(chunk_pos.y + range) {
            for x in (chunk_pos.x - range)..=(chunk_pos.x + range) {
                let chunk_pos = IVec2::new(x, y);
                if !res_manager
                    .chunk_manager
                    .spawned_chunks
                    .contains(&chunk_pos)
                {
                    let tx = chunk_sender.clone();
                    let zoom_manager = res_manager.zoom_manager.clone();
                    let refrence_long_lat = res_manager.chunk_manager.refrence_long_lat;
                    let world_pos = chunk_pos_to_world_pos(chunk_pos, zoom_manager.tile_quality);
                    let position = game_to_coord(
                        world_pos.x,
                        world_pos.y,
                        refrence_long_lat,
                        Vec2::ZERO,
                        res_manager.zoom_manager.zoom_level,
                        zoom_manager.tile_quality,
                    );
                    let tile_requester = res_manager.tile_request_client.clone();
                    thread::spawn(move || {
                        let tile_coords = position.to_tile_coords(zoom_manager.zoom_level);
                        let _ = tx.send((
                            chunk_pos,
                            tile_requester.get_tile(
                                tile_coords.x as u64,
                                tile_coords.y as u64,
                                zoom_manager.zoom_level as u64,
                            ),
                        ));
                    });

                    res_manager.chunk_manager.spawned_chunks.insert(chunk_pos);
                }
            }
        }
    }
}

// Zoom handling //
#[derive(Resource)]
struct ZoomCooldown(pub Timer);

#[derive(Resource)]
struct MoveCooldown(pub Timer);

fn camera_rect(window: &Window, projection: Projection) -> (f32, f32) {
    match projection {
        Projection::Orthographic(ortho) => {
            let width = ortho.area.width();
            let height = ortho.area.height();
            (width, height)
        }
        Projection::Perspective(_) => {
            let width = window.width();
            let height = window.height();
            (width, height)
        }
        Projection::Custom(_) => {
            error!("Custom projection is not supported");
            (0.0, 0.0)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_zoom_level(
    mut res_manager: ResMut<TileMapResources>,
    mut ortho_projection_query: Query<&mut Projection, With<MapViewerMarker>>,
    mut camera_query: Query<&mut Transform, With<MapViewerMarker>>,
    #[cfg(feature = "ui_blocking")] state: Res<EguiBlockInputState>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    mut cooldown: ResMut<ZoomCooldown>,
    time: Res<Time>,
    mut clean: ResMut<Clean>,
    evr_scroll: EventReader<MouseWheel>,
    mut zoom_event: EventWriter<ZoomChangedEvent>,
    mut chunk_writer: EventWriter<UpdateChunkEvent>,
) {
    if !evr_scroll.is_empty() {
        cooldown.0.reset();
    }

    #[cfg(feature = "ui_blocking")]
    if state.block_input {
        return;
    }
    if cooldown.0.tick(time.delta()).finished() {
        let mut changed = false;
        if let Ok(projection) = ortho_projection_query.single_mut() {
            let mut width = camera_rect(
                q_windows
                    .single()
                    .expect("Fail because of not being  bale to get window"),
                projection.clone(),
            )
            .0 / res_manager.zoom_manager.tile_quality
                / res_manager.zoom_manager.scale.x;

            while !(3. ..=7.).contains(&width) {
                if width > 7. && res_manager.zoom_manager.zoom_level > 3 {
                    res_manager.zoom_manager.zoom_level -= 1;
                    res_manager.zoom_manager.scale *= 2.0;
                    res_manager.chunk_manager.refrence_long_lat *= Coord { lat: 2., long: 2. };
                    changed = true;
                } else if width < 3. && res_manager.zoom_manager.zoom_level < 20 {
                    res_manager.zoom_manager.scale /= 2.0;
                    res_manager.zoom_manager.zoom_level += 1;
                    res_manager.chunk_manager.refrence_long_lat /= Coord { lat: 2., long: 2. };
                    changed = true;
                } else {
                    return;
                }
                width = camera_rect(
                    q_windows
                        .single()
                        .expect("Fail because of not being  bale to get window"),
                    projection.clone(),
                )
                .0 / res_manager.zoom_manager.tile_quality
                    / res_manager.zoom_manager.scale.x;
            }

            if changed {
                if let Ok(camera) = camera_query.single_mut() {
                    let reference_point = res_manager.location_manager_to_point();
                    let camera_pos_unscaled =
                        camera.translation.xy() / res_manager.zoom_manager.scale.xy();
                    res_manager.chunk_manager.displacement = (reference_point
                        - camera_pos_unscaled)
                        * res_manager.zoom_manager.scale.xy();
                }
                let layer = res_manager.chunk_manager.layer_management.last().unwrap() + 1.0;
                res_manager.chunk_manager.layer_management.push(layer);
                res_manager.zoom_manager.scale.z = layer;

                zoom_event.write(ZoomChangedEvent);
                chunk_writer.write(UpdateChunkEvent);
                res_manager.chunk_manager.spawned_chunks.clear();
                res_manager.chunk_manager.to_spawn_chunks.clear();
                cooldown.0.reset();
            }
        } else {
            error!("Failed to get camera projection");
        }
    }
    if res_manager.tile_request_client.tile_web_origin_changed {
        res_manager.tile_request_client.tile_web_origin_changed = false;
        chunk_writer.write(UpdateChunkEvent);
        clean.clean = true;
        cooldown.0.reset();
    }
}

// Chunk handling //

type ChunkData = (IVec2, Result<Vec<u8>, image::ImageError>);
type ChunkSenderType = Sender<ChunkData>;
type ChunkReceiverType = Receiver<ChunkData>;

#[derive(Component)]
#[allow(unused)]
// Chunklayer and chunk location
struct ChunkLayer(f32, IVec2);

#[derive(Component)]
struct TileMarker;

#[derive(Resource, Deref)]
struct ChunkReceiver(Receiver<(IVec2, Result<Vec<u8>, image::ImageError>)>); // Use Vec<u8> for raw image data

#[derive(Resource, Deref)]
struct ChunkSender(Sender<(IVec2, Result<Vec<u8>, image::ImageError>)>);

fn camera_pos_to_chunk_pos(camera_pos: &Vec2, tile_quality: f32) -> IVec2 {
    let camera_pos = Vec2::new(camera_pos.x, camera_pos.y) / tile_quality;
    camera_pos.floor().as_ivec2()
}

fn chunk_pos_to_world_pos(chunk_pos: IVec2, tile_quality: f32) -> Vec2 {
    Vec2::new(
        chunk_pos.x as f32 * tile_quality,
        chunk_pos.y as f32 * tile_quality,
    )
}

fn read_tile_map_receiver(
    map_receiver: Res<ChunkReceiver>,
    mut res_manager: ResMut<TileMapResources>,
) {
    let mut new_chunks = Vec::new();
    while let Ok((chunk_pos, raw_image_data)) = map_receiver.try_recv() {
        if !res_manager
            .chunk_manager
            .to_spawn_chunks
            .contains_key(&chunk_pos)
            && raw_image_data.is_ok()
        {
            new_chunks.push((chunk_pos, raw_image_data));
        }
    }

    for (pos, data) in new_chunks {
        if let Ok(data) = data {
            res_manager.chunk_manager.to_spawn_chunks.insert(pos, data);
        }
    }
}

fn spawn_to_needed_chunks(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut res_manager: ResMut<TileMapResources>,
) {
    let to_spawn_chunks: Vec<(IVec2, Vec<u8>)> = res_manager
        .chunk_manager
        .to_spawn_chunks
        .iter()
        .map(|(pos, data)| (*pos, data.clone()))
        .collect();
    for (chunk_pos, raw_image_data) in to_spawn_chunks {
        let tile_handle = images.add(buffer_to_bevy_image(
            raw_image_data,
            res_manager.zoom_manager.tile_quality as u32,
        ));
        res_manager.chunk_manager.spawned_chunks.insert(chunk_pos);
        spawn_chunk(
            &mut commands,
            tile_handle,
            chunk_pos,
            res_manager.zoom_manager.tile_quality,
            res_manager.zoom_manager.scale,
            res_manager.chunk_manager.displacement,
        );
    }
    res_manager.chunk_manager.to_spawn_chunks.clear();
}

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
    tile_quality: f32,
    scale: Vec3,
    offset: Vec2,
) {
    let world_x = chunk_pos.x as f32 * tile_quality * scale.x - offset.x;
    let world_y = chunk_pos.y as f32 * tile_quality * scale.x - offset.y;
    commands.spawn((
        (
            Sprite::from_image(tile),
            Transform::from_translation(Vec3::new(world_x, world_y, scale.z)).with_scale(scale),
            Visibility::Visible,
        ),
        ChunkLayer(scale.z, chunk_pos),
        TileMarker,
        RenderLayers::layer(0),
    ));
}

// Despawn handling //

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<MapViewerMarker>>,
    chunks_query: Query<(Entity, &Transform, &ChunkLayer)>,
    mut res_manager: ResMut<TileMapResources>,
) {
    let mut chunks_to_remove = Vec::new();

    if res_manager.chunk_manager.layer_management.len() > 2 {
        let oldest_layer = res_manager.chunk_manager.layer_management[0];
        for (entity, _, chunk_layer) in chunks_query.iter() {
            if chunk_layer.0 == oldest_layer && !chunks_to_remove.contains(&(entity, chunk_layer.1))
            {
                chunks_to_remove.push((entity, chunk_layer.1));
            }
        }
        res_manager.chunk_manager.layer_management.remove(0);
    }

    if let Ok(camera_transform) = camera_query.single() {
        let camera_pos = camera_transform.translation.xy();
        for (entity, chunk_transform, chunk_layer) in chunks_query.iter() {
            let chunk_world_pos: Vec2 = chunk_transform.translation.xy();
            let distance = camera_pos.distance(chunk_world_pos);

            let threshold =
                res_manager.zoom_manager.tile_quality * 10.0 * res_manager.zoom_manager.scale.x;

            if distance > threshold && !chunks_to_remove.contains(&(entity, chunk_layer.1)) {
                chunks_to_remove.push((entity, chunk_layer.1));
            }
        }
    }

    for (entity, chunk_pos) in chunks_to_remove {
        res_manager.chunk_manager.spawned_chunks.remove(&chunk_pos);
        commands.entity(entity).despawn();
    }
}

#[derive(Resource, Clone, Default)]
struct Clean {
    clean: bool,
}

#[allow(unused)]
fn clean_tile_map(
    mut res_manager: ResMut<TileMapResources>,
    mut commands: Commands,
    chunk_query: Query<(Entity, &ChunkLayer)>,
    mut clean: ResMut<Clean>,
) {
    if clean.clean {
        clean.clean = false;
        for (entity, _) in chunk_query.iter() {
            commands.entity(entity).despawn();
        }
        res_manager.chunk_manager.spawned_chunks.clear();
        res_manager.chunk_manager.to_spawn_chunks.clear();
    }
}
