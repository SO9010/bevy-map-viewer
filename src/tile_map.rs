// Ok so this section needs to be rewritten. The current implementation is not optimal and is not working as expected.
// So the idea is to ditch bevy ecs tile map. not only will this make it so we have less imports but it will give us more control.
// So each tile when we want more detail gets put into quaters like this 

/*
┌───┬───┐
│ 0 │ 1 │   
├───┼───┤   ====>   (2x, 2y), (2x+1, 2y), (2x, 2y+1), (2x+1, 2y+1)
│ 2 │ 3 │
└───┴───┘
*/

// Oh damn we should use a quad tree for this. That is the perfect way of doing it. This is how a tile map works lmao.
// Each time we zoom in the new tiles will be needed to be divided by 2 or by as many zoomins we have done. This will ensure that the tiles overlap and dont move spaces.
// We want to overlap the tiles, but we need to figure out when to remove the overapped tiles to ensure we dont exessivly spawn tiles.
// Ok so first lets figure out how to spawn in the tiles.

// Use sprites.

// We want to also use bevy thread pools. 

// So the functions I will want will be:
// 1. spawn_chunk
// 2. camera_pos_to_chunk_pos
// 3. chunk_pos_to_world_pos
// 4. Get camera position
// 5. Get tile size
// 6. Load visible tiles
// 7. Render tiles
// 8. Despawn out of range tiles
// 9. Handle zoom
// 10. Adjust based of viewport.

use std::thread;
use bevy::{input::mouse::MouseWheel, prelude::*, window::PrimaryWindow};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{api::{buffer_to_bevy_image, get_mvt_data, get_rasta_data}, camera::camera_helper::{camera_rect, EguiBlockInputState}, types::{world_mercator_to_lat_lon, Coord, Location, TileMapResources, TileType}};

pub struct TileMapPlugin;

impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx): (ChunkSenderType, ChunkReceiverType) = bounded(10);
        app.insert_resource(ChunkReceiver(rx))
            .insert_resource(ChunkSender(tx))
            .insert_resource(TileMapResources::default())
            .insert_resource(Clean::default())
            .add_event::<ZoomEvent>()
            .add_systems(FixedUpdate, (spawn_chunks_around_middle, spawn_to_needed_chunks))
            .add_systems(Update, (detect_zoom_level))
            .add_systems(FixedUpdate, (despawn_outofrange_chunks, read_tile_map_receiver, clean_tile_map).chain())
            .insert_resource(ZoomCooldown(Timer::from_seconds(0.2, TimerMode::Repeating)));
    }
}

fn spawn_chunks_around_middle(
    camera_query: Query<&Transform, With<Camera>>,
    chunk_sender: Res<ChunkSender>,
    mut res_manager: ResMut<TileMapResources>,
) {
    // We must find where we need to split the tiles into quaters.
    // We also need to find how to do that.
    // To be fair, i dont really care that much about splitting it i just want to make it so that each time the whole map is gotten then split in half.
    // What we need to do is create a "mock" camera, where we get the 
    if res_manager.chunk_manager.update {
        res_manager.chunk_manager.update = false;

        let chunk_manager_clone = res_manager.chunk_manager.clone();
        let enabled_origins = chunk_manager_clone.get_enabled_tile_web_origins();
        if let Some((url, (_, tile_type))) = enabled_origins {
            let camera_chunk_pos = camera_pos_to_chunk_pos(&res_manager.location_manager.location.to_game_coords(res_manager.chunk_manager.refrence_long_lat, res_manager.zoom_manager.zoom_level, res_manager.zoom_manager.tile_size as f64), res_manager.zoom_manager.tile_size);
            let range = 4;

            for y in (camera_chunk_pos.y - range)..=(camera_chunk_pos.y + range) {
                for x in (camera_chunk_pos.x - range)..=(camera_chunk_pos.x + range) {
                    let chunk_pos = IVec2::new(x, y);
                    if !res_manager.chunk_manager.spawned_chunks.contains(&chunk_pos) {
                        let tx = chunk_sender.clone();
                        let zoom_manager = res_manager.zoom_manager.clone();
                        let refrence_long_lat = res_manager.chunk_manager.refrence_long_lat;
                        let world_pos = chunk_pos_to_world_pos(chunk_pos, zoom_manager.tile_size);
                        let position = world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), refrence_long_lat, res_manager.zoom_manager.zoom_level, zoom_manager.tile_size);
                        let url = url.clone();
                        let tile_type = tile_type.clone();
                        thread::spawn(move || {
                            let tile_coords = position.to_tile_coords(zoom_manager.zoom_level);

                            match tile_type {
                                TileType::Raster => {
                                    let tile_image = get_rasta_data(tile_coords.x as u64, tile_coords.y as u64, zoom_manager.zoom_level as u64, url.to_string());
                                    if let Err(e) = tx.send((chunk_pos, tile_image)) {
                                        error!("Failed to send chunk data: {:?}", e);
                                    }
                                },
                                TileType::Vector => {
                                    let tile_image = get_mvt_data(tile_coords.x as u64, tile_coords.y as u64, zoom_manager.zoom_level as u64, zoom_manager.tile_size as u32, url.to_string());
                                    if let Err(e) = tx.send((chunk_pos, tile_image)) {
                                        error!("Failed to send chunk data: {:?}", e);
                                    }
                                }
                            }
                        });

                        res_manager.chunk_manager.spawned_chunks.insert(chunk_pos);
                    }
                }
            }
        }
    }
}

