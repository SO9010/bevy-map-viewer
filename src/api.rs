use std::{fs, io::{BufReader, Cursor, Read}, path::Path, time::Duration};

use bevy::{asset::RenderAssetUsages, image::Image, render::render_resource::{Extent3d, TextureDimension, TextureFormat}, utils::HashMap};
use image::ImageReader;
use mvt_reader::Reader;

use raqote::{AntialiasMode, DrawOptions, DrawTarget, PathBuilder, SolidSource, Source, StrokeStyle};
use ureq::Agent;

use crate::{tile_width_meters, TileType};

#[derive(Debug, Clone)]
pub struct TileRequestClient {
    agent: Agent,
    cache_dir: String,
    pub tile_web_origin: HashMap<String, (bool, TileType)>,
    pub tile_web_origin_changed: bool,
}

impl Default for TileRequestClient {
    fn default() -> Self {
        let mut tile_web_origin = HashMap::default();

        tile_web_origin.insert(
            "https://tile.openstreetmap.org".to_string(),
            (false, TileType::Raster),
        );
        tile_web_origin.insert(
            "https://mt1.google.com/vt/lyrs=y".to_string(),
            (true, TileType::Raster),
        );
        tile_web_origin.insert(
            "https://mt1.google.com/vt/lyrs=m".to_string(),
            (false, TileType::Raster),
        );
        tile_web_origin.insert(
            "https://mt1.google.com/vt/lyrs=s".to_string(),
            (false, TileType::Raster),
        );
        tile_web_origin.insert(
            "https://tiles.openfreemap.org/planet/20250122_001001_pt".to_string(),
            (false, TileType::Vector),
        );
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build();
        let agent: Agent = config.into();
        TileRequestClient { 
            agent,
            // Change this to be in a cache dir
            cache_dir: "cache".to_string(), 
            tile_web_origin,
            tile_web_origin_changed: false,
        }
    }
}

impl TileRequestClient {
    pub fn new(cache_dir: String, url: Option<String>) -> Self {
        let mut me = TileRequestClient::default();
        me.cache_dir = cache_dir;
        if let Some(url) = url {
            if me.tile_web_origin.contains_key(&url) {
                me.enable_tile_web_origin(&url);
            } else {
                me.add_tile_web_origin(url, true, TileType::Raster);
            }        
        }
        me
    }

    pub fn get_tile(&self, x: u64, y: u64, zoom: u64) -> Result<Vec<u8>, image::ImageError> {
        let (_, tile_type) = self.get_enabled_tile_web_origins().unwrap().1;
        let extension = match tile_type {
            TileType::Raster => "png",
            TileType::Vector => "pbf",
        };

        let url = self.get_enabled_tile_web_origins().unwrap().0;
        let cache_dir = format!("{}/{}", self.cache_dir, url);
        let cache_file: String = format!("{}/{}_{}_{}.{}", cache_dir.clone(), zoom, x, y, extension);
        
        // Check if the file exists in the cache
        if Path::new(&cache_file).exists() {
            return match tile_type {
                TileType::Raster => decode_image(fs::read(&cache_file).expect("Failed to read cache file")),
                TileType::Vector => ofm_to_data_image(fs::read(&cache_file).expect("Failed to read cache file").clone(), 256, zoom as u32),
            };
        }
        
        let mut req = format!("{}/{}/{}/{}.{}", url, zoom, x, y, extension);
        if url.contains("google") {
            req = format!("{}&x={x}&y={y}&z={zoom}", url);
        }

        // If not in cache, fetch from the network
        let mut status = 429;
        while status == 429 {
            if let Ok(mut response) = self.agent.get(req.as_str()).call() {
                if response.status() == 200 {
                    let mut reader: BufReader<Box<dyn Read + Send + Sync>> = BufReader::new(Box::new(response.body_mut().as_reader()));
                    let mut bytes = Vec::new();
                    reader.read_to_end(&mut bytes).expect("Failed to read bytes from response");
    
                    // Save to cache
                    fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");
                    fs::write(&cache_file, &bytes).expect("Failed to write cache file");
                    return match tile_type {
                        TileType::Raster => decode_image(bytes),
                        TileType::Vector => ofm_to_data_image(bytes.clone(), 256, zoom as u32),
                    };
                } else if response.status() == 429 {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                } else {
                    status = 0;
                }
            }
        }
        Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to fetch tile: {}", status),
        )))
    }
 
}

impl TileRequestClient {
    pub fn add_tile_web_origin(&mut self, url: String, enabled: bool, tile_type: TileType) {
        self.tile_web_origin_changed = true;
        self.tile_web_origin.insert(url, (enabled, tile_type));
    }

