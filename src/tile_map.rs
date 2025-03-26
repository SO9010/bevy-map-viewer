// Ok so this section needs to be rewritten. The current implementation is not optimal and is not working as expected.
// So the idea is to ditch bevy ecs tile map. not only will this make it so we have less imports but it will give us more control.
// So each tile when we want more detail gets put into quaters like this 

/*
┌───┬───┐
│ 0 │ 1 │   
├───┼───┤   →   (2x, 2y), (2x+1, 2y), (2x, 2y+1), (2x+1, 2y+1)
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
use bevy::{input::mouse::MouseWheel, prelude::*, utils::{HashMap, HashSet}, window::PrimaryWindow};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{camera::{camera_helper::{camera_rect, EguiBlockInputState}, camera_system::STARTING_DISPLACEMENT}, api::{buffer_to_bevy_image, get_mvt_data, get_rasta_data}, types::{world_mercator_to_lat_lon, Coord}};

const CHUNK_SIZE: UVec2 = UVec2 { x: 1, y: 1 };

pub struct TileMapPlugin;

impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx): (ChunkSenderType, ChunkReceiverType) = bounded(10);
        app.insert_resource(ChunkReceiver(rx))
            .insert_resource(ChunkSender(tx))
            .insert_resource(TileMapResources::default())
            .insert_resource(Clean::default())
            .add_event::<ZoomEvent>()
            .add_systems(FixedUpdate, (spawn_chunks_around_camera, spawn_to_needed_chunks))
            .add_systems(Update, (detect_zoom_level, zoom_system))
            .add_systems(FixedUpdate, (despawn_outofrange_chunks, read_tile_map_receiver, clean_tile_map).chain())
            .insert_resource(ZoomCooldown(Timer::from_seconds(0.2, TimerMode::Repeating)));
    }
}

#[derive(Debug, Resource, Clone, Default)]
pub struct TileMapResources {
    pub zoom_manager: ZoomManager,
    pub chunk_manager: ChunkManager,
    pub location_manager: Location,
}

#[derive(Component)]
struct ChunkPosition(IVec2);

#[derive(Component)]
struct TileMarker;

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
    tile_size: f32,
    scale: Vec3,
) {
    let world_x = chunk_pos.x as f32 * tile_size;
    let world_y = chunk_pos.y as f32 * tile_size;
    
    info!("Spawning chunk at position: {:?}, world coords: ({}, {}), with scale: {:?}, tile_size: {}", 
          chunk_pos, world_x, world_y, scale, tile_size);
    
    let entity = commands.spawn((
        (
            Sprite::from_image(
                tile
            ),
            Transform::from_translation(Vec3::new(
                world_x,
                world_y,
                1.0,
            )),
            Visibility::Visible,
        ),
        ChunkPosition(chunk_pos),
        TileMarker,
    )).id();
    
    info!("Spawned chunk entity {:?} at position: {:?}", entity, chunk_pos);
}

fn spawn_chunks_around_camera(
    camera_query: Query<&Transform, With<Camera>>,
    chunk_sender: Res<ChunkSender>,
    mut res_manager: ResMut<TileMapResources>,
) {
    if res_manager.chunk_manager.update {
        res_manager.chunk_manager.update = false;

        let chunk_manager_clone = res_manager.chunk_manager.clone();
        let enabled_origins = chunk_manager_clone.get_enabled_tile_web_origins();
        if let Some((url, (_, tile_type))) = enabled_origins {
            for transform in camera_query.iter() {
                info!("camera: {:?}", transform);
                let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation.xy(), res_manager.zoom_manager.tile_size);
                let range = 4;

                for y in (camera_chunk_pos.y - range)..=(camera_chunk_pos.y + range) {
                    for x in (camera_chunk_pos.x - range)..=(camera_chunk_pos.x + range) {
                        let chunk_pos = IVec2::new(x, y);
                        if !res_manager.chunk_manager.spawned_chunks.contains(&chunk_pos) {
                            let tx = chunk_sender.clone();
                            let zoom_manager = res_manager.zoom_manager.clone();
                            let refrence_long_lat = res_manager.chunk_manager.refrence_long_lat;
                            let world_pos = chunk_pos_to_world_pos(chunk_pos, zoom_manager.tile_size);
                            let position = world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), refrence_long_lat, 14, zoom_manager.tile_size);
                            let url = url.clone();
                            let tile_type = tile_type.clone();
                            thread::spawn(move || {
                                let tile_coords = position.to_tile_coords(zoom_manager.zoom_level);

                                match tile_type {
                                    TileType::Raster => {
                                        let tile_image = get_rasta_data(tile_coords.x as u64, tile_coords.y as u64, zoom_manager.zoom_level as u64, url.to_string());
                                        if let Err(e) = tx.send((chunk_pos, tile_image)) {
                                            eprintln!("Failed to send chunk data: {:?}", e);
                                        }
                                    },
                                    TileType::Vector => {
                                        let tile_image = get_mvt_data(tile_coords.x as u64, tile_coords.y as u64, zoom_manager.zoom_level as u64, zoom_manager.tile_size as u32, url.to_string());
                                        if let Err(e) = tx.send((chunk_pos, tile_image)) {
                                            eprintln!("Failed to send chunk data: {:?}", e);
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
}

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

fn clean_tile_map(
    mut res_manager: ResMut<TileMapResources>,
    mut commands: Commands,
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

#[derive(Debug, Clone)]
pub struct ZoomManager {
    pub zoom_level: u32,
    pub last_projection_level: f32,
    pub scale: Vec3,
    pub tile_size: f32,
    zoom_level_changed: bool,
}


impl Default for ZoomManager {
    fn default() -> Self {
        Self {
            zoom_level: 14,
            last_projection_level: 1.0,
            // Default tile size.#
            scale: Vec3::splat(1.0),
            tile_size: 256 as f32,
            zoom_level_changed: false
        }
    }
}

impl ZoomManager {
    pub fn has_changed(&self) -> bool {
        self.zoom_level_changed
    }
}

#[derive(Debug, Clone)]
pub enum TileType {
    Raster,
    Vector
}

#[derive(Debug, Clone)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub to_spawn_chunks: HashMap<IVec2, Vec<u8>>, // Store raw image data
    pub update: bool, // Store raw image data
    pub refrence_long_lat: Coord,
    pub tile_web_origin: HashMap<String, (bool, TileType)>,
    pub tile_web_origin_changed: bool,
}

impl ChunkManager {
    pub fn add_tile_web_origin(&mut self, url: String, enabled: bool, tile_type: TileType) {
        self.tile_web_origin.insert(url, (enabled, tile_type));
    }

    pub fn enable_tile_web_origin(&mut self, url: &str) {
        if let Some((enabled, _)) = self.tile_web_origin.get_mut(url) {
            *enabled = true;
        }
    }
    
    pub fn disable_all_tile_web_origins(&mut self) {
        for (_, (enabled, _)) in self.tile_web_origin.iter_mut() {
            *enabled = false;
        }
    }
    
    pub fn enable_only_tile_web_origin(&mut self, url: &str) {
        self.disable_all_tile_web_origins();
        
        if let Some((enabled, _)) = self.tile_web_origin.get_mut(url) {
            *enabled = true;
            self.tile_web_origin_changed = true;
        }
    }

    pub fn get_enabled_tile_web_origins(&self) -> Option<(&String, (&bool, &TileType))> {
        for (url, (enabled, tile_type)) in &self.tile_web_origin {
            if *enabled {
                return Some((url, (enabled, tile_type)));
            }
        }
        None
    }
}

impl Default for ChunkManager {
    fn default() -> Self {
        let mut tile_web_origin = HashMap::default();
        tile_web_origin.insert("https://tile.openstreetmap.org".to_string(), (false, TileType::Raster));
        tile_web_origin.insert("https://mt1.google.com/vt/lyrs=y".to_string(), (true, TileType::Raster));
        tile_web_origin.insert("https://mt1.google.com/vt/lyrs=m".to_string(), (false, TileType::Raster));
        tile_web_origin.insert("https://mt1.google.com/vt/lyrs=s".to_string(), (false, TileType::Raster));
        tile_web_origin.insert("https://tiles.openfreemap.org/planet/20250122_001001_pt".to_string(), (false, TileType::Vector));
        Self {
            spawned_chunks: HashSet::default(),
            to_spawn_chunks: HashMap::default(),
            update: true,
            // TODO MAKE THIS CONFIGURABLE BY THE DEVELOPER
            refrence_long_lat: Coord { lat: 0.011, long: 0.011 },
            tile_web_origin,
            tile_web_origin_changed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Location {
    pub location: Coord,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            location: STARTING_DISPLACEMENT,
        }
    }
}

#[derive(Resource, Clone, Default)]
struct Clean {
    clean: bool,
}

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
    evr_scroll: EventReader<MouseWheel>,
    mut cooldown: ResMut<ZoomCooldown>,
    time: Res<Time>,
    mut clean: ResMut<Clean>,
) {
    cooldown.0.tick(time.delta());

    if cooldown.0.finished() && !state.block_input && !evr_scroll.is_empty() {
        if let (Ok(mut projection), Ok(mut camera)) = ( ortho_projection_query.get_single_mut(), camera_query.get_single_mut()) {
            let width = camera_rect(q_windows.single(), projection.clone()).0 / res_manager.zoom_manager.tile_size as f32;
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

            res_manager.zoom_manager.zoom_level_changed = true;
            projection.scale = 1.0;

            res_manager.chunk_manager.update = true;
            clean.clean = true;
            cooldown.0.reset();
        }
    } else {
        res_manager.zoom_manager.zoom_level_changed = false;
    }
    if res_manager.chunk_manager.tile_web_origin_changed {
        res_manager.chunk_manager.tile_web_origin_changed = false;
        res_manager.zoom_manager.zoom_level_changed = true;
        res_manager.chunk_manager.update = true;
        clean.clean = true;
        cooldown.0.reset();
    }
}

fn zoom_system(
    mut event_writer: EventWriter<ZoomEvent>,
    mut cooldown: ResMut<ZoomCooldown>,
    time: Res<Time>,
) {
    if cooldown.0.tick(time.delta()).finished() {
        event_writer.send(ZoomEvent());

        cooldown.0.reset();
    }
}


pub type ChunkData = (IVec2, Vec<u8>);
pub type ChunkSenderType = Sender<ChunkData>;
pub type ChunkReceiverType = Receiver<ChunkData>;

#[derive(Resource, Deref)]
pub struct ChunkReceiver(Receiver<(IVec2, Vec<u8>)>); // Use Vec<u8> for raw image data

#[derive(Resource, Deref)]
pub struct ChunkSender(Sender<(IVec2, Vec<u8>)>);

fn camera_pos_to_chunk_pos(camera_pos: &Vec2, tile_size: f32) -> IVec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * tile_size,
        CHUNK_SIZE.y as f32 * tile_size,
    );
    let camera_pos = Vec2::new(camera_pos.x, camera_pos.y) / chunk_size;
    camera_pos.floor().as_ivec2()
}


fn chunk_pos_to_world_pos(chunk_pos: IVec2, tile_size: f32) -> Vec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * tile_size,
        CHUNK_SIZE.y as f32 * tile_size,
    );
    Vec2::new(
        chunk_pos.x as f32 * chunk_size.x,
        chunk_pos.y as f32 * chunk_size.y,
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
        spawn_chunk(&mut commands, tile_handle, chunk_pos, res_manager.zoom_manager.tile_size, res_manager.zoom_manager.scale);
        res_manager.chunk_manager.spawned_chunks.insert(chunk_pos);
    }
    res_manager.chunk_manager.to_spawn_chunks.clear();
}