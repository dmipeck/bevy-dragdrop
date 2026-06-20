use bevy::{platform::collections::HashMap, prelude::*};
use bevy_ecs::{
    entity::EntityHashSet,
    relationship::RelationshipSourceCollection,
    system::{QueryLens, SystemId, SystemState},
};

pub struct DragDropPlugin;

impl Plugin for DragDropPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CandidateFound>()
            .add_message::<CandidateLost>()
            .add_message::<TargetFound>()
            .add_message::<TargetLost>()
            .add_systems(
                Update,
                (
                    candidate_leave_system,
                    candidate_enter_system,
                    target_leave_system,
                    target_enter_system,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (drag_start_system, drag_system, drag_end_system).chain(),
            );
    }
}

#[derive(Component, Default)]
/// Mark the entity as draggable
#[require(Transform, Pickable, Candidates, Targets)]
pub struct Draggable;

#[derive(Component)]
/// Marks the entity as being drag and contains dragging state
#[require(Draggable)]
pub struct Dragging {
    pub origin: Vec3,
}

#[derive(Message, EntityEvent, Clone, Copy)]
/// Tiggered on entites being considered as the target of a drag-drop
pub struct TargetFound {
    pub entity: Entity,
    pub dragging: Entity,
}

/// Triggered on entities no longer being considered as the target of a drag-drop
#[derive(Message, EntityEvent, Clone, Copy)]
pub struct TargetLost {
    pub entity: Entity,
    pub dragging: Entity,
}

#[derive(Message, EntityEvent, Clone, Copy)]
/// Triggered on entities eligible for targeting when dragging starts
pub struct CandidateFound {
    pub entity: Entity,
    pub dragging: Entity,
}

/// Triggered on all entites are no longer being considered as eligible for targeting when dragging stops
#[derive(Message, EntityEvent, Clone, Copy)]
pub struct CandidateLost {
    pub entity: Entity,
    pub dragging: Entity,
}

#[derive(Component)]
/// Marks an entity as targetable by drag-drop operations
pub struct Targetable;

type SelectorSystemId = SystemId<In<Entity>, Vec<Entity>>;

#[derive(Component)]
/// A callback defining what entities a draggable may consider for targeting when dropped
pub struct CandidateSelector(pub SelectorSystemId);

#[derive(Component, Debug, Default)]
/// A store of candidate entities being considered as drop targets
pub struct Candidates(pub Vec<Entity>);

#[derive(Component)]
/// A callback defining what entities a draggable intends to target when dropped.
pub struct TargetSelector(pub SelectorSystemId);

#[derive(Component, Debug, Default)]
/// A store of selecte target entities for when the draggable is dropped
pub struct Targets(pub Vec<Entity>);

/// A selector for all targetables
pub fn all_targetables_selector(
    _: In<Entity>,
    targetables: Query<Entity, With<Targetable>>,
) -> Vec<Entity> {
    return targetables.iter().collect();
}

