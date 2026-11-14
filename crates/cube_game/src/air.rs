//! What is in the air, so the world is not a still picture: pollen drifting on the wind,
//! leaves coming down in the woods, butterflies over the meadows, birds wheeling high up.
//!
//! These are dressing, not creatures. Nothing in the simulation knows of them and nothing
//! reacts to them; they are kept round the camera and exist only while it looks. So they
//! live here, in the crate that draws, and take their chances from their own small generator
//! and not the simulation's seeded one. A bird that can be hunted is a creature and belongs
//! to `cube_sim`; these are the ones only ever seen.

use crate::world::Terrain;
use crate::creatures::{Creature, Registry};
use bevy::{light::NotShadowCaster, prelude::*};
use cube_sim::ecology;
use std::sync::atomic::{AtomicU32, Ordering};
use cube_voxel::CELL;
use std::f32::consts::TAU;

pub struct AirPlugin;

impl Plugin for AirPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, clock).add_systems(Update, rain).add_systems(Update, (motes, leaves, butterflies, birds, glow, heavens, songbirds, dragonflies, fireflies, swallows, fish));
    }
}

/// The hour, for every system here: each asks the ecology whether its kind of life is about
/// at this place and hour, and none of them should need the simulation handed to it for that.
static HOUR: AtomicU32 = AtomicU32::new(0);

fn hour() -> f32 {
    f32::from_bits(HOUR.load(Ordering::Relaxed))
}

fn clock(live: Option<Res<crate::motion::Live>>) {
    HOUR.store(live.map_or(cube_sim::sky::START, |l| l.0.when).to_bits(), Ordering::Relaxed);
}

/// Whether one of a kind settles at a spot: the better the place and hour suit it
/// (`ecology`), the likelier. Every kind of small life is placed through this.
fn settles(niche: &ecology::Niche, terrain: &Terrain, x: f32, z: f32, state: &mut u32) -> bool {
    roll(state) < terrain.fit(niche, x, z, hour())
}

/// How many of each there are round the camera.
const MOTES: usize = 70;
const FIREFLIES: usize = 60;
const SONGBIRDS: usize = 18;
const DRAGONFLIES: usize = 12;
const SWALLOWS: usize = 5;
/// Schools of fish near the camera, and how many swim in each.
const SCHOOLS: usize = 4;
const SCHOOL: usize = 7;
const LEAVES: usize = 56;
const BUTTERFLIES: usize = 20;
const FLOCKS: usize = 3;

/// The wind, as in `assets/shaders/wind.wesl`: which way, and metres a second for what
/// drifts on it when it blows its hardest (`world::Breeze` says how hard it blows now).
const WIND: Vec3 = Vec3::new(0.8, 0.0, 0.6);
const DRIFT: f32 = 1.0;

/// 0 to 1, a different one each call.
pub(crate) fn roll(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state & 0xFFFF) as f32 / 65535.0
}

/// The pollen's material and the fireflies', kept so their own light can follow the hour.
#[derive(Resource)]
struct Pale(Handle<StandardMaterial>, Handle<StandardMaterial>);

/// A firefly: the spot it keeps to, and its own number.
#[derive(Component)]
struct Firefly {
    home: Vec3,
    own: f32,
}

/// A star: which way it lies from anywhere.
#[derive(Component)]
struct Stars;
#[derive(Component)]
struct MoonDisc;


/// The stars' material and the moon's.
#[derive(Resource)]
struct Heavens(Handle<StandardMaterial>, Handle<StandardMaterial>);

const STARS: usize = 2600;
/// How far off the stars and moon are drawn, metres: beyond the furthest cloud (`clouds`),
/// which must pass in front of them. (Nothing is cut off for being far: the camera's far
/// plane only says what is worth drawing, and these always are.)
pub(crate) const DOME: f32 = 12_000.0;
/// The line the Milky Way's band is square to: it crosses the sky from the north-east to the
/// south-west, a little off overhead.
const GALAXY: Vec3 = Vec3::new(0.62, 0.32, -0.72);

/// A speck of pollen or down. `0` is its own number, so no two move alike.
#[derive(Component)]
struct Mote(f32);

/// A drop of rain, or a flake of snow: its own number, which says where it starts and how
/// hard it must rain before it falls at all.
#[derive(Component)]
struct Drop(f32);

/// As many drops as fall round the camera in a downpour.
const DROPS: usize = 2400;

/// A leaf on its way down: how it tumbles, and its own number.
#[derive(Component)]
struct Leaf(Vec3, f32);

/// A butterfly, and the spot it keeps to.
#[derive(Component)]
struct Butterfly {
    home: Vec3,
    own: f32,
}

/// Something here that is heard now and then: the name of its sound (`audio`).
#[derive(Component)]
pub struct Sings(pub &'static str);

/// A small bird of the meadows and woods. It is either down, hopping and pecking where it
/// landed (`to`), or in the air between `from` and `to` since `since` for `flight` seconds.
#[derive(Component)]
struct Songbird {
    from: Vec3,
    to: Vec3,
    since: f32,
    flight: f32,
    own: f32,
    facing: f32,
    /// Whether it was on the wing when last looked at: its clip changes when this does.
    aloft: bool,
}

/// A dragonfly: the water it keeps over, and the dart it is making or has made.
#[derive(Component)]
struct Dragonfly {
    over: Vec3,
    from: Vec3,
    to: Vec3,
    since: f32,
    own: f32,
}

/// A swallow: the point it loops round (which follows the camera), how wide and how high.
#[derive(Component)]
struct Swallow {
    home: Vec3,
    own: f32,
    out: f32,
    high: f32,
}

/// A fish: which school it swims with, its own number, how long it is beside the usual, and
/// when it last leapt.
#[derive(Component)]
struct Fish {
    school: usize,
    own: f32,
    long: f32,
    leapt: f32,
}

/// The ring left on the water by a leap.
#[derive(Component)]
struct Ring {
    school: usize,
    since: f32,
}

/// Where each school keeps to.
#[derive(Resource)]
struct Schools(Vec<Vec3>);

/// A bird in a flock that wheels round a point: how far out, how high, how fast, where in
/// the circle.
#[derive(Component)]
struct Bird {
    middle: Vec3,
    out: f32,
    speed: f32,
    at: f32,
}

/// What a thing of the air or the water is drawn as: the model forge built for it
/// (`assets/creatures/ID`, of boxes, rigged there), playing one of its clips. Until
/// 2026-11-11 each was a shape made here and its wings turned by hand; now they come down
/// the same pipeline as every other creature. A model faces +z and the things here move
/// toward -z, so it is turned about.
fn model(registry: &Registry, id: &str, clip: &str, speed: f32) -> impl Bundle {
    let mut creature = Creature::new(registry.index(id).unwrap_or_else(|| panic!("assets/creatures/{id} has no model: cargo forge finish {id}")));
    creature.clip_name = Some(clip.into());
    creature.speed = speed;
    (creature, crate::creatures::Picture, Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)), Visibility::default())
}

