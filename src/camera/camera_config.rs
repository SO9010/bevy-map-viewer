use bevy::ecs::system::Resource;

#[derive(Resource, Clone)]
pub struct CameraConfig {
    pub enable_camera: bool,
    pub enable_pancam: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            enable_camera: true,
            enable_pancam: true,
        }
    }
}
