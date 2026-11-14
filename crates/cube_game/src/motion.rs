//! Creatures in motion. The moving is the simulation's (`cube_sim::life`): every creature in
//! the game has an agent there, stepped at the fixed tick. This file keeps the two in step:
//! it enrols creatures as agents, says who drives each (the keys, a wander, its own mind, or
//! nobody), and each frame draws the agent between ticks with the clip for what it is doing.

use crate::{
    creatures::{Creature, Registry, Wander},
    player::Controlled,
    world::Terrain,
};
use bevy::prelude::*;
use cube_sim::life::{Act, Agent, Driver, Kind, Sim};
pub use cube_sim::life::Gait;

pub struct MotionPlugin;

/// Draw the shape each creature is bumped as (`--bodies`, or the panel).
#[derive(Resource, Default)]
pub struct ShowBodies(pub bool);

/// The simulation of everything alive.
#[derive(Resource)]
pub struct Live(pub Sim);

impl Live {
    pub fn agent(&self, mover: &Mover) -> Option<&Agent> {
        self.0.agents.get(&mover.id)
    }
    pub fn agent_mut(&mut self, mover: &Mover) -> Option<&mut Agent> {
        self.0.agents.get_mut(&mover.id)
    }
    pub fn kind(&self, mover: &Mover) -> Option<&Kind> {
        self.agent(mover).map(|a| &self.0.kinds[a.kind])
    }
}

/// On every creature: its agent in the simulation, and what only the drawing needs.
#[derive(Component)]
pub struct Mover {
    pub id: u32,
    /// How far the drawn model still is from the body after a step up or down, eased out.
    lift: f32,
    /// How high its feet were last drawn while it stood on the ground, if it did.
    shown: Option<f32>,
    /// How fast that is rising, metres a second: it comes to a new level as on a spring.
    rising: f32,
    /// The model tipped into its turns and its starts and stops: roll and pitch, radians.
    tilt: Vec2,
    was_speed: f32,
}

impl Mover {
    pub fn new(id: u32) -> Self {
        Self { id, lift: 0.0, shown: None, rising: 0.0, tilt: Vec2::ZERO, was_speed: 0.0 }
    }
}

/// On a creature that lives by its own mind.
#[derive(Component)]
pub struct Wild;

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<crate::Args>().clone();
        let kinds = Kind::load(&std::path::Path::new("assets").join("creatures"));
        app.insert_resource(Time::<Fixed>::from_hz(cube_sim::TICKS as f64))
            .insert_resource(ShowBodies(args.bodies))
            .insert_resource(Live({
                let mut sim = Sim::new(kinds, args.seed as u64);
                // `--day` and `--hour` set the calendar: the day picks the season and the moon.
                if args.day.is_some() || args.hour.is_some() {
                    sim.when = cube_sim::sky::at(args.day.unwrap_or(sim.when as u32), args.hour.unwrap_or(cube_sim::sky::hour(sim.when)));
                }
                sim
            }))
            .add_systems(FixedUpdate, tick)
            .add_systems(Update, (populate, mirror, enrol, drivers, show, far_off, dens).chain())
            .add_systems(Update, bodies)
            .add_systems(PostUpdate, watch.before(bevy::transform::TransformSystems::Propagate));
    }
}

/// `--wild`: once the world exists, its creatures are put in it, each with its own mind.
fn populate(mut commands: Commands, args: Res<crate::Args>, registry: Res<Registry>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, mut done: Local<bool>) {
    let Some(terrain) = terrain.filter(|_| args.wild && !*done) else { return };
    *done = true;
    live.0.populate(&terrain.0, &terrain.1);
    let _ = (&mut commands, &registry);
}