fn setup(mut commands: Commands, registry: Res<Registry>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut state = 0x9E37_79B9u32;
    let far = Vec3::new(0.0, -500.0, 0.0);
    // Pollen: lit like anything else, so it flares crossing a shaft of sun and dims in shade,
    // with a little light of its own so it is not lost there.
    let speck = meshes.add(Sphere::new(0.007).mesh().ico(1).unwrap());
    let pale = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.96, 0.78), emissive: LinearRgba::rgb(40.0, 36.0, 24.0), perceptual_roughness: 1.0, ..default() });
    // One in six is a firefly: by day a speck like the rest, by night a slow green lamp.
    let lamp = materials.add(StandardMaterial { base_color: Color::srgb(0.9, 1.0, 0.6), perceptual_roughness: 1.0, ..default() });
    commands.insert_resource(Pale(pale.clone(), lamp.clone()));
    for _ in 0..MOTES {
        let size = 0.6 + roll(&mut state) * 1.2;
        let mut mote = commands.spawn((Mote(roll(&mut state) * 100.0), Mesh3d(speck.clone()), Transform::from_translation(far).with_scale(Vec3::splat(size)), NotShadowCaster));
        mote.insert(MeshMaterial3d(pale.clone()));
    }
    // Rain: thin pale streaks, each a cuboid as everything is; the same ones are snow in the
    // cold, small and slow. Unlit and a little clear: two thousand of them lit and shadowed
    // would be the cost of a wood.
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let wet = materials.add(StandardMaterial { base_color: Color::srgba(0.6, 0.66, 0.76, 0.32), alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
    for _ in 0..DROPS {
        commands.spawn((Drop(roll(&mut state)), Mesh3d(streak.clone()), MeshMaterial3d(wet.clone()), Transform::from_translation(far).with_scale(Vec3::ZERO), NotShadowCaster));
    }
    // Fireflies: out from dusk, a few together at the edge of a wood or over long grass, each
    // drifting about its own spot and lighting for a moment every few seconds.
    let spark = meshes.add(Sphere::new(0.045).mesh().ico(1).unwrap());
    for _ in 0..FIREFLIES {
        commands.spawn((Firefly { home: far, own: roll(&mut state) * 100.0 }, Mesh3d(spark.clone()), MeshMaterial3d(lamp.clone()), Transform::from_translation(far), NotShadowCaster));
    }
    // The night sky: stars on a dome round the camera, and a moon. Their own light only, not
    // dimmed by haze; `heavens` turns them up as the day goes.
    // Each star is a small square of its own light, turned any way, and all of them are one
    // mesh: most strewn evenly over the sky, most of them faint and a few bright, as the
    // real ones are; two in five in a band across it, the Milky Way, fainter and closer set.
    // Added to the sky behind and never drawn over it, so a faint one in a sky still bright
    // is not seen at all. (As lit black balls they were black specks on every dusk and dawn.)
    let starlight = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, fog_enabled: false, cull_mode: None, ..default() });
    let (mut places, mut facing, mut tones, mut corners) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let galaxy = GALAXY.normalize();
    let (across, along) = galaxy.any_orthonormal_pair();
    for i in 0..STARS {
        let banded = i % 5 < 2;
        let to = if banded {
            let turn = roll(&mut state) * TAU;
            (across * turn.cos() + along * turn.sin() + galaxy * ((roll(&mut state) + roll(&mut state) + roll(&mut state) - 1.5) * 0.24)).normalize()
        } else {
            let (turn, up) = (roll(&mut state) * TAU, roll(&mut state));
            Vec3::new(turn.cos() * (1.0 - up * up).sqrt(), up, turn.sin() * (1.0 - up * up).sqrt())
        };
        // (Fainter toward the land's edge, where there is most air to see them through.)
        let bright = if banded { 0.1 + 0.45 * roll(&mut state).powi(4) } else { 0.14 + 0.86 * roll(&mut state).powi(6) } * ((to.y - 0.02) / 0.3).clamp(0.2, 1.0);
        // Most are white, some blue, a few amber.
        let tone = match (roll(&mut state) * 10.0) as u32 {
            0..=5 => [1.0, 1.0, 1.0],
            6..=8 => [0.75, 0.86, 1.0],
            _ => [1.0, 0.82, 0.62],
        };
        let spin = Quat::from_axis_angle(to, roll(&mut state) * TAU);
        // None under the land's edge: the sky is only drawn over it.
        if to.y < 0.02 {
            continue;
        }
        let half = DOME * (0.0005 + 0.0015 * bright);
        let (side, up) = (spin * to.any_orthonormal_vector() * half, spin * to.cross(to.any_orthonormal_vector()) * half);
        let first = places.len() as u32;
        for corner in [-side - up, side - up, side + up, -side + up] {
            places.push((to * DOME + corner).to_array());
            facing.push((-to).to_array());
            tones.push([tone[0] * bright, tone[1] * bright, tone[2] * bright, 1.0]);
        }
        corners.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    // The band's own glow, the stars too many and too faint to be seen one by one: soft round
    // patches along it, each brightest at its middle, lying over one another.
    for _ in 0..260 {
        let turn = roll(&mut state) * TAU;
        let to = (across * turn.cos() + along * turn.sin() + galaxy * ((roll(&mut state) + roll(&mut state) - 1.0) * 0.14)).normalize();
        if to.y < 0.06 {
            continue;
        }
        let (wide, bright) = (DOME * (0.03 + 0.04 * roll(&mut state)), (0.005 + 0.008 * roll(&mut state)) * ((to.y - 0.02) / 0.3).clamp(0.2, 1.0));
        let (side, up) = (to.any_orthonormal_vector() * wide, to.cross(to.any_orthonormal_vector()) * wide);
        let middle = places.len() as u32;
        places.push((to * DOME * 1.01).to_array());
        facing.push((-to).to_array());
        tones.push([0.8 * bright, 0.88 * bright, bright, 1.0]);
        for step in 0..12 {
            let round = step as f32 / 12.0 * TAU;
            places.push((to * DOME * 1.01 + side * round.cos() + up * round.sin()).to_array());
            facing.push((-to).to_array());
            tones.push([0.0, 0.0, 0.0, 1.0]);
            corners.extend([middle, middle + 1 + step, middle + 1 + (step + 1) % 12]);
        }
    }
    let dome = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, places)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, facing)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, tones)
        .with_inserted_indices(bevy::mesh::Indices::U32(corners));
    commands.spawn((Stars, Mesh3d(meshes.add(dome)), MeshMaterial3d(starlight.clone()), Transform::from_translation(far), NotShadowCaster));
    // The moon: a ball whose one half is bright and the other the colour of the night, kept
    // turned so the bright half faces the sun. Seen from the ground that is its phase, by
    // the same geometry as the real one: a thin crescent near the sun, a full face opposite.
    let moonlight = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, fog_enabled: false, ..default() });
    let mut ball = Sphere::new(1.0).mesh().uv(48, 24);
    let faces: Vec<[f32; 4]> = match ball.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(at)) => at.iter().map(|p| if p[2] > 0.0 { [1.0, 0.97, 0.88, 1.0] } else { [0.012, 0.018, 0.04, 1.0] }).collect(),
        _ => Vec::new(),
    };
    ball.insert_attribute(Mesh::ATTRIBUTE_COLOR, faces);
    commands.spawn((MoonDisc, Mesh3d(meshes.add(ball)), MeshMaterial3d(moonlight.clone()), Transform::from_translation(far).with_scale(Vec3::splat(16.0 * DOME / 700.0)), NotShadowCaster));
    commands.insert_resource(Heavens(starlight, moonlight));
    // Leaves: a flat chip of one of four autumn-turning greens and browns.
    let chip = meshes.add(Cuboid::new(0.17, 0.008, 0.12));
    let tones = [Color::srgb(0.62, 0.55, 0.14), Color::srgb(0.72, 0.5, 0.12), Color::srgb(0.62, 0.3, 0.08), Color::srgb(0.4, 0.5, 0.14)].map(|c| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, double_sided: true, ..default() }));
    for i in 0..LEAVES {
        let spin = Vec3::new(roll(&mut state) - 0.5, roll(&mut state) - 0.5, roll(&mut state) - 0.5) * 6.0;
        commands.spawn((Leaf(spin, roll(&mut state) * 100.0), Mesh3d(chip.clone()), MeshMaterial3d(tones[i % 4].clone()), Transform::from_translation(far), NotShadowCaster));
    }
    // Butterflies.
    for _ in 0..BUTTERFLIES {
        let own = roll(&mut state) * 100.0;
        commands.spawn((Butterfly { home: far, own }, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "flutterby", "flutter", 1.5 + own * 0.003));
    }
    // Small birds: on the wing, or down with their wings folded.
    for _ in 0..SONGBIRDS {
        commands.spawn((Sings("bird"), Songbird { from: far, to: far, since: -100.0, flight: 0.0, own: roll(&mut state) * 100.0, facing: 0.0, aloft: false }, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "songbird", "perch", 1.0));
    }
    // Dragonflies: their wings too fast to see beat.
    for _ in 0..DRAGONFLIES {
        commands.spawn((Sings("buzz"), Dragonfly { over: far, from: far, to: far, since: 0.0, own: roll(&mut state) * 100.0 }, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "darter", "fly", 4.0));
    }
    // Swallows: always a few on the wing near the camera, low and fast in long loops, so the
    // sky in any view has something crossing it. True to size, 15 cm long and 33 across.
    for _ in 0..SWALLOWS {
        commands.spawn((Sings("bird"), Swallow { home: far, own: roll(&mut state) * 100.0, out: 6.0 + roll(&mut state) * 9.0, high: 2.5 + roll(&mut state) * 6.0 }, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "swallow", "fly", 1.3));
    }
    // Fish: the length of a hand to a forearm, swimming by the tail. They keep just under
    // the surface where they can be seen, a school together.
    let ring = meshes.add(Annulus::new(0.9, 1.0).mesh().resolution(24));
    let ripple = materials.add(StandardMaterial { base_color: Color::srgba(0.9, 0.95, 1.0, 0.5), alpha_mode: AlphaMode::Blend, unlit: true, double_sided: true, cull_mode: None, ..default() });
    for school in 0..SCHOOLS {
        for _ in 0..SCHOOL {
            let long = 0.8 + roll(&mut state) * 1.4;
            commands.spawn((Fish { school, own: roll(&mut state) * 100.0, long, leapt: -100.0 }, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "minnow", "swim", 1.2 + roll(&mut state) * 0.6));
        }
        // The ring a leaping fish leaves, one to a school.
        commands.spawn((Ring { school, since: -100.0 }, Mesh3d(ring.clone()), MeshMaterial3d(ripple.clone()), Transform::from_translation(far), NotShadowCaster));
    }
    commands.insert_resource(Schools(vec![far; SCHOOLS]));
    // Hawks, high over the land: wheeling on spread wings, seen from below against the sky.
    for _ in 0..FLOCKS {
        let turn = roll(&mut state) * TAU;
        let middle = Vec3::new(turn.cos(), 0.0, turn.sin()) * (10.0 + roll(&mut state) * 50.0) + Vec3::Y * (34.0 + roll(&mut state) * 16.0);
        let (out, speed) = (22.0 + roll(&mut state) * 30.0, 7.0 + roll(&mut state) * 4.0);
        for b in 0..4 + (roll(&mut state) * 5.0) as usize {
            let bird = Bird { middle: middle + Vec3::Y * (roll(&mut state) - 0.5) * 6.0, out: out + (roll(&mut state) - 0.5) * 9.0, speed, at: b as f32 * 0.22 + roll(&mut state) * 0.2 };
            commands.spawn((Sings("hawk"), bird, Transform::from_translation(far), Visibility::default())).with_child(model(&registry, "soarer", if b % 3 == 0 { "fly" } else { "glide" }, 0.8));
        }
    }
}

