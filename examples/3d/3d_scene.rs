//! A simple 3D scene with light shining over a cube sitting on a plane.

use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::{prelude::*, window::WindowResized};
use bevy_render::camera::{MainPassResolutionScale, Viewport};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FpsOverlayPlugin::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, set_viewport)
        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    // camera
    commands.spawn((
        Camera {
            hdr: true,
            ..default()
        },
        Camera3d::default(),
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
        MainPassResolutionScale(0.5),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn set_viewport(mut camera: Single<&mut Camera>, mut events: EventReader<WindowResized>) {
    if let Some(event) = events.read().last() {
        camera.viewport = Some(Viewport {
            physical_size: UVec2::new((event.width) as u32, (event.height) as u32),
            ..default()
        });
    }
}