/// The wild creatures that are drawn are the ones being lived: one that wakes (the player
/// has come near) gets its model, one that sleeps or is gone loses it. The island holds
/// thousands; a couple of hundred are ever on screen.
fn mirror(mut commands: Commands, registry: Res<Registry>, mut live: ResMut<Live>, cameras: Query<&GlobalTransform, With<Camera3d>>, controlled: Query<&Mover, With<Controlled>>, wild: Query<(Entity, &Mover), With<Wild>>, mut stepped: Local<u64>) {
    let _slow = crate::Slow::new("mirror");
    // Whoever is watching: the creature being played, or the camera.
    let watcher = controlled.iter().next().and_then(|m| live.agent(m)).map(|a| a.body.pos).or(cameras.iter().next().map(|c| c.translation()));
    // (Until the simulation has been told where the watcher is, and taken a step knowing it,
    // every creature on the island counts as awake: the first frame gave all 1,890 a model
    // and the second took 1,800 away again. `watch` counted them, 2026-11-13.)
    let told = live.0.focus.is_some() && *stepped != live.0.tick;
    if live.0.focus.is_none() {
        *stepped = live.0.tick;
    }
    live.0.focus = watcher;
    if !told {
        return;
    }
    let mut drawn = std::collections::HashSet::new();
    for (entity, mover) in &wild {
        if live.agent(mover).is_some_and(|a| a.awake) {
            drawn.insert(mover.id);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (&id, agent) in live.0.agents.iter().filter(|(id, a)| a.awake && a.driver == Driver::Mind && !drawn.contains(id)) {
        let Some(def) = registry.index(&live.0.kinds[agent.kind].id) else { continue };
        let at = Transform::from_translation(agent.body.pos).with_rotation(Quat::from_rotation_y(agent.facing));
        commands.spawn((Creature::new(def), at, Visibility::default(), Mover::new(id), Wild));
    }
}

/// A creature with no agent gets one where it stands; an agent whose creature is gone goes.
fn enrol(mut commands: Commands, registry: Res<Registry>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, mut new: Query<(Entity, &Creature, &mut Transform), (Without<Mover>, Without<crate::creatures::Picture>)>, movers: Query<&Mover>) {
    let Some(terrain) = terrain else { return };
    for (entity, creature, mut at) in &mut new {
        let Some(kind) = live.0.kind(&registry.0[creature.def].id) else { continue };
        let id = live.0.add(&terrain.0, kind, at.translation, at.rotation.to_euler(EulerRot::YXZ).0, Driver::Stand);
        // Put down astride a step it is stood on the higher part: drawn there too, not sunk in it.
        at.translation = live.0.agents[&id].body.pos;
        commands.entity(entity).insert(Mover::new(id));
    }
    // One put down by the panel or played, whose creature has gone, goes too. (The wild are
    // the simulation's to keep or let go, drawn or not: `mirror`. Ones just enrolled have no
    // `Mover` to be found by until the commands are applied; they are the last in the list.)
    let kept: std::collections::HashSet<u32> = movers.iter().map(|m| m.id).collect();
    let fresh = live.0.agents.len() - new.iter().count();
    let mut n = 0;
    live.0.agents.retain(|id, agent| {
        n += 1;
        kept.contains(id) || n > fresh || agent.driver == Driver::Mind
    });
}

/// Who drives each creature: the keys or a wander (set from outside each frame), its own
/// mind, or nobody, in which case it stands where the panel put it.
fn drivers(mut live: ResMut<Live>, movers: Query<(&Mover, &Transform, Has<Controlled>, Has<Wander>, Has<Wild>)>) {
    for (mover, at, controlled, wander, wild) in &movers {
        let Some(agent) = live.agent_mut(mover) else { continue };
        let driver = if controlled || wander {
            Driver::Outside
        } else if wild {
            Driver::Mind
        } else {
            Driver::Stand
        };
        if agent.driver != driver {
            agent.driver = driver;
            agent.intent = default();
            agent.turn_to = None;
            agent.doing = None;
            agent.hold_jump = false;
            if driver == Driver::Stand {
                agent.body.vel = Vec3::ZERO;
            }
        }
        if driver == Driver::Stand {
            agent.body.pos = at.translation;
            agent.before = at.translation;
        }
    }
}

/// (Behind the loading screen the world is held still: no time passes, nothing moves or is
/// hurt, but what is near the player is woken, so its model is there when the screen lifts.)
pub(crate) fn tick(terrain: Option<ResMut<Terrain>>, loading: Res<crate::loading::Loading>, mut live: ResMut<Live>, mut dirty: ResMut<crate::world::Dirty>, mut grazed: Local<u32>) {
    let _slow = crate::Slow::new("tick");
    if let Some(mut terrain) = terrain {
        if loading.up {
            live.0.wake(&terrain.0)
        } else {
            let before = live.0.when.floor();
            live.0.step(&terrain.0);
            // What has been cut loose falls, and what lands is the world's again (`cube_sim::loose`).
            if !live.0.loose.pieces.is_empty() {
                let changed = live.0.loose.step(&mut terrain.0, cube_sim::DT);
                terrain.touched(&mut dirty, changed);
            }
            // What was grazed is gone from the ground: the grass and flowers an animal ate.
            for cell in std::mem::take(&mut live.0.eaten) {
                if terrain.0.get(cell).sprig() {
                    terrain.0.set(cell, cube_voxel::Block::Air);
                    *grazed += 1;
                    dirty.0.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                }
            }
            // As each day turns, the island's day: births, deaths and who moved on (`Sim::day`).
            if live.0.when.floor() != before {
                let day = live.0.day(&terrain.0, Some(&terrain.1));
                info!("day {}: born {}, died of age {}, hunted {}, starved {}, of winter {} · grazing {:.0}% left, {} tufts eaten in sight", live.0.when.floor(), day.born, day.old, day.hunted, day.starved, day.winter, day.forage.0 * 100.0, std::mem::take(&mut *grazed));
            }
        }
    }
}

/// Draws each body between ticks, tips the model into its turns, and plays the clip that
/// fits what it is doing, at the rate that keeps its feet from sliding.
/// How far ahead of itself a creature going uphill makes for the ground, seconds.
const AHEAD: f32 = 0.25;

fn show(time: Res<Time>, fixed: Res<Time<Fixed>>, registry: Res<Registry>, terrain: Option<Res<Terrain>>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut live: ResMut<Live>, mut movers: Query<(&mut Mover, &mut Creature, &mut Transform)>) {
    let _slow = crate::Slow::new("show");
    let dt = time.delta_secs();
    let between = fixed.overstep_fraction();
    let eye = cameras.iter().next().map(|c| c.translation());
    let Sim { agents, kinds, when, .. } = &mut live.0;
    for (mut m, mut creature, mut transform) in &mut movers {
        let Some(agent) = agents.get_mut(&m.id) else { continue };
        if agent.driver == Driver::Stand {
            continue;
        }
        // The model eases after a step so it does not pop, but never trails the body by more
        // than a few centimetres: on a climb the steps come sixteen a second, and an uncapped
        // lag put the feet a third of a metre inside the hill.
        // (Over it by as much as it was when it left the ground, fading: see below.)
        m.lift = ((m.lift - std::mem::take(&mut agent.stepped)) * (-22.0 * dt).exp()).clamp(-0.08, 0.5);
        transform.translation = agent.before.lerp(agent.body.pos, between) + Vec3::Y * m.lift;
        // The young are small: born at under half their size, grown by the time they breed.
        transform.scale = Vec3::splat((0.45 + 0.55 * (*when - agent.born) / cube_sim::sky::YEAR / kinds[agent.kind].maturity()).clamp(0.45, 1.0));
        // The body stands on the ground as it is drawn (`World::stood`), block by block, and
        // from one block to the next is a step of a hand or of half a metre: the model (and
        // the eyes that ride on it) come to the new level over a tenth of a second. Put
        // straight on each block as it was reached, walking was a string of jolts. Coming
        // down it is never more than a third of a metre over the body; going up, never more
        // than a hand under it, where it would be in the block it is climbing (its feet are
        // put on the ground meanwhile, `feet`).
        // (Until 2026-11-12 the body stood on the cells and the model was moved from there to
        // the drawn ground under its middle: see `World::stood`.)
        if agent.body.grounded {
            // (And going up it begins before it is there. The body is put up onto a block as
            // it reaches it, all at once; a model that only came after it, and may not be
            // more than a hand under it, went up a hillside 20 cm in a frame at every block
            // (`trunk` in a scene's lines: a deer's 20, a wolf's 12, against 3 on the level).
            // It makes for the ground drawn a quarter of a second ahead of it where that is
            // higher, as on a spring, and a climb is a ramp.)
            let at = agent.before.lerp(agent.body.pos, between);
            // (From the front of its box, which is what is put up onto a block: a deer's is
            // a third of a metre before its middle.)
            let going = (agent.body.pos - agent.before).with_y(0.0) * cube_sim::TICKS as f32;
            let front = going.normalize_or_zero() * agent.body.half;
            // (Only for what is going and is near enough to be seen at it: asked of every
            // creature every frame, the drawn ground was 8 ms of a frame and more.)
            let seen = going.length_squared() > 0.04 && eye.is_none_or(|e| e.distance_squared(at) < 35.0 * 35.0);
            let rise = [0.4, 1.0].into_iter().filter(|_| seen).filter_map(|part| terrain.as_ref().and_then(|t| crate::feet::ground_at(t, at.x + front.x + going.x * AHEAD * part, at.z + front.z + going.z * AHEAD * part, at.y))).map(|g| (g - at.y).clamp(0.0, 0.5)).fold(0.0, f32::max);
            let (y, want) = (at.y, at.y + rise);
            // (Come down from the air it is where its body is, and rises from there.)
            let was = m.shown.unwrap_or(y);
            // (A spring that neither overshoots nor starts at a stroke: critically damped.)
            let omega = 2.0 / if want > was { 0.11 } else { 0.16 };
            let (x, change) = (omega * dt, was - want);
            let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
            let temp = (m.rising + omega * change) * dt;
            m.rising = (m.rising - omega * temp) * decay;
            let eased = (want + (change + temp) * decay).clamp(y - 0.1, y + 0.5);
            if std::env::var_os("HILL_TRACE").is_some() && agent.driver == Driver::Outside {
                println!("hill t={:.3} body={:.3} want={:.3} was={:.3} shown={:.3} dt={:.4}", time.elapsed_secs(), y, want, was, eased, dt);
            }
            transform.translation.y = eased;
            m.shown = Some(eased);
            m.lift = 0.0;
        } else {
            // (Leaving the ground it is where it was drawn, and comes to its body from there:
            // put straight on its body it dropped up to half a metre at every jump of a run uphill.)
            if let Some(shown) = m.shown.take() {
                m.lift = (shown - agent.before.lerp(agent.body.pos, between).y).clamp(-0.08, 0.5);
                transform.translation.y += m.lift;
            }
            m.rising = 0.0;
        }

        let speed = agent.body.ground_speed();
        let turn = (agent.facing - agent.facing_before + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let facing = agent.facing_before + turn * between;
        // Tip into turns and into speeding up, out of slowing down: a few degrees, eased.
        let pushing = if dt > 0.0 && agent.body.grounded { (speed - m.was_speed) / dt } else { 0.0 };
        m.was_speed = speed;
        let lean = Vec2::new((-turn * cube_sim::TICKS as f32 * speed * 0.009).clamp(-0.16, 0.16), (pushing * 0.006).clamp(-0.07, 0.09));
        let ease = 1.0 - (-8.0 * dt).exp();
        m.tilt = m.tilt + (lean - m.tilt) * ease;
        transform.rotation = Quat::from_rotation_y(facing) * Quat::from_rotation_x(m.tilt.y) * Quat::from_rotation_z(m.tilt.x);

        let kind = &kinds[agent.kind];
        let def = &registry.0[creature.def];
        let rate = match agent.act {
            Act::Swim => {
                if speed > 0.3 {
                    1.1
                } else {
                    0.55
                }
            }
            Act::Walk => speed / kind.walk.max(0.1),
            Act::Run => speed / kind.run.max(0.1),
            Act::Sprint if kind.sprint > 0.0 => speed / kind.sprint,
            Act::Sprint => speed / kind.run.max(0.1),
            _ => 1.0,
        };
        // (Who shoots stands to do it: the clip of a blow is a lunge, and the bow went down with it.
        // The arms are the bow's own, `held::carry`.)
        let aims = agent.act == Act::Attack && agent.pack.as_ref().and_then(|p| p.held()).is_some_and(|i| i.reach().1 < 0.2);
        let (name, or) = if aims { ("idle", "idle") } else { agent.act.clip() };
        let find = |name: &str| def.clips.iter().position(|clip| clip.0 == name);
        // A blow struck on the move is seen over the gait (`creatures::legs`); standing, it is the whole clip.
        creature.blow = !aims && agent.dead.is_none() && agent.doing.is_some_and(|d| d.0 == Act::Attack) && matches!(agent.act, Act::Walk | Act::Run | Act::Sprint);
        // Going over the ground its gaits are mixed by its speed (`creatures::drive`).
        creature.pace = (!aims && agent.body.grounded && matches!(agent.act, Act::Idle | Act::Walk | Act::Run | Act::Sprint)).then(|| {
            // (Each gait by the speed its own clip covers the ground at, which is not always the
            // speed the creature goes at in it: a person's run is between a walk and the
            // library's run, and is their mix.)
            let own = |name: &str, or: f32| def.meta.clips.get(name).map(|c| c.speed).filter(|s| *s > 0.0).unwrap_or(or);
            [speed, own("walk", kind.walk), own("run", kind.run), own("sprint", kind.sprint)]
        });
        // (It lies down and gets up over most of a second, not the fifth of one a change of clip takes.)
        creature.blend = if agent.act == Act::Lie || creature.clip == find("lie").unwrap_or(usize::MAX) { 0.7 } else { 0.2 };
        if let Some(index) = find(name).or_else(|| find(or)) {
            creature.clip = index;
            creature.speed = rate.clamp(0.3, 2.5);
            creature.hold = None;
        }
        // Moved by its feet (`limbs`), standing, going and lying are not clips: it is stood
        // still as its idle begins and its legs and trunk do the rest.
        if creature.walked && matches!(agent.act, Act::Idle | Act::Walk | Act::Run | Act::Sprint | Act::Lie) && let Some(idle) = find("idle") {
            (creature.clip, creature.hold, creature.pace, creature.blow) = (idle, Some(0.0), None, false);
        }
        // With a hunk in their arms, the clip is the one that holds it: the walker's legs
        // go under it, and the chest up is the clip's (`stride`).
        if agent.carries.is_some() && agent.dead.is_none() && matches!(agent.act, Act::Idle | Act::Walk | Act::Run | Act::Sprint) && let Some(carry) = find("carry") {
            (creature.clip, creature.hold, creature.pace, creature.blow, creature.speed) = (carry, None, None, false, 1.0);
        }
    }
}

/// The shape each creature is bumped as: a ring at each end of its capsule, at its feet and
/// its top, joined along the sides.
fn bodies(show: Res<ShowBodies>, live: Res<Live>, creatures: Query<(&Mover, &Transform)>, mut gizmos: Gizmos) {
    if !show.0 {
        return;
    }
    let colour = Color::srgb(0.2, 0.9, 1.0);
    for (mover, at) in &creatures {
        let Some(kind) = live.kind(mover) else { continue };
        let (radius, reach) = kind.shape();
        let facing = Quat::from_rotation_y(at.rotation.to_euler(EulerRot::YXZ).0);
        // (And what of it a blow lands on: a box to its head, its trunk, each limb, its tail.)
        let parts = match live.agent(mover).map(|a| a.act) {
            Some(Act::Eat) if !kind.feeding.is_empty() => &kind.feeding,
            Some(Act::Lie) if !kind.lying.is_empty() => &kind.lying,
            _ => &kind.parts,
        };
        for (part, lo, hi) in parts {
            let tint = match part {
                cube_sim::life::Part::Head => Color::srgb(1.0, 0.3, 0.2),
                cube_sim::life::Part::Trunk => Color::srgb(1.0, 0.85, 0.2),
                _ => Color::srgb(0.4, 1.0, 0.4),
            };
            gizmos.cube(Transform { translation: at.translation + facing * ((*lo + *hi) / 2.0), rotation: facing, scale: *hi - *lo }, tint);
        }
        let (ahead, side) = (facing * Vec3::Z * reach, facing * Vec3::X * radius);
        for up in [0.02, kind.height] {
            let middle = at.translation + Vec3::Y * up;
            for end in [ahead, -ahead] {
                gizmos.circle(Isometry3d::new(middle + end, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, colour);
            }
            for edge in [side, -side] {
                gizmos.line(middle + ahead + edge, middle - ahead + edge, colour);
            }
        }
        gizmos.line(at.translation + ahead, at.translation + ahead + Vec3::Y * kind.height, colour);
        gizmos.line(at.translation - ahead, at.translation - ahead + Vec3::Y * kind.height, colour);
    }
}

/// How far off a creature is still drawn and its clip still played, metres. Beyond it a deer
/// is a few pixels; the simulation goes on moving it all the same.
const SEEN: f32 = 120.0;

/// The mouth of a den, drawn where one is: which cell of the ground it is at.
#[derive(Component)]
struct Den(IVec2);

/// A burrow is seen where something that keeps a den lives near the eye: a low heap of earth
/// with a dark mouth in it, on the ground at its home. (What goes to ground went into
/// nothing: there was no telling where a rabbit had gone, or that it had gone anywhere.)
fn dens(mut commands: Commands, live: Res<Live>, terrain: Option<Res<Terrain>>, drawn: Query<(Entity, &Den)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut made: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>, Handle<StandardMaterial>)>>) {
    let Some(terrain) = terrain else { return };
    let cell = cube_voxel::CELL;
    let wanted: std::collections::HashSet<IVec2> = live.0.agents.values().filter(|a| a.awake && a.dead.is_none() && live.0.kinds[a.kind].den).map(|a| (a.mind.home / cell).floor().as_ivec2()).collect();
    let mut have = std::collections::HashSet::new();
    for (entity, den) in &drawn {
        if wanted.contains(&den.0) {
            have.insert(den.0);
        } else {
            commands.entity(entity).despawn();
        }
    }
    let (cube, earth, dark) = made.get_or_insert_with(|| (meshes.add(Cuboid::new(1.0, 1.0, 1.0)), materials.add(StandardMaterial { base_color: Color::srgb(0.33, 0.24, 0.16), perceptual_roughness: 1.0, ..default() }), materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.025, 0.02), perceptual_roughness: 1.0, ..default() }))).clone();
    for at in wanted.difference(&have) {
        let (x, z) = ((at.x as f32 + 0.5) * cell, (at.y as f32 + 0.5) * cell);
        // (Only where the ground is made: its height is not known before.)
        if !terrain.0.chunks.keys().any(|c| c.x == at.x.div_euclid(cube_voxel::CHUNK) && c.z == at.y.div_euclid(cube_voxel::CHUNK)) {
            continue;
        }
        let ground = terrain.0.ground_height(x, z);
        // (`DEN_TRACE=1` says where each is, to go and look at one: `--wild --at X,Z`.)
        if std::env::var_os("DEN_TRACE").is_some() {
            println!("den at {x:.0},{z:.0}");
        }
        commands.spawn((Den(*at), Transform::from_xyz(x, ground, z), Visibility::default())).with_children(|den| {
            den.spawn((Mesh3d(cube.clone()), MeshMaterial3d(earth.clone()), Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::new(0.6, 0.14, 0.6))));
            den.spawn((Mesh3d(cube.clone()), MeshMaterial3d(earth.clone()), Transform::from_xyz(0.05, 0.16, -0.08).with_scale(Vec3::new(0.4, 0.12, 0.35))));
            den.spawn((Mesh3d(cube.clone()), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, 0.1, 0.22).with_scale(Vec3::new(0.26, 0.18, 0.2))));
        });
    }
}

