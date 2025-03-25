use bevy::prelude::*;

use super::{CameraConfig, CameraSystemPlugin};
pub struct CameraPlugin;

impl CameraPlugin {
    pub fn new(config: CameraConfig) -> CameraSystemPlugin {
        CameraSystemPlugin::new(config)
    }
}

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CameraSystemPlugin::new(CameraConfig::default()));
    }
}