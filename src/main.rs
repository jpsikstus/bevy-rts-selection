use std::f32::consts::PI;

use bevy::{
    input::mouse::MouseButtonInput,
    prelude::{shape::Capsule, *},
    window::close_on_esc,
};
use bevy_mod_raycast::{
    DefaultRaycastingPlugin, RaycastMesh, RaycastMethod, RaycastPluginState, RaycastSource,
    RaycastSystem,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

#[derive(Reflect)]
struct GroundRaycastSet;

#[derive(Reflect)]
struct SelectableRaycastSet;

#[derive(Component)]
struct Selectable {
    radius: f32,
}

#[derive(Component)]
struct Selected;

#[derive(Resource)]
struct UnitMaterials {
    normal: Handle<StandardMaterial>,
    selected: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct BoxSelectionPosition(Option<Vec2>);

#[derive(Event)]
struct BoxSelect {
    start: Vec2,
    end: Vec2,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            DefaultRaycastingPlugin::<GroundRaycastSet>::default(),
            DefaultRaycastingPlugin::<SelectableRaycastSet>::default(),
        ))
        .insert_resource(RaycastPluginState::<GroundRaycastSet>::default())
        .insert_resource(RaycastPluginState::<SelectableRaycastSet>::default())
        .insert_resource(BoxSelectionPosition::default())
        .add_event::<BoxSelect>()
        .add_systems(Startup, setup)
        .add_systems(
            First,
            update_raycast_with_cursor
                .before(RaycastSystem::BuildRays::<GroundRaycastSet>)
                .before(RaycastSystem::BuildRays::<SelectableRaycastSet>),
        )
        .add_systems(
            Update,
            (
                close_on_esc,
                add_raycast_mesh,
                start_mouse_selection,
                end_mouse_selection,
                select_single,
                box_select,
                show_selection_box,
                set_selected_unit_material,
                set_unselected_unit_material,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
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
        RaycastSource::<GroundRaycastSet>::new(),
        RaycastSource::<SelectableRaycastSet>::new(),
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
    commands.spawn((SceneBundle {
        scene: asset_server.load("ground.glb#Scene0"),
        ..default()
    },));

    // Units
    let unit_mesh = meshes.add(Capsule::default().into());
    let large_unit_mesh = meshes.add(
        Capsule {
            radius: 1.0,
            ..default()
        }
        .into(),
    );

    let unit_material = materials.add(Color::GRAY.into());

    commands.insert_resource(UnitMaterials {
        normal: unit_material.clone(),
        selected: materials.add(Color::BLUE.into()),
    });

    for (x, y, z) in [(8.0, 2.2, 0.0), (5.0, 5.0, 8.0), (-2.0, 1.2, 3.0)] {
        commands.spawn((
            PbrBundle {
                transform: Transform::from_xyz(x, y, z),
                mesh: unit_mesh.clone(),
                material: unit_material.clone(),
                ..default()
            },
            Selectable { radius: 0.5 },
            RaycastMesh::<SelectableRaycastSet>::default(),
        ));
    }

    commands.spawn((
        PbrBundle {
            transform: Transform::from_xyz(-6.0, 1.2, 8.0),
            mesh: large_unit_mesh,
            material: unit_material.clone(),
            ..default()
        },
        Selectable { radius: 1.0 },
        RaycastMesh::<SelectableRaycastSet>::default(),
    ));
}

// Update our `RaycastSource` with the current cursor position every frame.
fn update_raycast_with_cursor(
    mut cursor: EventReader<CursorMoved>,
    mut ground_raycast_source: Query<&mut RaycastSource<GroundRaycastSet>>,
    mut selectable_raycast_source: Query<&mut RaycastSource<SelectableRaycastSet>>,
) {
    // Grab the most recent cursor event if it exists:
    let Some(cursor_moved) = cursor.iter().last() else {
        return;
    };

    for mut pick_source in &mut ground_raycast_source {
        pick_source.cast_method = RaycastMethod::Screenspace(cursor_moved.position);
    }

    for mut pick_source in &mut selectable_raycast_source {
        pick_source.cast_method = RaycastMethod::Screenspace(cursor_moved.position);
    }
}

// Add raycast mesh to loaded ground scene
fn add_raycast_mesh(
    mut commands: Commands,
    query: Query<Entity, (Added<Handle<Mesh>>, Without<Selectable>)>,
) {
    for entity in query.iter() {
        commands
            .entity(entity)
            .insert(RaycastMesh::<GroundRaycastSet>::default());
    }
}

fn start_mouse_selection(
    mut mouse_events: EventReader<MouseButtonInput>,
    mut selection: ResMut<BoxSelectionPosition>,
    raycast_source: Query<&RaycastSource<GroundRaycastSet>>,
    selectable_raycast_source: Query<&RaycastSource<SelectableRaycastSet>>,
) {
    if !selectable_raycast_source
        .single()
        .intersections()
        .is_empty()
    {
        return;
    }
    let source = raycast_source.single();
    for event in mouse_events.iter() {
        if event.button == MouseButton::Left && event.state.is_pressed() {
            if let Some((_, hit_data)) = source.intersections().first() {
                let position = Vec2::new(hit_data.position().x, hit_data.position().z);
                selection.0 = Some(position);
            }
        }
    }
}

fn end_mouse_selection(
    mut mouse_events: EventReader<MouseButtonInput>,
    mut selection: ResMut<BoxSelectionPosition>,
    mut selection_events: EventWriter<BoxSelect>,
    raycast_source: Query<&RaycastSource<GroundRaycastSet>>,
) {
    let source = raycast_source.single();
    for event in mouse_events.iter() {
        if event.button == MouseButton::Left && !event.state.is_pressed() && selection.0.is_some() {
            if let Some((_, hit_data)) = source.intersections().first() {
                let start = selection.0.unwrap();
                selection_events.send(BoxSelect {
                    start,
                    end: Vec2::new(hit_data.position().x, hit_data.position().z),
                });
            }
            selection.0 = None;
        }
    }
}

fn select_single(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    raycast_source: Query<&RaycastSource<SelectableRaycastSet>>,
) {
    let source = raycast_source.single();
    if buttons.just_pressed(MouseButton::Left) {
        if let Some((entity, _)) = source.intersections().first() {
            commands.entity(*entity).insert(Selected);
        }
    }
}

fn box_select(
    mut commands: Commands,
    mut selection_events: EventReader<BoxSelect>,
    units: Query<(Entity, &GlobalTransform, &Selectable)>,
) {
    for event in selection_events.iter() {
        for (entity, transform, unit) in units.iter() {
            let position = Vec2::new(transform.translation().x, transform.translation().z);
            let rect = Rect::from_corners(event.start, event.end);

            if rect_circle_intersect(rect, position, unit.radius) {
                commands.entity(entity).insert(Selected);
            } else {
                commands.entity(entity).remove::<Selected>();
            }
        }
    }
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
    raycast_source: Query<&RaycastSource<GroundRaycastSet>>,
) {
    let source = raycast_source.single();
    let Some(start) = selection.0 else {
        return;
    };
    let Some((_, hit_data)) = source.intersections().first() else {
        return;
    };
    let end = Vec2::new(hit_data.position().x, hit_data.position().z);
    let middle = (start + end) / 2.0;
    let size = (start - end).abs();

    gizmos.rect(
        Vec3::new(middle.x, 1.0, middle.y),
        Quat::from_rotation_x(PI / 2.0),
        size,
        Color::YELLOW,
    );
}

fn rect_circle_intersect(rect: Rect, center: Vec2, radius: f32) -> bool {
    let closest_point = center.clamp(rect.min, rect.max);
    let distance = (center - closest_point).length();
    distance < radius
}