/// Where the camera is, if there is one that looks at the world.
fn eye(cameras: &Query<&GlobalTransform, With<Camera3d>>) -> Option<Vec3> {
    cameras.iter().next().map(|c| c.translation())
}

/// Pollen drifts downwind and wanders, kept in a box round the camera: one that leaves by
/// one side comes in by the other, so there is always some wherever the camera goes.
fn motes(time: Res<Time>, breeze: Res<crate::world::Breeze>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut motes: Query<(&Mote, &mut Transform)>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let reach = Vec3::new(13.0, 5.0, 13.0);
    for (mote, mut at) in &mut motes {
        let own = mote.0;
        let wander = Vec3::new((t * 0.7 + own).sin(), (t * 0.9 + own * 1.7).sin() * 0.6, (t * 0.6 + own * 2.3).cos()) * 0.18;
        at.translation += (WIND * DRIFT * (0.1 + breeze.0) + wander) * dt;
        let mut off = at.translation - eye;
        // Far off (at the start, or after the camera jumps): anywhere in the box, by its number.
        if off.abs().max_element() > reach.x * 3.0 {
            off = Vec3::new((own * 12.989).fract() * 2.0 - 1.0, (own * 78.233).fract() * 2.0 - 1.0, (own * 37.719).fract() * 2.0 - 1.0) * reach;
        }
        // Out of the box (or never yet in it): in again on the far side.
        let wrapped = Vec3::new((off.x + reach.x).rem_euclid(reach.x * 2.0) - reach.x, (off.y + reach.y).rem_euclid(reach.y * 2.0) - reach.y, (off.z + reach.z).rem_euclid(reach.z * 2.0) - reach.z);
        at.translation = eye + wrapped;
        let floor = terrain.ground(at.translation.x, at.translation.z) + 0.15;
        if at.translation.y < floor {
            at.translation.y = floor + (own * 7.3).rem_euclid(3.0);
        }
        // Not in the camera's face: a speck a hand from the lens glowed as a white ball the
        // size of a head. It thins to nothing inside three metres.
        at.scale = Vec3::splat(((at.translation.distance(eye) - 1.5) / 1.5).clamp(0.0, 1.0));
    }
}