// Zoom handling //

#[derive(Event)]
struct ZoomEvent();
#[derive(Resource)]
struct ZoomCooldown(pub Timer);

fn detect_zoom_level(
    mut res_manager: ResMut<TileMapResources>,
    mut ortho_projection_query: Query<&mut OrthographicProjection, With<Camera>>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    state: Res<EguiBlockInputState>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    mut cooldown: ResMut<ZoomCooldown>,
    time: Res<Time>,
    mut clean: ResMut<Clean>,
    evr_scroll: EventReader<MouseWheel>,
) {
    if cooldown.0.tick(time.delta()).finished() && !state.block_input {
        if let Ok(mut projection) = ortho_projection_query.get_single_mut() {
            let width = camera_rect(q_windows.single(), projection.clone()).0 / res_manager.zoom_manager.tile_size as f32 / res_manager.zoom_manager.scale.x;
            if width > 6.5 && res_manager.zoom_manager.zoom_level > 3 {
                res_manager.zoom_manager.zoom_level -= 1;
                res_manager.zoom_manager.scale*=2.0;
                res_manager.chunk_manager.refrence_long_lat *= Coord {lat: 2., long: 2.};
            } else if width < 3.5 && res_manager.zoom_manager.zoom_level < 20 {
                res_manager.zoom_manager.scale/=2.0;
                res_manager.zoom_manager.zoom_level += 1;
                res_manager.chunk_manager.refrence_long_lat /= Coord {lat: 2., long: 2.};
            } else {
                return;
            }

            // We need to get the displacement rather than the location.
            if let Ok(mut camera) = camera_query.get_single_mut() {
                
                res_manager.chunk_manager.displacement = (res_manager.location_manager.location
                    .to_game_coords(res_manager.chunk_manager.refrence_long_lat, 14, res_manager.zoom_manager.tile_size.into()).extend(1.0) - camera.translation).xy();
            }
      
            res_manager.zoom_manager.zoom_level_changed = true;
            res_manager.chunk_manager.update = true;
            clean.clean = true;
            cooldown.0.reset(); // Ensure cooldown is reset
        }
    } else {
        res_manager.zoom_manager.zoom_level_changed = false;
    }
    if !evr_scroll.is_empty() {
        cooldown.0.reset(); // Ensure cooldown is reset
    }
    if res_manager.chunk_manager.tile_web_origin_changed {
        res_manager.chunk_manager.tile_web_origin_changed = false;
        res_manager.zoom_manager.zoom_level_changed = true;
        res_manager.chunk_manager.update = true;
        clean.clean = true;
        cooldown.0.reset();
    }
}

// Chunk handling //

type ChunkData = (IVec2, Vec<u8>);
type ChunkSenderType = Sender<ChunkData>;
type ChunkReceiverType = Receiver<ChunkData>;

#[derive(Component)]
struct ChunkPosition(IVec2);

#[derive(Component)]
struct TileMarker;

#[derive(Resource, Deref)]
struct ChunkReceiver(Receiver<(IVec2, Vec<u8>)>); // Use Vec<u8> for raw image data

