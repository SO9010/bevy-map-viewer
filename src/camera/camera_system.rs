use bevy::{core_pipeline::bloom::Bloom, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_pancam::{DirectionKeys, PanCam, PanCamPlugin};

use crate::types::TileMapResources;

use super::{camera_helper::{absorb_egui_inputs, camera_change, track_camera_position, CameraPosition, CameraTrackingEvent, EguiBlockInputState}, CameraConfig};

pub struct CameraSystemPlugin {
    config: CameraConfig,
}

impl CameraSystemPlugin {
    pub fn new(config: CameraConfig) -> Self {
        CameraSystemPlugin { config }
    }
}

impl Plugin for CameraSystemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraConfig::from(self.config.clone()))
            .add_plugins(EguiPlugin)
            .insert_resource(EguiBlockInputState::default())
            .add_systems(Update, absorb_egui_inputs);

        if self.config.enable_camera {
            app.add_plugins(PanCamPlugin)
                .insert_resource(CameraPosition::default())
                .add_systems(Startup, setup_camera)
                .add_event::<CameraTrackingEvent>()
                .add_systems(Update, (track_camera_position, camera_change));
        }
        
        // If the panning camera is enabled, add the panning system
        if self.config.enable_pancam {
            app.add_systems(Update, handle_pancam);
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
            Camera {
                hdr: true, // HDR is required for the bloom effect
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
                min_scale: f32::NEG_INFINITY,
                max_scale: f32::INFINITY,
                min_x: f32::NEG_INFINITY,
                max_x: f32::INFINITY,
                min_y: f32::NEG_INFINITY,
                max_y: f32::INFINITY,
            },
            Bloom::NATURAL,
        ));
    } else {
        error!("TileMapResources not found. Please add the tilemap addon first.");
    }
}

fn handle_pancam(
    mut query: Query<&mut PanCam>, 
    config: Res<CameraConfig>,
    // state: Res<EguiBlockInputState>,
) {
    for mut pancam in &mut query {
        if config.enable_pancam {
            //if state.is_changed() {
            //         pancam.enabled = !state.block_input;
            //}
        } else if pancam.enabled {
            pancam.enabled = false;
        }
    }

}