    pub fn enable_tile_web_origin(&mut self, url: &str) {
        if let Some((enabled, _)) = self.tile_web_origin.get_mut(url) {
            self.tile_web_origin_changed = true;
            *enabled = true;
        }
    }

    pub fn disable_all_tile_web_origins(&mut self) {
        for (_, (enabled, _)) in self.tile_web_origin.iter_mut() {
            self.tile_web_origin_changed = true;
            *enabled = false;
        }
    }

    pub fn enable_only_tile_web_origin(&mut self, url: &str) {
        self.disable_all_tile_web_origins();

        if let Some((enabled, _)) = self.tile_web_origin.get_mut(url) {
            *enabled = true;
            // Tell the chunks to upadte completely, 
        }
    }

    pub fn get_enabled_tile_web_origins(&self) -> Option<(String, (bool, TileType))> {
        for (url, (enabled, tile_type)) in self.tile_web_origin.clone() {
            if enabled {
                return Some((url, (enabled, tile_type)));
            }
        }
        None
    }
}

pub fn buffer_to_bevy_image(data: Vec<u8>, tile_quality: u32) -> Image {
    Image::new(
        Extent3d {
            width: tile_quality,
            height: tile_quality,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

// Helper convert png to uncompressed image
fn decode_image(data: Vec<u8>) -> Result<Vec<u8>, image::ImageError> {
    // Failed to decode PNG data: Decoding(DecodingError { format: Exact(Jpeg), underlying: Some("No more bytes") })
    let img = ImageReader::new(Cursor::new(data)).with_guessed_format()?.decode()?;
    let rgba = img.to_rgba8();
    Ok(rgba.to_vec())
}

/// This converts it to an image which is as many meters as the tile width This would be AAAMAAZZZING to multithread
/// It would also be good to add a settings struct to control the colors, perhaps add background images and select what specificlly is rendered.
// What would be good is if we slipt tile tiles into 4 when we start getting a zoom over the amount which cant go in anymore like over zoom = 16
fn ofm_to_data_image(data: Vec<u8>, size: u32, zoom: u32) -> Result<Vec<u8>, image::ImageError> {
    let tile = Reader::new(data).unwrap();
    //let size_multiplyer = TILE_QUALITY as u32 / size ;
    let mut dt = DrawTarget::new(size as i32 , size as i32);

    if cfg!(debug_assertions) {
        let mut pb: PathBuilder = PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(size as f32, 0.0);
        pb.line_to(size as f32, size as f32);
        pb.line_to(0.0, size as f32);
        pb.line_to(0.0, 0.0);
        let path = pb.finish();
    
        let stroke_style = StrokeStyle {
            cap: raqote::LineCap::Round,
            join: raqote::LineJoin::Round,
            width: 1.0,
            miter_limit: 10.0,
            dash_array: vec![5.0, 10.0], // 5 units of dash followed by 3 units of gap
            dash_offset: 0.0, // Start at the beginning of the dash pattern
        };
        dt.stroke(
            &path,
        &Source::Solid(SolidSource {
                r: 0xff,
                g: 0xff,
                b: 0xff,
                a: 0xff,
            }),        
            
            &stroke_style,
            &DrawOptions {
                antialias: AntialiasMode::Gray,
                ..Default::default()
            },
        );
    }
    
    let scale = (size as f32 / tile_width_meters(14.try_into().unwrap()).round() as f32) * 0.597_014_9;
    dt.set_transform(&raqote::Transform::scale(scale, scale));

    // Iterate over layers and features]
    let layer_names = tile.get_layer_names().unwrap();
    for (i, title) in layer_names.into_iter().enumerate() {
        for features in tile.get_features(i).iter() {
            for feature in features {
                let mut pb: PathBuilder = PathBuilder::new();
                match &feature.geometry {
                    geo::Geometry::Point(point) 
                        => {
                            if zoom >= 15 {
                                pb.move_to(point.x(), point.y());
                                pb.line_to(point.x() + 1.0, point.y() + 1.0);
                                pb.line_to(point.x() + 1.0, point.y());
                                pb.line_to(point.x(), point.y() + 1.0)
                            }
                        },
                    geo::Geometry::Line(line) 
                        => {
                            pb.move_to(line.start.x, line.start.y);
                            pb.line_to(line.end.x, line.end.y);
                        },
                    geo::Geometry::LineString(line_string) 
                        => {
                            for (j, line) in line_string.lines().enumerate() {
                                if j == 0 {
                                    pb.move_to(line.start.x, line.start.y);
                                    pb.line_to(line.end.x, line.end.y);
                                } else {
                                    pb.line_to(line.start.x, line.start.y);
                                    pb.line_to(line.end.x, line.end.y);
                                }
                            }
                        },
                    geo::Geometry::Polygon(polygon) 
                        => {
                                for (j, line) in polygon.exterior().0.iter().enumerate() {
                                    if j == 0 {
                                        pb.move_to(line.x, line.y);
                                        pb.line_to(line.x, line.y);
                                    } else {
                                        pb.line_to(line.x, line.y);
                                        pb.line_to(line.x, line.y);
                                    }
                                }
                        },
                    geo::Geometry::MultiPolygon(multi_polygon)
                        => {
                                for polygon in multi_polygon {
                                    for (j, line) in polygon.exterior().0.iter().enumerate() {
                                        if j == 0 {
                                            pb.move_to(line.x, line.y);
                                            pb.line_to(line.x, line.y);
                                        } else {
                                            pb.line_to(line.x, line.y);
                                            pb.line_to(line.x, line.y);
                                        }
                                    }
                                }
                        },
                    geo::Geometry::MultiPoint(multi_point) 
                        => {
                            if zoom >= 15 {
                                for point in multi_point {
                                    pb.move_to(point.x(), point.y());
                                    pb.line_to(point.x() + 1.0, point.y() + 1.0);
                                    pb.line_to(point.x() + 1.0, point.y());
                                    pb.line_to(point.x(), point.y() + 1.0)
                                }
                            }
                        },
                    geo::Geometry::MultiLineString(multi_line_string) 
                        => {
                            for line_string in multi_line_string {
                                for (j, line) in line_string.lines().enumerate() {
                                    if j == 0 {
                                        pb.move_to(line.start.x, line.start.y);
                                        pb.line_to(line.end.x, line.end.y);
                                    } else {
                                        pb.line_to(line.start.x, line.start.y);
                                        pb.line_to(line.end.x, line.end.y);
                                    }
                                }
                            }
                        },
                    geo::Geometry::GeometryCollection(geometry_collection) => {
                        println!("GeometryCollection: {:?}", geometry_collection);
                    },
                    geo::Geometry::Rect(rect) => {
                        println!("Rect: {:?}", rect);
                    },
                    geo::Geometry::Triangle(triangle) => {
                        println!("Triangle: {:?}", triangle);
                    },
                }

                if title == "building" {
                    let path = pb.finish();
                    dt.fill(
                        &path,
                    &Source::Solid(SolidSource {
                            r: 0xff,
                            g: 0xff,
                            b: 0xff,
                            a: 0xff,
                        }),        
                        
                        &DrawOptions {
                            antialias: AntialiasMode::Gray,
                            blend_mode: raqote::BlendMode::SrcOver,
                            alpha: 0.5,
                        },
                    );
                } else if title == "park" {
                    let path = pb.finish();
                    dt.fill(
                        &path,
                    &Source::Solid(SolidSource {
                            r: 0x00,
                            g: 0xff,
                            b: 0x00,
                            a: 0xff,
                        }),        
                        
                        &DrawOptions {
                            antialias: AntialiasMode::Gray,
                            blend_mode: raqote::BlendMode::SrcOver,
                            alpha: 0.5,
                        },
                    );
                } else if title == ("water") {
                    let path = pb.finish();
                    dt.fill(
                        &path,
                        // For some reason red and blue are swapped
                    &Source::Solid(SolidSource {
                            b: 0x00,
                            g: 0x00,
                            r: 0xff,
                            a: 0xff,
                        }),        
                        
                        &DrawOptions {
                            antialias: AntialiasMode::Gray,
                            blend_mode: raqote::BlendMode::SrcOver,
                            alpha: 0.5,
                        },
                    );
                } else if title.contains("water") || title.contains("mountian") || title.contains("land") {

                }
                else {
                    let path = pb.finish();

                    let stroke_style = StrokeStyle {
                        cap: raqote::LineCap::Round,
                        join: raqote::LineJoin::Round,
                        width: 10.,
                        miter_limit: 10.0,
                        dash_array: vec![],
                        dash_offset: 0.0,
                    };
                
                    dt.stroke(
                        &path,
                    &Source::Solid(SolidSource {
                            r: 0xff,
                            g: 0xff,
                            b: 0xff,
                            a: 0xff,
                        }),        
                        
                        &stroke_style,
                        &DrawOptions {
                            antialias: AntialiasMode::Gray,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    Ok(dt.get_data_u8().to_vec())
}