/// Rain falls round the camera when it rains (`world::falling`): as many of the drops as it
/// rains hard, straight down and a little with the wind, each gone at the ground and begun
/// again overhead. In the cold they are flakes, small and slow and wandering.
/// What stops the rain round the camera: the top of whatever is highest over each half
/// metre of ground (a crown, a ledge, the ground itself), so that none falls under a tree
/// or into a cave. A few columns are looked at each frame, going round; the square is begun
/// again when the camera has left its middle.
struct Cover {
    /// The corner of the square, in half metres, and the next column to look at.
    corner: IVec2,
    next: usize,
    tops: Vec<f32>,
}

impl Default for Cover {
    fn default() -> Self {
        Cover { corner: IVec2::MAX, next: 0, tops: vec![f32::NEG_INFINITY; Self::ACROSS * Self::ACROSS] }
    }
}

impl Cover {
    /// Half metres across (eighteen metres: the drops' box and a little over), how far over
    /// the camera a roof is looked for, and how many columns a frame.
    const ACROSS: usize = 36;
    const OVER: f32 = 22.0;
    const EACH: usize = 48;

    fn look(&mut self, terrain: &Terrain, eye: Vec3) {
        let middle = (eye.xz() * 2.0).floor().as_ivec2() - IVec2::splat(Self::ACROSS as i32 / 2);
        if (middle - self.corner).abs().max_element() > 4 {
            *self = Cover { corner: middle, ..default() };
        }
        for _ in 0..Self::EACH {
            let (i, j) = (self.next % Self::ACROSS, self.next / Self::ACROSS);
            let (x, z) = ((self.corner.x + i as i32) as f32 * 0.5 + 0.25, (self.corner.y + j as i32) as f32 * 0.5 + 0.25);
            let ground = terrain.ground(x, z);
            // Down from well over the camera, a half metre at a time: a crown is thicker.
            let from = eye.y + Self::OVER;
            let steps = ((from - ground) / 0.5).max(0.0) as i32;
            let top = (0..steps).map(|k| from - k as f32 * 0.5).find(|&y| terrain.0.get((Vec3::new(x, y, z) / CELL).floor().as_ivec3()).solid());
            self.tops[self.next] = top.map_or(ground, |y| y + 0.25);
            self.next = (self.next + 1) % self.tops.len();
        }
    }

    /// The top of what is over a place, if it has been looked at.
    fn top(&self, x: f32, z: f32) -> Option<f32> {
        let at = (Vec2::new(x, z) * 2.0).floor().as_ivec2() - self.corner;
        let inside = at.min_element() >= 0 && at.max_element() < Self::ACROSS as i32;
        inside.then(|| self.tops[at.x as usize + at.y as usize * Self::ACROSS]).filter(|t| t.is_finite())
    }
}

fn rain(time: Res<Time>, live: Option<Res<crate::motion::Live>>, breeze: Res<crate::world::Breeze>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut drops: Query<(&Drop, &mut Transform)>, mut cover: Local<Cover>) {
    let (Some(eye), Some(terrain), Some(live)) = (eye(&cameras), terrain, live) else { return };
    let (hard, snow) = crate::world::falling(live.0.when);
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let reach = Vec3::new(7.0, 5.0, 7.0);
    if hard > 0.02 {
        cover.look(&terrain, eye);
    }
    for (drop, mut at) in &mut drops {
        let own = drop.0;
        // (The lightest rain is a seventh of them; none when it is dry.)
        if hard <= 0.02 || own > 0.15 + hard * 0.85 {
            at.scale = Vec3::ZERO;
            continue;
        }
        let fall = if snow { Vec3::new((t * 0.8 + own * 40.0).sin() * 0.5, -1.3, (t * 0.7 + own * 90.0).cos() * 0.5) } else { Vec3::new(0.0, -9.5, 0.0) };
        at.translation += (fall + WIND * (0.3 + 2.0 * breeze.0)) * dt;
        let mut off = at.translation - eye;
        if off.abs().max_element() > reach.x * 3.0 {
            off = Vec3::new((own * 12.989 * 31.0).fract() * 2.0 - 1.0, (own * 78.233 * 17.0).fract() * 2.0 - 1.0, (own * 37.719 * 23.0).fract() * 2.0 - 1.0) * reach;
        }
        let wrapped = Vec3::new((off.x + reach.x).rem_euclid(reach.x * 2.0) - reach.x, (off.y + reach.y).rem_euclid(reach.y * 2.0) - reach.y, (off.z + reach.z).rem_euclid(reach.z * 2.0) - reach.z);
        at.translation = eye + wrapped;
        // (At the ground, or whatever is over it, it is gone: under a crown or a roof none
        // is seen, and it is seen again when it has gone round to above it.)
        let top = cover.top(at.translation.x, at.translation.z).unwrap_or_else(|| terrain.ground(at.translation.x, at.translation.z));
        if at.translation.y < top {
            at.scale = Vec3::ZERO;
            continue;
        }
        // (Not in the camera's face, as a speck is not.)
        let near = ((at.translation.distance(eye) - 0.8) / 1.0).clamp(0.0, 1.0);
        at.scale = if snow { Vec3::splat(0.035) } else { Vec3::new(0.008, 0.3, 0.008) } * near;
    }
}

