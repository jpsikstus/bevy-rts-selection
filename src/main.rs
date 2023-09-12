use std::f32::consts::PI;

use bevy::{
    prelude::{shape::Capsule, *},
    window::{close_on_esc, PrimaryWindow},
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use parry3d::{
    na::{Isometry3, Point3},
    shape::ConvexPolyhedron,
};

#[derive(Resource)]
struct UnitMaterials {
    normal: Handle<StandardMaterial>,
    selected: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct BoxSelectionPosition(Option<Vec2>);

#[derive(Event)]
struct BoxSelect(Rect);

#[derive(Component)]
struct Selectable {
    size: Vec3,
}

#[derive(Component)]
struct Selected;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PanOrbitCameraPlugin))
        .insert_resource(BoxSelectionPosition::default())
        .add_event::<BoxSelect>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                close_on_esc,
                start_mouse_selection,
                end_mouse_selection,
                box_select,
                show_selection_box,
                set_selected_unit_material,
                set_unselected_unit_material,
                show_debug_shapes,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 20.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        PanOrbitCamera {
            button_pan: MouseButton::Middle,
            button_orbit: MouseButton::Right,
            ..default()
        },
    ));

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_rotation_x(-PI / 2.0)),
        directional_light: DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        ..default()
    });

    // Ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane {
            size: 100.0,
            ..default()
        })),
        material: materials.add(Color::DARK_GREEN.into()),
        ..default()
    });

    // Units
    let unit_material = materials.add(Color::GRAY.into());

    commands.insert_resource(UnitMaterials {
        normal: unit_material.clone(),
        selected: materials.add(Color::BLUE.into()),
    });

    let unit_mesh = Mesh::from(Capsule::default());
    let unit_aabb = unit_mesh.compute_aabb().unwrap();

    let unit_mesh = meshes.add(unit_mesh);

    let large_unit_mesh = Mesh::from(Capsule {
        radius: 1.0,
        ..default()
    });
    let large_unit_aabb = large_unit_mesh.compute_aabb().unwrap();

    let large_unit_mesh = meshes.add(large_unit_mesh);

    for (x, y, z) in [(8.0, 2.2, 0.0), (5.0, 5.0, 8.0), (-2.0, 1.2, 3.0)] {
        commands.spawn((
            PbrBundle {
                transform: Transform::from_xyz(x, y, z),
                mesh: unit_mesh.clone(),
                material: unit_material.clone(),
                ..default()
            },
            Selectable {
                size: unit_aabb.half_extents.into(),
            },
        ));
    }

    commands.spawn((
        PbrBundle {
            transform: Transform::from_xyz(-6.0, 1.2, 8.0),
            mesh: large_unit_mesh,
            material: unit_material.clone(),
            ..default()
        },
        Selectable {
            size: large_unit_aabb.half_extents.into(),
        },
    ));
}

fn start_mouse_selection(
    buttons: Res<Input<MouseButton>>,
    mut selection: ResMut<BoxSelectionPosition>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        selection.0 = window.single().cursor_position();
    }
}

fn end_mouse_selection(
    mut selection_events: EventWriter<BoxSelect>,
    buttons: Res<Input<MouseButton>>,
    mut selection: ResMut<BoxSelectionPosition>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    if buttons.just_released(MouseButton::Left) {
        if let Some(selection_position) = selection.0 {
            if let Some(cursor_position) = window.single().cursor_position() {
                let rect = Rect::from_corners(cursor_position, selection_position);
                if rect.size().x > 0.0 && rect.size().y > 0.0 {
                    selection_events.send(BoxSelect(rect));
                }
            }
            selection.0 = None;
        }
    }
}

