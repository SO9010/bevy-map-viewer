use bevy::{
    app::{App, Plugin, Startup},
    ecs::{
        component::Component,
        event::{Event, EventWriter},
        resource::Resource,
    },
    math::{IVec2, Vec2, Vec3},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    ops::{AddAssign, DivAssign, MulAssign, SubAssign},
};

use crate::api::TileRequestClient;

#[derive(Component, Debug, Clone)]
pub struct MapViewerMarker;

pub struct InitTileMapPlugin {
    pub starting_location: Coord,
    pub starting_zoom: u32,
    pub starting_url: Option<String>,
    pub tile_quality: f32,
    pub cache_dir: String,
}

impl Plugin for InitTileMapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TileMapResources::new(
            self.starting_location,
            self.starting_zoom,
            self.starting_url.clone(),
            self.tile_quality,
            self.cache_dir.clone(),
        ))
        .add_event::<ZoomChangedEvent>()
        .add_event::<UpdateChunkEvent>()
        .add_systems(Startup, send_initial_events);
    }
}

#[derive(Debug, Resource, Clone, Default)]
pub struct TileMapResources {
    pub zoom_manager: ZoomManager,
    pub chunk_manager: ChunkManager,
    pub location_manager: Location,
    pub tile_request_client: TileRequestClient,
}

impl TileMapResources {
    pub fn new(
        starting_location: Coord,
        zoom: u32,
        starting_url: Option<String>,
        tile_quality: f32,
        cache_dir: String,
    ) -> Self {
        Self {
            zoom_manager: ZoomManager::new(zoom, tile_quality),
            chunk_manager: ChunkManager::new(),
            location_manager: Location::new(starting_location),
            tile_request_client: TileRequestClient::new(cache_dir, starting_url),
        }
    }

    pub fn point_to_coord(&self, point: Vec2) -> Coord {
        game_to_coord(
            point.x,
            point.y,
            self.chunk_manager.refrence_long_lat,
            self.chunk_manager.displacement,
            self.zoom_manager.starting_zoom,
            self.zoom_manager.tile_quality,
        )
    }

    pub fn coord_to_point(&self, coord: Coord) -> Vec2 {
        coord_to_game(
            coord,
            self.chunk_manager.refrence_long_lat,
            self.zoom_manager.starting_zoom,
            self.zoom_manager.tile_quality,
            self.chunk_manager.displacement,
        )
        .into()
    }

    pub fn location_manager_to_point(&self) -> Vec2 {
        self.location_manager
            .location
            .to_game_coords_without_displacement(self.clone())
    }
}

//------------------------------------------------------------------------------
// Basic Types and Structures
//------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldSpaceRect {
    pub top_left: Coord,
    pub bottom_right: Coord,
}

pub fn tile_width_meters(zoom: u32) -> f64 {
    let earth_circumference_meters = 40075016.686;
    let num_tiles = 2_u32.pow(zoom) as f64;
    earth_circumference_meters / num_tiles
}
pub enum DistanceType {
    Km,
    M,
    CM,
}

#[derive(Debug, Clone)]
pub enum TileType {
    Raster,
    Vector,
}

impl std::fmt::Debug for DistanceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceType::Km => write!(f, "Km"),
            DistanceType::M => write!(f, "M"),
            DistanceType::CM => write!(f, "CM"),
        }
    }
}

//------------------------------------------------------------------------------
// Coordinate System and Transformations
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Coord {
    pub lat: f32,
    #[serde(rename = "lon")]
    pub long: f32,
}

impl Coord {
    pub const fn new(lat: f32, long: f32) -> Self {
        Self { lat, long }
    }

    pub fn to_tuple(&self) -> (f32, f32) {
        (self.lat, self.long)
    }