/// A leaf tumbles down in a wood, swinging as it falls; on the ground it is gone, and
/// another starts from the crowns somewhere near the camera.
fn leaves(time: Res<Time>, breeze: Res<crate::world::Breeze>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut leaves: Query<(&Leaf, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x1234_5679;
    }
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    for (leaf, mut at) in &mut leaves {
        let own = leaf.1;
        let swing = Vec3::new((t * 1.9 + own).sin(), 0.0, (t * 1.6 + own * 1.3).cos()) * 0.5;
        at.translation += (WIND * DRIFT * 1.4 * (0.1 + breeze.0) + swing + Vec3::NEG_Y * (0.8 + 0.3 * (t * 2.3 + own).sin())) * dt;
        at.rotation *= Quat::from_scaled_axis(leaf.0 * dt);
        let ground = terrain.ground(at.translation.x, at.translation.z);
        if at.translation.y < ground + 0.03 || at.translation.distance(eye) > 45.0 {
            // It waits out of sight until a tree near the camera lets some go: now and then a
            // gust takes a flurry from one crown, and between gusts one comes down alone.
            at.translation = Vec3::new(0.0, -500.0, 0.0);
            let gusting = breeze.0 > 0.55;
            if roll(&mut state) > if gusting { 0.25 } else { 0.012 } {
                continue;
            }
            let trees = &terrain.1.trees;
            if trees.is_empty() {
                continue;
            }
            // In a gust every leaf looks to the same few trees.
            let first = if gusting { ((t * 0.11).floor() as usize).wrapping_mul(2_654_435_761) } else { (roll(&mut state) * 65_535.0) as usize * 7919 };
            // One of the trees near the camera, by its place in the list.
            let near: Vec<usize> = (0..trees.len()).filter(|&i| (Vec3::new(trees[i].x as f32, 0.0, trees[i].z as f32) * CELL - eye.with_y(0.0)).length() < 14.0).collect();
            if let Some(&i) = near.get(first % near.len().max(1)).filter(|&&i| ecology::FALLING_LEAF.suits(&terrain.2.place(&terrain.1, trees[i].x as f32 * CELL, trees[i].z as f32 * CELL)) > 0.2) {
                let foot = Vec3::new(trees[i].x as f32 + 0.5, trees[i].y as f32, trees[i].z as f32 + 0.5) * CELL;
                let tall = terrain.1.grown.get(i).copied().unwrap_or(12.0);
                let (turn, out) = (roll(&mut state) * TAU, roll(&mut state) * tall * 0.25);
                at.translation = foot + Vec3::new(turn.cos() * out, tall * (0.3 + 0.35 * roll(&mut state)), turn.sin() * out);
            }
        }
    }
}

/// A butterfly keeps to a few metres of meadow near the camera, flying a loose looping path
/// low over the grass, its wings beating too fast to count.
fn butterflies(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Butterfly, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x0BAD_5EED;
    }
    let t = time.elapsed_secs();
    for (mut fly, mut at) in &mut flies {
        // Kept close: a butterfly thirty metres off is nothing at all. And gone when the
        // hour or the place stops suiting it: none at night.
        let gone = fly.home.y > -100.0 && terrain.fit(&ecology::BUTTERFLY, fly.home.x, fly.home.z, hour()) < 0.08;
        if fly.home.distance(eye) > 19.0 || gone {
            fly.home = Vec3::new(0.0, -500.0, 0.0);
            for _ in 0..6 {
                let (turn, out) = (roll(&mut state) * TAU, 3.0 + roll(&mut state) * 12.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                if settles(&ecology::BUTTERFLY, &terrain, x, z, &mut state) {
                    fly.home = Vec3::new(x, 0.0, z);
                    break;
                }
            }
        }
        if fly.home.y < -100.0 {
            at.translation = fly.home;
            continue;
        }
        let own = fly.own;
        let path = |t: f32| Vec3::new((t * 0.6 + own).sin() * 2.2 + (t * 1.7 + own * 2.0).sin() * 0.6, 0.0, (t * 0.47 + own * 1.4).cos() * 2.2 + (t * 1.3 + own).cos() * 0.6);
        let (now, soon) = (fly.home + path(t), fly.home + path(t + 0.1));
        let high = terrain.ground(now.x, now.z) + 0.55 + 0.3 * (t * 2.1 + own).sin() + 0.12 * (t * 9.0 + own).sin();
        at.translation = Vec3::new(now.x, high, now.z);
        at.look_to((soon - now).normalize_or(Vec3::X), Vec3::Y);
    }
}