#[derive(Resource, Deref)]
struct ChunkSender(Sender<(IVec2, Vec<u8>)>);

fn camera_pos_to_chunk_pos(camera_pos: &Vec2, tile_size: f32) -> IVec2 {
    let camera_pos = Vec2::new(camera_pos.x, camera_pos.y) / tile_size;
    camera_pos.floor().as_ivec2()
}


fn chunk_pos_to_world_pos(chunk_pos: IVec2, tile_size: f32) -> Vec2 {
    Vec2::new(
        chunk_pos.x as f32 * tile_size,
        chunk_pos.y as f32 * tile_size,
    )
}

fn read_tile_map_receiver(
    map_receiver: Res<ChunkReceiver>,
    mut res_manager: ResMut<TileMapResources>,
) {
    let mut new_chunks = Vec::new();

    while let Ok((chunk_pos, raw_image_data)) = map_receiver.try_recv() {
        if !res_manager.chunk_manager.to_spawn_chunks.contains_key(&chunk_pos) {
            new_chunks.push((chunk_pos, raw_image_data));
            // new_chunks.push((chunk_pos / res_manager.zoom_manager.scale.xy().as_ivec2(), raw_image_data));
             
        }
    }

    for (pos, data) in new_chunks {
        res_manager.chunk_manager.to_spawn_chunks.insert(pos, data);
    }
}

fn spawn_to_needed_chunks(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut res_manager: ResMut<TileMapResources>,
) {
    let to_spawn_chunks: Vec<(IVec2, Vec<u8>)> = res_manager.chunk_manager.to_spawn_chunks.iter().map(|(pos, data)| (*pos, data.clone())).collect();
    for (chunk_pos, raw_image_data) in to_spawn_chunks {
        let tile_handle = images.add(buffer_to_bevy_image(raw_image_data, res_manager.zoom_manager.tile_size as u32));
        res_manager.zoom_manager.scale.z = 1.0;
        spawn_chunk(&mut commands, tile_handle, chunk_pos, res_manager.zoom_manager.tile_size, res_manager.zoom_manager.scale, res_manager.chunk_manager.displacement);
        res_manager.chunk_manager.spawned_chunks.insert(chunk_pos);
    }
    res_manager.chunk_manager.to_spawn_chunks.clear();
}

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
    tile_size: f32,
    scale: Vec3,
    offset: Vec2,
) {
    let world_x = chunk_pos.x as f32 * tile_size * scale.x - offset.x;
    let world_y = chunk_pos.y as f32 * tile_size * scale.x - offset.y;
    commands.spawn((
        (
            Sprite::from_image(
                tile
            ),
            Transform::from_translation(Vec3::new(
                world_x,
                world_y,
                1.0,
            )).with_scale(scale),
            Visibility::Visible,
        ),
        ChunkPosition(chunk_pos),
        TileMarker,
    ));
}

// Despawn handling //

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<Camera>>,
    chunks_query: Query<(Entity, &Transform, &ChunkPosition)>,
    mut res_manager: ResMut<TileMapResources>,
) {
    /*
    for camera_transform in camera_query.iter() {
        for (entity, chunk_transform, chunk_pos) in chunks_query.iter() {
            let chunk_world_pos = chunk_transform.translation.xy();
            let distance = camera_transform.translation.xy().distance(chunk_world_pos);
            if distance > res_manager.zoom_manager.tile_size * 10.0 {
                res_manager.chunk_manager.spawned_chunks.remove(&chunk_pos.0);
                commands.entity(entity).despawn_recursive();
            }
        }
    }
    */
}

#[derive(Resource, Clone, Default)]
struct Clean {
    clean: bool,
}

fn clean_tile_map(
    mut res_manager: ResMut<TileMapResources>,
    commands: Commands,
    chunk_query: Query<(Entity, &ChunkPosition)>,
    mut clean: ResMut<Clean>,
) {
    if clean.clean {
        clean.clean = false;
        despawn_all_chunks(commands, chunk_query);
        res_manager.chunk_manager.spawned_chunks.clear();
        res_manager.chunk_manager.to_spawn_chunks.clear();
    }
}

fn despawn_all_chunks(
    mut commands: Commands,
    chunks_query: Query<(Entity, &ChunkPosition)>,
) {
    for (entity, _) in chunks_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