/// Creatures far from the camera are neither drawn nor animated. With every one of 280 posed
/// and skinned each frame wherever it was, creatures were a quarter of the frame.
fn far_off(live: Res<Live>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut movers: Query<(&Mover, &Transform, &mut Visibility, Option<&crate::creatures::Rig>, Has<crate::player::Controlled>)>, mut players: Query<&mut AnimationPlayer>) {
    let _slow = crate::Slow::new("far_off");
    let Some(eye) = cameras.iter().next().map(|c| c.translation()) else { return };
    for (mover, at, mut seen, rig, controlled) in &mut movers {
        // The controlled creature's own showing is the player's to set (first person hides it).
        if controlled {
            continue;
        }
        // (Nor is what is in its den drawn: it is under the ground, `Agent::under`.)
        let near = at.translation.distance(eye) < SEEN && !live.0.agents.get(&mover.id).is_some_and(|a| a.under > 0.0);
        let want = if near { Visibility::Inherited } else { Visibility::Hidden };
        if *seen == want {
            continue;
        }
        *seen = want;
        if let Some(Ok(mut player)) = rig.map(|r| players.get_mut(r.0)) {
            if near { player.resume_all(); } else { player.pause_all(); }
        }
    }
}

/// `--watch KIND`: the scene's camera is kept on the nearest creature of a kind that lives by
/// its own mind, from a few of its lengths off, a little above and to one side. Looking for
/// wildlife by pointing a camera at where it is said to live found empty ground three times
/// in four; this finds the animal. Says once which it found and how far from the place asked.
/// With `--play` too, the player is then put down beside it.
fn watch(args: Res<crate::Args>, registry: Res<Registry>, wild: Query<(&Creature, &Transform), (With<Wild>, Without<Camera3d>)>, mut cameras: Query<&mut Transform, With<Camera3d>>, mut said: Local<bool>, mut live: ResMut<Live>, controlled: Query<&Mover, With<Controlled>>, time: Res<Time>, mut found: Local<Option<f32>>) {
    let Some(kind) = args.watch.as_deref() else { return };
    let from = Vec3::new(args.at.0, 0.0, args.at.1);
    let near = wild.iter().filter(|(c, _)| registry.0.get(c.def).is_some_and(|d| d.id == kind)).min_by(|a, b| a.1.translation.with_y(0.0).distance(from).total_cmp(&b.1.translation.with_y(0.0).distance(from)));
    // (None of its kind is being lived near the camera: then to where the nearest of them
    // is, asleep or not, which wakes it. Asked for a wolf at four in the afternoon the
    // camera sat on empty ground for ten seconds, and a fox the same.)
    if near.is_none() && let Some(mut camera) = cameras.iter_mut().next() {
        let Some(k) = live.0.kind(kind) else { return };
        if let Some(a) = live.0.agents.values().filter(|a| a.kind == k && a.dead.is_none()).min_by(|a, b| a.body.pos.with_y(0.0).distance(from).total_cmp(&b.body.pos.with_y(0.0).distance(from))) {
            *camera = Transform::from_translation(a.body.pos + Vec3::new(5.0, 3.0, 5.0)).looking_at(a.body.pos, Vec3::Y);
        }
        return;
    }
    let (Some((creature, at)), Some(mut camera)) = (near, cameras.iter_mut().next()) else { return };
    let size = registry.0[creature.def].meta.height.max(registry.0[creature.def].meta.length).max(0.3);
    let focus = at.translation + Vec3::Y * size * 0.4;
    *camera = Transform::from_translation(focus + (at.rotation * Vec3::new(0.8, 0.0, 0.6)).normalize() * size * 4.0 + Vec3::Y * size * 1.6).looking_at(focus, Vec3::Y);
    // With `--play` the player is put down six metres from it two seconds after it is found:
    // what it does when a person is suddenly near (a bird goes up).
    let since = *found.get_or_insert(time.elapsed_secs());
    if *said && time.elapsed_secs() - since > 2.0 && since >= 0.0 && let Some(agent) = controlled.iter().next().and_then(|m| live.agent_mut(m)) {
        *found = Some(-1.0);
        agent.body.pos = at.translation + at.rotation * Vec3::new(-6.0, 0.5, 0.0);
        (agent.before, agent.body.vel) = (agent.body.pos, Vec3::ZERO);
        println!("watch {kind}: the player put down 6 m from it");
    }
    if !*said {
        *said = true;
        println!("watch {kind}: found {:.0} m from the place asked, of {} of its kind awake and {} creatures in all", at.translation.with_y(0.0).distance(from), wild.iter().filter(|(c, _)| registry.0.get(c.def).is_some_and(|d| d.id == kind)).count(), wild.iter().count());
    }
}