/// Birds wheel in loose flocks high over the land, banked into the turn, a few slow
/// wingbeats and then a glide.
fn birds(time: Res<Time>, mut birds: Query<(&Bird, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (bird, mut at) in &mut birds {
        let round = bird.at + t * bird.speed / bird.out;
        let rise = (t * 0.21 + bird.at * 3.0).sin() * 3.0;
        at.translation = bird.middle + Vec3::new(round.cos(), 0.0, round.sin()) * bird.out + Vec3::Y * rise;
        let ahead = Vec3::new(-round.sin(), 0.0, round.cos());
        *at = at.looking_to(ahead, Vec3::Y);
        at.rotate_local_z(-0.35);
    }
}

/// Pollen's own light is set for day. As the camera opens up for night it is turned down by
/// as much, or every speck would blaze like a lamp; the fireflies' is turned up instead.
fn glow(pale: Option<Res<Pale>>, cameras: Query<&bevy::camera::Exposure, With<Camera3d>>, mut materials: ResMut<Assets<StandardMaterial>>, mut was: Local<Option<f32>>) {
    let (Some(pale), Some(exposure)) = (pale, cameras.iter().next()) else { return };
    // (`was` starts at nothing, not at 0: full night is an exposure of 0.)
    if was.is_some_and(|w: f32| (exposure.ev100 - w).abs() < 0.05) {
        return;
    }
    *was = Some(exposure.ev100);
    let night = 1.0 - ((exposure.ev100 - 1.0) / 8.0).clamp(0.0, 1.0);
    let dim = 2.0f32.powf(exposure.ev100 - 13.4) * (0.1 + 0.9 * ((exposure.ev100 - 6.0) / 7.0).clamp(0.0, 1.0));
    if let Some(mut material) = materials.get_mut(&pale.0) {
        material.emissive = LinearRgba::rgb(22.0 * dim, 20.0 * dim, 13.0 * dim);
    }
    if let Some(mut material) = materials.get_mut(&pale.1) {
        // Bright for the exposure it is seen at: dusk needs more than full night.
        let lit = 2.4 * night;
        material.emissive = LinearRgba::rgb(0.55 * lit, 1.0 * lit, 0.12 * lit);
    }
}

/// Fireflies come out as the light goes: each keeps to a spot near the camera in a wood, a
/// meadow or by water, a few sharing one place, wanders slowly about it at knee to head
/// height, and lights for half a second every few. Dark, it is not drawn.
fn fireflies(time: Res<Time>, live: Option<Res<crate::motion::Live>>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Firefly, &mut Transform)>, mut state: Local<u32>, mut gather: Local<Vec3>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0xF12E_F1E5;
    }
    let hour = live.map_or(cube_sim::sky::START, |l| l.0.when);
    let out = 1.0 - cube_sim::sky::daylight(hour);
    let t = time.elapsed_secs();
    let hidden = Vec3::new(0.0, -500.0, 0.0);
    for (mut fly, mut at) in &mut flies {
        if out < 0.25 {
            (fly.home, at.translation) = (hidden, hidden);
            continue;
        }
        if fly.home.distance(eye) > 26.0 {
            fly.home = hidden;
            // Three in four settle beside the last one that found a place: clusters.
            let near_last = gather.distance(eye) < 30.0 && roll(&mut state) < 0.75;
            for _ in 0..6 {
                let (turn, far) = (roll(&mut state) * TAU, if near_last { roll(&mut state) * 3.5 } else { 4.0 + roll(&mut state) * 16.0 });
                let middle = if near_last { *gather } else { eye };
                let (x, z) = (middle.x + turn.cos() * far, middle.z + turn.sin() * far);
                if settles(&ecology::FIREFLY, &terrain, x, z, &mut state) {
                    fly.home = Vec3::new(x, terrain.ground(x, z), z);
                    if !near_last {
                        *gather = fly.home;
                    }
                    break;
                }
            }
        }
        if fly.home.y < -100.0 {
            at.translation = hidden;
            continue;
        }
        let own = fly.own;
        let wander = Vec3::new((t * 0.31 + own).sin() * 1.3 + (t * 0.83 + own * 2.0).sin() * 0.4, 0.9 + 0.5 * (t * 0.23 + own * 1.7).sin() + 0.12 * (t * 1.9 + own).sin(), (t * 0.27 + own * 1.3).cos() * 1.3 + (t * 0.71 + own).cos() * 0.4);
        at.translation = fly.home + wander;
        // Lit for a moment in every few seconds, each to its own time, swelling and fading.
        let every = 2.6 + (own * 1.7).fract() * 2.4;
        let phase = ((t + own * 3.1) / every).fract();
        let lit = if phase < 0.3 { (phase / 0.3 * std::f32::consts::PI).sin() } else { 0.0 };
        at.scale = Vec3::splat(lit * (0.6 + 0.4 * out));
    }
}

/// Stars and the moon keep their places in the sky wherever the camera goes, and come out as
/// the light goes: the moon with the dusk, the stars after it, from the end of twilight (the
/// sun six degrees under) to night proper, and no star while the sky is still bright.
fn heavens(live: Option<Res<crate::motion::Live>>, heavens: Option<Res<Heavens>>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut materials: ResMut<Assets<StandardMaterial>>, mut stars: Query<&mut Transform, (With<Stars>, Without<MoonDisc>)>, mut moon: Query<&mut Transform, With<MoonDisc>>, mut was: Local<f32>) {
    let (Some(eye), Some(heavens)) = (eye(&cameras), heavens) else { return };
    let hour = live.map_or(cube_sim::sky::START, |l| l.0.when);
    let (to_sun, night) = (cube_sim::sky::sun(hour), 1.0 - cube_sim::sky::daylight(hour));
    let out = ((-0.105 - to_sun.y) / 0.12).clamp(0.0, 1.0);
    for mut at in &mut stars {
        at.translation = if out > 0.0 { eye } else { Vec3::new(0.0, -50_000.0, 0.0) };
    }
    // Where `world::sky` puts the moon's light.
    let to_moon = cube_sim::sky::moon(hour);
    if let Ok(mut at) = moon.single_mut() {
        at.translation = if night > 0.0 && to_moon.y > -0.05 { eye + to_moon * DOME } else { Vec3::new(0.0, -50_000.0, 0.0) };
        // Its bright half to the sun.
        at.look_to(-to_sun, Vec3::Y);
    }
    // A full moon's sky has fewer stars in it.
    let stars = 1.7 * out * (1.0 - 0.5 * cube_sim::sky::moonlight(hour));
    if (night + stars - *was).abs() < 0.01 {
        return;
    }
    *was = night + stars;
    if let Some(mut material) = materials.get_mut(&heavens.0) {
        material.base_color = Color::linear_rgb(stars, stars, stars);
    }
    if let Some(mut material) = materials.get_mut(&heavens.1) {
        material.base_color = Color::linear_rgb(1.6 * night, 1.6 * night, 1.6 * night);
    }
}