    pub fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.lat, self.long)
    }

    pub fn to_tile_coords(&self, zoom: u32) -> Tile {
        let x = ((self.long + 180.0) / 360.0 * (2_i32.pow(zoom) as f32)).floor() as i32;
        let y = ((1.0
            - (self.lat.to_radians().tan() + 1.0 / self.lat.to_radians().cos()).ln()
                / std::f32::consts::PI)
            / 2.0
            * (2_i32.pow(zoom) as f32))
            .floor() as i32;
        Tile { x, y, zoom }
    }

    pub fn to_mercator(&self) -> Vec2 {
        let lon_rad = self.long.to_radians() as f64;
        let lat_rad = self.lat.to_radians() as f64;
        let x = lon_rad * 20037508.34 / std::f64::consts::PI;
        let y = lat_rad.tan().asinh() * 20037508.34 / std::f64::consts::PI;

        Vec2::new(x as f32, y as f32)
    }

    // https://stackoverflow.com/questions/639695/how-to-convert-latitude-or-longitude-to-meters
    pub fn distance(&self, other: &Coord) -> (f32, DistanceType) {
        let earth_radius_in_km = 6378.137;
        let lat1 = self.lat * PI / 180.0;
        let lat2 = other.lat * PI / 180.0;
        let d_lat = lat2 - lat1;
        let d_lon = (other.long - self.long) * PI / 180.0;

        let a = (d_lat / 2.0).sin() * (d_lat / 2.0).sin()
            + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin() * (d_lon / 2.0).sin();
        let c = 2.0 * ((a).sqrt().atan2((1.0 - a).sqrt()));
        let d = earth_radius_in_km * c;

        if d * 1000. > 999. {
            (d, DistanceType::Km)
        } else {
            (d * 1000., DistanceType::M)
        }
    }

    pub fn to_game_coords_without_displacement(
        &self,
        tile_map_resources: TileMapResources,
    ) -> Vec2 {
        let mut ref_coords = Vec2 { x: 1., y: 1. };
        if tile_map_resources.chunk_manager.refrence_long_lat.lat != 0.0
            && tile_map_resources.chunk_manager.refrence_long_lat.long != 0.0
        {
            ref_coords = tile_map_resources
                .chunk_manager
                .refrence_long_lat
                .to_mercator();
        }

        let meters_per_tile =
            20037508.34 * 2.0 / (2.0_f64.powi(tile_map_resources.zoom_manager.zoom_level as i32)); // At zoom level N
        let scale = meters_per_tile as f32 / tile_map_resources.zoom_manager.tile_quality;

        let x = self.long * 20037508.34 / 180.0;
        let y = (self.lat.to_radians().tan() + 1.0 / self.lat.to_radians().cos()).ln()
            * 20037508.34
            / std::f32::consts::PI;

        let x_offset = (x - ref_coords.x) / scale;
        let y_offset = (y - ref_coords.y) / scale;

        Vec2 {
            x: x_offset,
            y: y_offset,
        }
    }

    // We need to pass the map resources to this function to get the correct scale
    pub fn to_game_coords(&self, tile_map_resources: TileMapResources) -> Vec2 {
        coord_to_game(
            *self,
            tile_map_resources.chunk_manager.refrence_long_lat,
            tile_map_resources.zoom_manager.starting_zoom,
            tile_map_resources.zoom_manager.tile_quality,
            tile_map_resources.chunk_manager.displacement,
        )
        .into()
    }
}

fn send_initial_events(
    mut zoom_event_writer: EventWriter<ZoomChangedEvent>,
    mut update_chunk_writer: EventWriter<UpdateChunkEvent>,
) {
    zoom_event_writer.write(ZoomChangedEvent);

    update_chunk_writer.write(UpdateChunkEvent);
}

//------------------------------------------------------------------------------
// Coordinate Operations Implementation
//------------------------------------------------------------------------------
impl std::ops::Mul for Coord {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Coord {
            lat: self.lat * rhs.lat,
            long: self.long * rhs.long,
        }
    }
}

impl std::ops::Div for Coord {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Coord {
            lat: self.lat / rhs.lat,
            long: self.long / rhs.long,
        }
    }
}

impl std::ops::Sub for Coord {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Coord {
            lat: self.lat - rhs.lat,
            long: self.long - rhs.long,
        }
    }
}

impl std::ops::Add for Coord {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Coord {
            lat: self.lat + rhs.lat,
            long: self.long + rhs.long,
        }
    }
}

impl SubAssign for Coord {
    fn sub_assign(&mut self, rhs: Self) {
        self.lat -= rhs.lat;
        self.long -= rhs.long;
    }
}

impl AddAssign for Coord {
    fn add_assign(&mut self, rhs: Self) {
        self.lat += rhs.lat;
        self.long += rhs.long;
    }
}

impl MulAssign for Coord {
    fn mul_assign(&mut self, rhs: Self) {
        self.lat *= rhs.lat;
        self.long *= rhs.long;
    }
}

impl DivAssign for Coord {
    fn div_assign(&mut self, rhs: Self) {
        self.lat /= rhs.lat;
        self.long /= rhs.long;
    }
}

//------------------------------------------------------------------------------
// Tile System and Conversions
//------------------------------------------------------------------------------
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub zoom: u32,
}

impl Tile {
    pub const fn new(x: i32, y: i32, zoom: u32) -> Self {
        Self { x, y, zoom }
    }