/// A selector for the closest targetable
pub fn closest_targetable_selector(
    In(entity): In<Entity>,
    mut reader: MessageReader<Pointer<DragEnter>>,
    mut transforms: Query<&Transform>,
    mut targetables: Query<Entity, With<Targetable>>,
) -> Vec<Entity> {
    let entered: EntityHashSet = reader
        .read()
        .filter(|m| m.dragged == entity)
        .map(|m| m.entity)
        .collect();

    let entity_pos = transforms
        .get(entity)
        .map(|t| t.translation)
        .expect("missing transform");

    let mut candidates: QueryLens<(Entity, &Transform), With<Targetable>> =
        targetables.join_filtered(&mut transforms);

    let q = candidates.query();

    let closest_target = q.iter_many(entered).min_by(|(_, trans_a), (_, trans_b)| {
        let dist_a = entity_pos.distance_squared(trans_a.translation);
        let dist_b = entity_pos.distance_squared(trans_b.translation);
        dist_a
            .partial_cmp(&dist_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    return closest_target.map(|(e, _)| e).into_iter().collect();
}

fn run_selectors(
    selectors: Vec<(Entity, SelectorSystemId)>,
    world: &mut World,
) -> HashMap<Entity, Vec<Entity>> {
    selectors
        .iter()
        .map(|(e, selector)| {
            let candidates = world
                .run_system_with(*selector, *e)
                .expect("failed to run selector system");
            (*e, candidates)
        })
        .collect()
}

fn candidate_enter_system(
    world: &mut World,
    read_params: &mut SystemState<(
        MessageReader<Pointer<DragStart>>,
        Query<(Entity, &CandidateSelector), With<Draggable>>,
    )>,
    write_params: &mut SystemState<(
        Commands,
        Query<&mut Candidates>,
        MessageWriter<CandidateFound>,
    )>,
) {
    let selectors = {
        let (reader, draggables) = read_params.get(world);
        collect_candidate_selectors(reader, draggables)
    };

    let candidates = run_selectors(selectors, world);

    {
        let (commands, draggables, writer) = write_params.get_mut(world);
        write_candidates(candidates, commands, draggables, writer);
        write_params.apply(world);
    }
}

fn collect_candidate_selectors(
    mut events: MessageReader<Pointer<DragStart>>,
    draggables: Query<(Entity, &CandidateSelector), With<Draggable>>,
) -> Vec<(Entity, SelectorSystemId)> {
    events
        .read()
        .filter_map(|e| {
            draggables
                .get(e.entity)
                .map(|(e, selector)| (e, selector.0))
                .ok()
        })
        .collect()
}

fn write_candidates(
    candidates: HashMap<Entity, Vec<Entity>>,
    mut commands: Commands,
    mut draggables: Query<&mut Candidates>,
    mut writer: MessageWriter<CandidateFound>,
) {
    for (e, candidate_vec) in candidates.iter() {
        let Ok(mut candidates) = draggables.get_mut(*e) else {
            debug!("{:?} not a draggable", e);
            continue;
        };

        candidates.0 = candidate_vec.to_vec();

        for c in candidate_vec {
            let event = CandidateFound {
                dragging: *e,
                entity: *c,
            };
            commands.trigger(event.clone());
            writer.write(event);
        }
    }
}

fn candidate_leave_system(
    mut commands: Commands,
    mut drag_ends: MessageReader<Pointer<DragEnd>>,
    mut candidate_leaves: MessageWriter<CandidateLost>,
    mut draggables: Query<(Entity, &mut Candidates), With<Draggable>>,
) {
    for event in drag_ends.read() {
        let Ok((draggable, mut candidates)) = draggables.get_mut(event.entity) else {
            debug!("{:?} is not a draggable", event.entity);
            continue;
        };

        for candidate in candidates.0.iter() {
            let event = CandidateLost {
                dragging: draggable,
                entity: candidate,
            };
            commands.trigger(event.clone());
            candidate_leaves.write(event);
        }

        candidates.0.clear();
    }
}

fn target_enter_system(
    world: &mut World,
    read_params: &mut SystemState<(
        MessageReader<Pointer<DragEnter>>,
        Query<(Entity, &TargetSelector), With<Draggable>>,
    )>,
    write_params: &mut SystemState<(Commands, Query<&mut Targets>, MessageWriter<TargetFound>)>,
) {
    let selectors = {
        let (reader, draggables) = read_params.get(world);
        collect_target_selectors(reader, draggables)
    };

    let targets = run_selectors(selectors, world);

    {
        let (commands, draggables, writer) = write_params.get_mut(world);
        write_targets(targets, commands, draggables, writer);
        write_params.apply(world);
    }
}

fn collect_target_selectors(
    mut events: MessageReader<Pointer<DragEnter>>,
    draggables: Query<(Entity, &TargetSelector), With<Draggable>>,
) -> Vec<(Entity, SelectorSystemId)> {
    events
        .read()
        .filter_map(|e| {
            draggables
                .get(e.dragged)
                .map(|(e, selector)| (e, selector.0))
                .ok()
        })
        .collect()
}

fn write_targets(
    selected: HashMap<Entity, Vec<Entity>>,
    mut commands: Commands,
    mut draggables: Query<&mut Targets>,
    mut writer: MessageWriter<TargetFound>,
) {
    for (e, target_vec) in selected.iter() {
        let Ok(mut targets) = draggables.get_mut(*e) else {
            debug!("{:?} not a draggable", e);
            continue;
        };

        targets.0 = target_vec.to_vec();

        for t in target_vec {
            let event = TargetFound {
                dragging: *e,
                entity: *t,
            };

            commands.trigger(event.clone());
            writer.write(event);
        }
    }
}

fn target_leave_system(
    mut commands: Commands,
    mut drag_leaves: MessageReader<Pointer<DragLeave>>,
    mut target_leaves: MessageWriter<TargetLost>,
    mut draggables: Query<(Entity, &mut Targets)>,
    targetables: Query<Entity, With<Targetable>>,
) {
    for event in drag_leaves.read() {
        let Ok((draggable, mut targets)) = draggables.get_mut(event.dragged) else {
            debug!("{:?} is not a draggable", event.dragged);
            continue;
        };

        let Ok(target) = targetables.get(event.entity) else {
            debug!("{:?} is not a targetable", event.entity);
            continue;
        };

        let Some(pos) = targets.0.iter().position(|x| x == target) else {
            error!(
                "can't remove target {:?} from targets {:?}",
                target, targets.0
            );
            continue;
        };

        targets.0.remove(pos);

        let event = TargetLost {
            dragging: draggable,
            entity: target,
        };
        commands.trigger(event.clone());
        target_leaves.write(event);
    }
}

fn drag_start_system(
    mut commands: Commands,
    mut starts: MessageReader<Pointer<DragStart>>,
    draggables: Query<&Transform, With<Draggable>>,
) {
    for event in starts.read() {
        let Ok(transform) = draggables.get(event.entity) else {
            continue;
        };

        commands.entity(event.entity).insert(Dragging {
            origin: transform.translation,
        });
    }
}

fn drag_system(
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut draggables: Query<&mut Transform, With<Dragging>>,
    mut events: MessageReader<Pointer<Drag>>,
) {
    let (camera, camera_transform) = camera.into_inner();

    for event in events.read() {
        let Ok(mut transform) = draggables.get_mut(event.entity) else {
            continue;
        };

        let Ok(world_pos) =
            camera.viewport_to_world_2d(camera_transform, event.pointer_location.position)
        else {
            continue;
        };

        transform.translation = Vec3::new(world_pos.x, world_pos.y, transform.translation.z);
    }
}

fn drag_end_system(
    mut draggables: Query<(&mut Transform, &Dragging)>,
    mut events: MessageReader<Pointer<DragEnd>>,
) {
    for event in events.read() {
        let Ok((mut transform, dragging)) = draggables.get_mut(event.entity) else {
            continue;
        };

        transform.translation = dragging.origin;
    }
}