/// Small birds live on the ground near the camera: a few hops, a peck, a look round, and
/// every so often a low flight to somewhere else. Anything that comes within a few metres
/// (the player, here the camera's own ground) puts them up at once, and they go further.
#[allow(clippy::type_complexity)]
fn songbirds(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    terrain: Option<Res<Terrain>>,
    live: Option<Res<crate::motion::Live>>,
    mut birds: Query<(&mut Songbird, &mut Transform, &Children)>,
    mut models: Query<&mut Creature>,
    mut state: Local<u32>,
) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x5EED_B12D;
    }
    let t = time.elapsed_secs();
    // What scares them: every creature that walks, the player among them.
    let walkers: Vec<Vec3> = live.map(|l| l.0.agents.values().map(|a| a.body.pos).collect()).unwrap_or_default();
    for (mut bird, mut at, children) in &mut birds {
        let flying = t - bird.since < bird.flight;
        if !flying {
            let scared = walkers.iter().any(|w| w.distance(bird.to) < 4.5);
            let restless = (t * 0.13 + bird.own).fract() < 0.012;
            let lost = bird.to.distance(eye) > 26.0;
            if scared || restless || lost {
                // Somewhere else on dry ground: near if it is only restless, far if scared.
                for _ in 0..6 {
                    let (turn, out) = (roll(&mut state) * TAU, if lost { 4.0 + roll(&mut state) * 13.0 } else if scared { 12.0 + roll(&mut state) * 14.0 } else { 3.0 + roll(&mut state) * 8.0 });
                    let middle = if lost { eye } else { bird.to };
                    let (x, z) = (middle.x + turn.cos() * out, middle.z + turn.sin() * out);
                    // (By where it suits them, whatever the hour: at night they roost where they are.)
                    if roll(&mut state) < ecology::SONGBIRD.suits(&terrain.2.place(&terrain.1, x, z)) {
                        let from = if lost { Vec3::new(x, terrain.ground(x, z) + 6.0, z) } else { at.translation };
                        (bird.from, bird.to, bird.since) = (from, Vec3::new(x, terrain.ground(x, z) + 0.04, z), t);
                        bird.flight = (bird.from.distance(bird.to) / 7.5).max(0.4);
                        break;
                    }
                }
            }
        }
        let flying = t - bird.since < bird.flight;
        if flying {
            // A low arc, quick to rise and gliding in.
            let part = (t - bird.since) / bird.flight;
            let eased = part * part * (3.0 - 2.0 * part);
            let arc = bird.from.distance(bird.to) * 0.3 * (4.0 * part * (1.0 - part));
            let now = bird.from.lerp(bird.to, eased) + Vec3::Y * arc;
            let ahead = (bird.to - bird.from).with_y(0.0).normalize_or(Vec3::X);
            bird.facing = ahead.x.atan2(ahead.z) + std::f32::consts::PI;
            *at = Transform::from_translation(now).with_rotation(Quat::from_rotation_y(bird.facing) * Quat::from_rotation_x(0.5 - part));
        } else {
            // Down: hops in little bursts, turning between them, with a peck now and then.
            let beat = t * 2.4 + bird.own;
            let hopping = beat.fract() < 0.35 && (beat.floor() * 0.37 + bird.own).fract() < 0.6;
            if beat.fract() < 0.02 {
                bird.facing += (roll(&mut state) - 0.5) * 1.6;
            }
            let hop = if hopping { (beat.fract() / 0.35 * std::f32::consts::PI).sin() } else { 0.0 };
            let forward = Quat::from_rotation_y(bird.facing) * Vec3::NEG_Z;
            if hopping {
                let step = forward * 0.25 * time.delta_secs();
                let next = bird.to + step;
                bird.to = Vec3::new(next.x, terrain.ground(next.x, next.z) + 0.04, next.z);
            }
            let peck = if !hopping && (beat.floor() * 0.61 + bird.own).fract() < 0.4 { (beat.fract() * TAU * 2.0).sin().max(0.0) * 0.7 } else { 0.0 };
            *at = Transform::from_translation(bird.to + Vec3::Y * (0.03 + hop * 0.05)).with_rotation(Quat::from_rotation_y(bird.facing) * Quat::from_rotation_x(-peck));
        }
        // Beating in the air, folded along the body on the ground: its clip, changed as it
        // takes off and as it lands.
        if flying != bird.aloft {
            bird.aloft = flying;
            for &child in children {
                if let Ok(mut model) = models.get_mut(child) {
                    model.clip_name = Some(if flying { "fly" } else { "perch" }.into());
                    model.speed = if flying { 2.4 } else { 1.0 };
                }
            }
        }
    }
}

/// Dragonflies keep to water near the camera: hanging still in the air for a moment, then
/// gone a couple of metres in a blink and hanging again.
fn dragonflies(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Dragonfly, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0xD2A6_0F17;
    }
    let t = time.elapsed_secs();
    let water = cube_sim::terrain::SEA_LEVEL;
    let wet = |x: f32, z: f32| terrain.1.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32) == cube_sim::terrain::Biome::Water && terrain.fit(&ecology::DRAGONFLY, x, z, hour()) > 0.3;
    for (mut fly, mut at) in &mut flies {
        if fly.over.distance(eye) > 45.0 {
            fly.over = Vec3::new(0.0, -500.0, 0.0);
            for _ in 0..8 {
                let (turn, out) = (roll(&mut state) * TAU, 4.0 + roll(&mut state) * 34.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                if wet(x, z) {
                    fly.over = Vec3::new(x, water, z);
                    (fly.from, fly.to, fly.since) = (fly.over + Vec3::Y * 0.6, fly.over + Vec3::Y * 0.6, t);
                    break;
                }
            }
        }
        if fly.over.y < -100.0 {
            at.translation = fly.over;
            continue;
        }
        // Hang, then dart: a new place over the same water every second or so.
        let hang = 0.7 + (fly.own * 3.7).fract() * 1.1;
        if t - fly.since > hang {
            let (turn, out) = (roll(&mut state) * TAU, 0.8 + roll(&mut state) * 2.6);
            let mut to = fly.over + Vec3::new(turn.cos() * out, 0.25 + roll(&mut state) * 0.9, turn.sin() * out);
            if !wet(to.x, to.z) {
                to = fly.over + Vec3::Y * 0.5;
            }
            (fly.from, fly.to, fly.since) = (fly.to, to, t);
        }
        let dart = ((t - fly.since) / 0.16).clamp(0.0, 1.0);
        let now = fly.from.lerp(fly.to, dart * dart * (3.0 - 2.0 * dart)) + Vec3::Y * 0.015 * (t * 31.0 + fly.own).sin();
        let ahead = (fly.to - fly.from).with_y(0.0).normalize_or(Vec3::X);
        *at = Transform::from_translation(now).looking_to(ahead, Vec3::Y);
    }
}