    pub fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }

    pub fn to_lat_long(&self) -> Coord {
        let n = 2.0f32.powi(self.zoom as i32);
        let lon_deg = self.x as f32 / n * 360.0 - 180.0;
        let lat_deg = (PI * (1.0 - 2.0 * self.y as f32 / n))
            .sinh()
            .atan()
            .to_degrees();
        Coord::new(lat_deg, normalize_longitude(lon_deg))
    }

    pub fn to_game_coords(&self, tile_map_resources: TileMapResources) -> Vec2 {
        self.to_lat_long().to_game_coords(tile_map_resources)
    }

    pub fn to_mercator(&self) -> Vec2 {
        self.to_lat_long().to_mercator()
    }
}

//------------------------------------------------------------------------------
// Utility Functions
//------------------------------------------------------------------------------

pub fn coord_to_game(
    coord: Coord,
    reference: Coord,
    zoom: u32,
    quality: f32,
    displacement: Vec2,
) -> (f32, f32) {
    let mercator_coord = coord.to_mercator();
    let reference_mercator = reference.to_mercator();

    let meters_per_tile = 20037508.34 * 2.0 / (2.0_f32.powi(zoom as i32));
    let scale = meters_per_tile / quality;

    // Reverse the calculations from global coordinates to offsets
    let x_offset = ((mercator_coord.x - reference_mercator.x) / scale) - displacement.x;
    let y_offset = ((mercator_coord.y - reference_mercator.y) / scale) - displacement.y;

    (x_offset, y_offset)
}

pub fn game_to_coord(
    x_offset: f32,
    y_offset: f32,
    reference: Coord,
    displacement: Vec2,
    zoom: u32,
    quality: f32,
) -> Coord {
    let refrence = reference.to_mercator();

    let meters_per_tile = 20037508.34 * 2.0 / (2.0_f32.powi(zoom as i32));
    let scale = meters_per_tile / quality;

    let global_x = refrence.x + ((x_offset + displacement.x) * scale);
    let global_y = refrence.y + ((y_offset + displacement.y) * scale);

    let lon = (global_x / 20037508.34) * 180.0;
    let lat = (global_y / 20037508.34 * 180.0).to_radians();
    let lat = 2.0 * lat.exp().atan() - std::f32::consts::FRAC_PI_2;
    let lat = lat.to_degrees();

    Coord::new(lat, normalize_longitude(lon))
}

fn normalize_longitude(lon: f32) -> f32 {
    let mut lon = lon;
    while lon > 180.0 {
        lon -= 360.0;
    }
    while lon < -180.0 {
        lon += 360.0;
    }
    lon
}

//------------------------------------------------------------------------------
// Managers and Resources
//------------------------------------------------------------------------------
#[derive(Event)]
pub struct ZoomChangedEvent;

#[derive(Debug, Clone)]
pub struct ZoomManager {
    pub zoom_level: u32,
    pub scale: Vec3,
    pub tile_quality: f32,
    pub starting_zoom: u32,
}

impl Default for ZoomManager {
    fn default() -> Self {
        Self {
            zoom_level: 14,
            scale: Vec3::splat(1.0),
            tile_quality: 256_f32,
            starting_zoom: 14,
        }
    }
}

impl ZoomManager {
    fn new(zoom: u32, tile_quality: f32) -> Self {
        Self {
            zoom_level: zoom,
            scale: Vec3::splat(1.0),
            tile_quality,
            starting_zoom: zoom,
        }
    }
}

#[derive(Event)]
pub struct UpdateChunkEvent;

#[derive(Debug, Clone)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub to_spawn_chunks: HashMap<IVec2, Vec<u8>>, // Store raw image data
    pub refrence_long_lat: Coord,
    pub displacement: Vec2,
    pub layer_management: Vec<f32>,
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self {
            spawned_chunks: HashSet::default(),
            to_spawn_chunks: HashMap::default(),
            refrence_long_lat: Coord {
                lat: 0.011,
                long: 0.011,
            },
            displacement: Vec2::new(0.0, 0.0),
            layer_management: vec![0.0],
        }
    }
}

impl ChunkManager {
    pub fn new() -> Self {
        Self::default()
    }
}
#[derive(Debug, Clone)]
pub struct Location {
    pub location: Coord,
}

impl Location {
    fn new(coord: Coord) -> Self {
        Self { location: coord }
    }
}
impl Default for Location {
    fn default() -> Self {
        Self {
            location: Coord::new(52.1951, 0.1313),
        }
    }
}
