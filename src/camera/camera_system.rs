use bevy::prelude::*;
#[cfg(feature = "ui_blocking")]
use super::camera_helper::{absorb_egui_inputs, EguiBlockInputState};
use super::camera_helper::{camera_change, track_camera_position, CameraTrackingEvent};

pub struct CameraSystemPlugin;

impl Plugin for CameraSystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CameraTrackingEvent>()
            .add_systems(Update, (track_camera_position, camera_change));
        #[cfg(feature = "ui_blocking")]
        app.insert_resource(EguiBlockInputState::default())
            .add_systems(Update, absorb_egui_inputs);
    }
}