/// Swallows hawk for insects round the camera: long fast loops a few metres up, dipping and
/// rising, banked into the turn. By night they are gone.
fn swallows(time: Res<Time>, live: Option<Res<crate::motion::Live>>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut birds: Query<(&mut Swallow, &mut Transform)>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    let day = cube_sim::sky::daylight(live.map_or(cube_sim::sky::START, |l| l.0.when));
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    for (mut bird, mut at) in &mut birds {
        // Over open ground within reach of water, by day (`ecology::SWALLOW`).
        if day < 0.5 || terrain.fit(&ecology::SWALLOW, eye.x, eye.z, hour()) < 0.25 {
            at.translation = Vec3::new(0.0, -500.0, 0.0);
            continue;
        }
        // The loop's middle drifts after the camera; far behind, it is there at once.
        let want = Vec3::new(eye.x, 0.0, eye.z);
        bird.home = if bird.home.distance(want) > 40.0 { want } else { bird.home.lerp(want, (dt * 0.3).min(1.0)) };
        let round = bird.own + t * 8.5 / bird.out;
        let flat = bird.home + Vec3::new(round.cos() * bird.out, 0.0, (round * 1.0 + bird.own).sin() * bird.out * 0.7);
        let up = terrain.ground(flat.x, flat.z).max(cube_sim::terrain::SEA_LEVEL) + bird.high + 1.6 * (round * 2.0 + bird.own).sin();
        let now = Vec3::new(flat.x, up, flat.z);
        let ahead = (now - at.translation).normalize_or(Vec3::X);
        *at = Transform::from_translation(now).looking_to(ahead, Vec3::Y);
        at.rotate_local_z(-0.5 * (round * 1.0).cos());
    }
}

/// Fish keep to water near the camera that is deep enough to swim in. A school circles its
/// place a hand's depth under the surface, each fish a little off the others' line, tails
/// beating; now and then one leaps clear and falls back, and a ring spreads from where it did.
#[allow(clippy::type_complexity)]
fn fish(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    terrain: Option<Res<Terrain>>,
    schools: Option<ResMut<Schools>>,
    mut fishes: Query<(&mut Fish, &mut Transform), Without<Ring>>,
    mut rings: Query<(&mut Ring, &mut Transform), Without<Fish>>,
    mut commands: Commands,
    mut bank: ResMut<crate::audio::Bank>,
    mut state: Local<u32>,
) {
    let (Some(eye), Some(terrain), Some(mut schools)) = (eye(&cameras), terrain, schools) else { return };
    if *state == 0 {
        *state = 0xF15C_A7E5;
    }
    let t = time.elapsed_secs();
    let surface = cube_sim::terrain::SEA_LEVEL;
    let hidden = Vec3::new(0.0, -500.0, 0.0);
    let deep = |x: f32, z: f32| terrain.fit(&ecology::FISH, x, z, hour()) > 0.5;
    for home in schools.0.iter_mut() {
        if home.distance(eye) > 42.0 {
            *home = hidden;
            for _ in 0..10 {
                let (turn, out) = (roll(&mut state) * TAU, 4.0 + roll(&mut state) * 30.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                // Room to circle: deep here and a couple of metres each way.
                if deep(x, z) && deep(x + 2.0, z) && deep(x - 2.0, z) && deep(x, z + 2.0) && deep(x, z - 2.0) {
                    *home = Vec3::new(x, surface, z);
                    break;
                }
            }
        }
    }
    for (mut one, mut at) in &mut fishes {
        let home = schools.0[one.school];
        if home.y < -100.0 {
            at.translation = hidden;
            continue;
        }
        let own = one.own;
        // Round the school's place, each on its own line and at its own depth.
        let round = t * 0.5 + own * 0.3 + one.school as f32;
        let wide = 1.2 + (own * 1.3).fract() * 1.2;
        let path = |a: f32| home + Vec3::new(a.cos() * wide + (a * 2.3 + own).sin() * 0.3, 0.0, (a * 1.0).sin() * wide * 0.8 + (a * 1.7 + own).cos() * 0.3);
        let (now, soon) = (path(round), path(round + 0.05));
        let mut height = -0.07 - 0.12 * (0.5 + 0.5 * (t * 0.4 + own).sin());
        // A leap: seldom, a third of a second up and a third down.
        // (By the second and not by the frame: a chance a frame made fish leap, and splash,
        // many times as often on a fast machine as on a slow one.)
        if t - one.leapt > 9.0 && roll(&mut state) < 0.036 * time.delta_secs() {
            one.leapt = t;
            // And is heard to, one leap in four: a small splash from where it came out. (Every
            // leap of every fish was a splash every two seconds beside any water.)
            if roll(&mut state) < 0.25 {
                bank.play(&mut commands, "splash", Some(now), 0.2);
            }
            if let Some((mut ring, _)) = rings.iter_mut().find(|(r, _)| r.school == one.school) {
                ring.since = t + 0.55;
            }
        }
        let leap = ((t - one.leapt) / 0.6).clamp(0.0, 1.0);
        let mut nose = 0.0;
        if leap < 1.0 {
            height += 0.42 * (4.0 * leap * (1.0 - leap));
            nose = 0.9 * (1.0 - 2.0 * leap);
        }
        *at = Transform::from_translation(now.with_y(surface + height)).looking_to((soon - now).normalize_or(Vec3::X), Vec3::Y).with_scale(Vec3::splat(one.long));
        at.rotate_local_x(nose);
    }
    for (ring, mut at) in &mut rings {
        // Spreads to a metre across in a second and a half, thinning, then is gone.
        let age = (t - ring.since) / 1.5;
        let home = schools.0[ring.school];
        *at = if (0.0..1.0).contains(&age) && home.y > -100.0 {
            Transform::from_translation(home.with_y(surface + 0.01)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(0.1 + 0.5 * age))
        } else {
            Transform::from_translation(hidden)
        };
    }
}