fn box_select(
    mut commands: Commands,
    mut selection_events: EventReader<BoxSelect>,
    camera: Query<(&Camera, &GlobalTransform)>,
    selectables: Query<(Entity, &GlobalTransform, &Selectable)>,
    selected: Query<Entity, With<Selected>>,
) {
    for event in selection_events.iter() {
        for entity in selected.iter() {
            commands.entity(entity).remove::<Selected>();
        }

        let (camera, camera_transform) = camera.single();

        let rays = [
            event.0.min,
            Vec2::new(event.0.min.x, event.0.max.y),
            event.0.max,
            Vec2::new(event.0.max.x, event.0.min.y),
        ]
        .map(|point| camera.viewport_to_world(camera_transform, point).unwrap());

        let near = rays.map(|ray| ray.origin);
        let far = rays.map(|ray| ray.origin + ray.direction * 1000.0);

        let collider = generate_selection_collider(near, far);

        for (entity, transform, selectable) in selectables.iter() {
            let selectable_collider = aabb_collider(selectable.size);
            let selectable_position = vec3_to_isometry(transform.translation());
            let hit = parry3d::query::intersection_test(
                &selectable_position,
                &selectable_collider,
                &Isometry3::identity(),
                &collider,
            );

            if let Ok(true) = hit {
                commands.entity(entity).insert(Selected);
            }
        }
    }
}

fn generate_selection_collider(near: [Vec3; 4], far: [Vec3; 4]) -> ConvexPolyhedron {
    let points = near
        .iter()
        .chain(far.iter())
        .map(|point| Point3::new(point.x, point.y, point.z))
        .collect::<Vec<_>>();

    ConvexPolyhedron::from_convex_hull(&points).unwrap()
}

fn aabb_collider(size: Vec3) -> parry3d::shape::Cuboid {
    parry3d::shape::Cuboid::new([size.x, size.y, size.z].into())
}

fn vec3_to_isometry(position: Vec3) -> Isometry3<parry3d::math::Real> {
    Isometry3::new(
        [position.x, position.y, position.z].into(),
        [0.0, 0.0, 0.0].into(),
    )
}

fn set_selected_unit_material(
    unit_materials: Res<UnitMaterials>,
    mut units: Query<&mut Handle<StandardMaterial>, Added<Selected>>,
) {
    for mut material in units.iter_mut() {
        *material = unit_materials.selected.clone()
    }
}

fn set_unselected_unit_material(
    unit_materials: Res<UnitMaterials>,
    mut materials: Query<&mut Handle<StandardMaterial>>,
    mut removed: RemovedComponents<Selected>,
) {
    for entity in removed.iter() {
        if let Ok(mut material) = materials.get_mut(entity) {
            *material = unit_materials.normal.clone();
        }
    }
}

fn show_selection_box(
    mut gizmos: Gizmos,
    selection: Res<BoxSelectionPosition>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    if let Some(selection_position) = selection.0 {
        if let Some(cursor_position) = window.single().cursor_position() {
            let rect = Rect::from_corners(selection_position, cursor_position);
            let (camera, camera_transform) = camera.single();

            let points = [
                rect.min,
                Vec2::new(rect.min.x, rect.max.y),
                rect.max,
                Vec2::new(rect.max.x, rect.min.y),
                rect.min,
            ]
            .map(|point| {
                camera
                    .viewport_to_world(camera_transform, point)
                    .unwrap()
                    .origin
                    + camera_transform.forward() * 0.0001
            });

            gizmos.linestrip(points, Color::YELLOW);
        }
    }
}

fn show_debug_shapes(selectables: Query<(&GlobalTransform, &Selectable)>, mut gizmos: Gizmos) {
    for (transform, selectable) in selectables.iter() {
        let position = transform.translation();
        let offset_x = Vec3::X * selectable.size.x;
        let offset_y = Vec3::Y * selectable.size.y;
        let offset_z = Vec3::Z * selectable.size.z;
        gizmos.linestrip(
            [
                position + offset_x + offset_y + offset_z,
                position + offset_x + offset_y - offset_z,
                position + offset_x - offset_y - offset_z,
                position + offset_x - offset_y + offset_z,
                position + offset_x + offset_y + offset_z,
                position - offset_x + offset_y + offset_z,
            ],
            Color::GREEN,
        );
        gizmos.linestrip(
            [
                position - offset_x - offset_y - offset_z,
                position - offset_x - offset_y + offset_z,
                position - offset_x + offset_y + offset_z,
                position - offset_x + offset_y - offset_z,
                position - offset_x - offset_y - offset_z,
                position + offset_x - offset_y - offset_z,
            ],
            Color::GREEN,
        );

        gizmos.line(
            position + offset_x + offset_y - offset_z,
            position - offset_x + offset_y - offset_z,
            Color::GREEN,
        );
        gizmos.line(
            position + offset_x - offset_y + offset_z,
            position - offset_x - offset_y + offset_z,
            Color::GREEN,
        );
    }
}
