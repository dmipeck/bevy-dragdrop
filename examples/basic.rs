use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy_dragdrop::*;

const CARD_SIZE: Vec2 = Vec2::new(100.0, 100.0);

const RED: Color = Color::srgba(0.8, 0.2, 0.2, 0.4);
const BLUE: Color = Color::srgba(0.2, 0.2, 0.8, 0.4);
const YELLOW: Color = Color::srgba(0.8, 0.8, 0.2, 0.4);
const WHITE: Color = Color::srgba(1.0, 1.0, 1.0, 0.4);

fn startup_system(mut cmd: Commands) {
    cmd.spawn(Camera2d::default());

    cmd.spawn((
        Targetable,
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        Sprite {
            color: RED,
            custom_size: Some(CARD_SIZE),
            ..default()
        },
        Transform::from_xyz(-200.0, -200.0, 2.0),
    ))
    .observe(candidate_highlight_observer)
    .observe(candidate_restore_observer)
    .observe(target_highlight_observer)
    .observe(target_restore_observer);

    cmd.spawn((
        Targetable,
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        Sprite {
            color: RED,
            custom_size: Some(CARD_SIZE),
            ..default()
        },
        Transform::from_xyz(-50.0, -50.0, 2.0),
    ))
    .observe(candidate_highlight_observer)
    .observe(candidate_restore_observer)
    .observe(target_highlight_observer)
    .observe(target_restore_observer);

    let candidate_selector_id = cmd.register_system(all_targetables_selector);
    let target_selector_id = cmd.register_system(closest_targetable_selector);

    cmd.spawn((
        Draggable,
        CandidateSelector(candidate_selector_id),
        TargetSelector(target_selector_id),
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        Sprite {
            color: BLUE,
            custom_size: Some(CARD_SIZE),
            ..default()
        },
        Transform::from_xyz(200.0, 200.0, 1.0),
    ))
    .observe(on_drop);
}

#[derive(Component)]
pub struct OriginalSpriteColor(pub Color);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DragDropPlugin)
        .add_systems(Startup, startup_system)
        .run();
}

fn candidate_highlight_observer(
    message: On<CandidateFound>,
    mut sprites: Query<&mut Sprite>,
    mut commands: Commands,
) {
    let Ok(mut sprite) = sprites.get_mut(message.entity) else {
        debug!("{:?} is not a sprite", message.entity);
        return;
    };

    commands
        .entity(message.entity)
        .insert(OriginalSpriteColor(sprite.color));

    sprite.color = YELLOW;
}

fn candidate_restore_observer(
    message: On<CandidateLost>,
    mut sprites: Query<(&mut Sprite, &OriginalSpriteColor)>,
    mut commands: Commands,
) {
    let Ok((mut sprite, original)) = sprites.get_mut(message.entity) else {
        debug!("{:?} is not a sprite", message.entity);
        return;
    };

    sprite.color = original.0;

    commands
        .entity(message.entity)
        .remove::<OriginalSpriteColor>();
}

fn target_highlight_observer(message: On<TargetFound>, mut sprites: Query<&mut Sprite>) {
    let Ok(mut sprite) = sprites.get_mut(message.entity) else {
        debug!("{:?} is not a sprite", message.entity);
        return;
    };

    sprite.color = WHITE;
}

fn target_restore_observer(message: On<TargetLost>, mut sprites: Query<&mut Sprite>) {
    let Ok(mut sprite) = sprites.get_mut(message.entity) else {
        debug!("{:?} is not a sprite", message.entity);
        return;
    };

    sprite.color = YELLOW;
}

fn on_drop(
    event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    draggables: Query<&Targets, With<Draggable>>,
) {
    let Ok(targets) = draggables.get(event.entity) else {
        warn!("Failed to get candidates for entity: {:?}", event.entity);
        return;
    };

    if targets.0.is_empty() {
        debug!("no targets");
        return;
    }

    for t in targets.0.iter() {
        commands.entity(*t).despawn();
    }

    commands.entity(event.entity).despawn();
}
