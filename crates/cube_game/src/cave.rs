//! A cave to look at light in (`--cave`): the land is a height map and has none of its own, so
//! one is cut into the hillside nearest where the player starts, straight into the cells (what
//! stands in it is put by a hand's puts, `cube_sim::loose`; the cutting is not in the journal). A passage in, a hall with a shaft up to the
//! sky, a low gallery to one side with a pool; stone standing on the floor and hanging from
//! the roof, rubble, moss and mushrooms. Nothing here is a rule of the game: it is a set for
//! pictures, until the land has caves of its own (docs/engine.md, "Not yet").
//!
//! Where it can be dug is a hillside: the ground must rise ten metres in the twenty-four
//! ahead; the nearest such place to the player is taken, and where it is is printed.

use crate::{
    motion::Live,
    player::Controlled,
    world::{Dirty, Terrain},
};
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension},
};
use cube_sim::terrain::SEA;
use cube_voxel::{Block, CELL};

pub struct CavePlugin;

impl Plugin for CavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dig, daylight, burn));
    }
}

/// How strong the sky's light is through the mouth and down the shaft at full day, lumens:
/// some five thousand lux in the passage and a thousand on the hall's floor (the cave's
/// stone is dark: at two thousand the passage was black to an eye inside).
const MOUTH: f32 = 1.4e7;
const SHAFT: f32 = 8.0e7;
/// How much light a patch of full sun on the hall's floor gives back, lumens.
const BOUNCE: f32 = 1.4e5;
/// How much of the day spills onto the passage round its mouth, lumens.
const SPILL: f32 = 5.0e5;
/// How bright the beam is in the air, looked through from side to side, as the picture has it.
const BEAM: f32 = 0.05;
/// How bright the rock all round is in a room of the cave by day, as light to what is in it:
/// a twentieth of what the open sky gives, to an eye that has opened four stops.
const ROOM: f32 = 25.0;

/// How far in the hall is from the mouth, and how much the ground must rise over that, metres.
const IN: f32 = 24.0;
const RISE: f32 = 10.0;

/// Where the cave is open to the sky, once it is dug: each opening as a point outside it and
/// the way in from there.
#[derive(Resource)]
struct Open([(Vec3, Vec3, f32, f32); 2], Vec3, Vec<(Transform, Vec3)>, Vec3, Vec3);

/// A fire on the hall's floor (`--fire`): its light, and whether it is the one that throws
/// shadows. Ten times a torch, so that where a torch lights the ground round whoever holds
/// it, a fire lights the hall.
#[derive(Component)]
pub struct Fire(bool);

/// The day spilling in at the mouth onto the passage's own walls and floor: the lamp outside
/// shines along it and lights only what faces out.
#[derive(Component)]
struct Spill;

/// The sun's beam down the shaft as it is seen in the air, and its share of the whole.
#[derive(Component)]
struct Beam(f32);

/// A room of the cave, as what lights it from all round: not the sky.
#[derive(Component)]
struct Room;

/// What comes back off the rock where the sun's beam lands: a hall with a patch of sun on
/// its floor is not dark.
#[derive(Component)]
struct Bounce;

/// The light an opening lets in.
#[derive(Component)]
struct Skylight(usize);

/// The sky's light through the mouth and down the shaft. The engine lights a thing from the
/// whole sky or (`world::Roofed`) from none of it: it has no way to know that from here the
/// sky is one bright hole. So each opening has a lamp outside it the colour of the sky,
/// shining in, with shadows, as strong as the day is (the mouth's reaching no further than the passage: through it,
/// it lit a round patch of the hall's far wall that hung in the mouth like a ball): what a cave's mouth is to whoever is
/// inside. (The sun's own beam down the shaft is the sun's, and the engine's.)
fn daylight(mut commands: Commands, time: Res<Time>, players: Query<&Transform, With<Controlled>>, open: Option<Res<Open>>, live: Res<Live>, mut lamps: Query<(&Skylight, &mut SpotLight)>, mut bounces: Query<&mut PointLight, (With<Bounce>, Without<Spill>)>, mut spills: Query<&mut PointLight, With<Spill>>, mut rooms: Query<&mut bevy::light::EnvironmentMapLight, With<Room>>, mut images: ResMut<Assets<Image>>, roofed: Res<crate::world::Roofed>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, beams: Query<(&Beam, &MeshMaterial3d<StandardMaterial>)>) {
    let Some(open) = open else { return };
    // (`CAVE_TRACE=1`: where the player is, every two seconds, for whoever sets a camera.)
    if std::env::var_os("CAVE_TRACE").is_some() && time.elapsed_secs() % 2.0 < time.delta_secs() {
        players.iter().for_each(|p| println!("cave: player at {:.1},{:.1},{:.1} at {:.0} s", p.translation.x, p.translation.y, p.translation.z, time.elapsed_secs()));
    }
    let day = cube_sim::sky::daylight(live.0.when);
    if lamps.is_empty() {
        for (n, &(from, to, lumens, wide)) in open.0.iter().enumerate() {
            commands.spawn((Skylight(n), SpotLight { color: Color::srgb(0.72, 0.84, 1.0), intensity: lumens * day, range: if n == 0 { 26.0 } else { 90.0 }, radius: 0.5, inner_angle: wide * 0.6, outer_angle: wide, shadow_maps_enabled: true, ..default() }, Transform::from_translation(from).looking_to(to, Vec3::Y)));
        }
    }
    if bounces.is_empty() {
        commands.spawn((Spill, PointLight { color: Color::srgb(0.78, 0.87, 1.0), intensity: 0.0, range: 11.0, radius: 0.6, shadow_maps_enabled: false, ..default() }, Transform::from_translation(open.3)));
        // What is all round a thing in a cave is rock, a little lit, and not the sky: each
        // room of it is a box in which the engine lights things from that and not from the
        // sky over the hill (a light probe, the one way it has to say so by place; and it
        // takes eight at most, so the rooms are few and square). Dim blue-grey, one colour.
        let mut dim = Image::new_fill(Extent3d { width: 1, height: 1, depth_or_array_layers: 6 }, TextureDimension::D2, &[150, 185, 255, 255], TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::RENDER_WORLD);
        dim.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
        let dim = images.add(dim);
        // The beam itself, seen in the cave's air: drawn, forty shafts of faint light one in
        // the other, so that it has no edge, along the sun's line down to the patch. (The engine's lit mist shows
        // nothing here at any thickness, tried to 0.3; why was not found.)
        let from = open.0[1].1;
        for (radius, share) in (1..=40).map(|k| (k as f32 * 0.048, 1.0 / 40.0)) {
            let lit = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, fog_enabled: false, cull_mode: None, ..default() });
            commands.spawn((Beam(share), Mesh3d(meshes.add(Cylinder::new(radius, 11.0))), MeshMaterial3d(lit), bevy::light::NotShadowCaster, Transform::from_translation(open.1 - from * 4.2).with_rotation(Quat::from_rotation_arc(Vec3::Y, -from))));
        }
        for &(place, falloff) in &open.2 {
            commands.spawn((Room, bevy::light::LightProbe { falloff }, bevy::light::EnvironmentMapLight { diffuse_map: dim.clone(), specular_map: dim.clone(), intensity: 0.0, ..default() }, place));
        }
        commands.spawn((Bounce, PointLight { color: Color::srgb(1.0, 0.86, 0.66), intensity: 0.0, range: 16.0, radius: 1.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(open.1)));
    }
    // (Set for an eye that is inside and has opened four stops. To one outside in the day
    // they are a sixth as strong: at their full strength the passage glowed out of the
    // hillside, brighter than the ground round its mouth.)
    let seen = 0.16 + 0.84 * roofed.0;
    for (lamp, mut light) in &mut lamps {
        light.intensity = open.0[lamp.0].2 * day * seen;
    }
    // The sun is on the shaft's line for an hour or so of the day; while it is, the
    // patch it lights gives its light back to the hall.
    let down = cube_sim::sky::sun(live.0.when).normalize().dot(-open.0[1].1).max(0.0);
    let aligned = ((down - 0.993) / 0.005).clamp(0.0, 1.0);
    for mut light in &mut spills {
        light.intensity = SPILL * day * seen;
    }
    for mut room in &mut rooms {
        room.intensity = ROOM * day;
    }
    for mut light in &mut bounces {
        light.intensity = BOUNCE * aligned * day;
    }
    for (beam, lit) in &beams {
        if let Some(mut lit) = materials.get_mut(&lit.0) {
            // (Warm with the sun in it; without, the sky's own light coming down, cooler
            // and three fifths as bright: the day pours in at every hour, not for one.)
            let (sun, sky) = (BEAM * beam.0 * aligned * day, BEAM * beam.0 * 0.6 * day);
            lit.base_color = Color::linear_rgb(sun + sky * 0.75, sun * 0.86 + sky * 0.86, sun * 0.62 + sky);
        }
    }
}

/// The fire's strength, lumens: what throws shadows, and what does not (the glow of the
/// embers and of the rock round it).
const FIRE: f32 = 26_000.0;
const EMBERS: f32 = 2_500.0;

/// Lights the fire once the cave is dug, and keeps it flickering.
fn burn(mut commands: Commands, args: Res<crate::Args>, time: Res<Time>, open: Option<Res<Open>>, live: Res<Live>, roofed: Res<crate::world::Roofed>, mut fires: Query<(&Fire, &mut PointLight)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let Some(open) = open.filter(|_| args.fire) else { return };
    if fires.is_empty() {
        let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.07, 0.04), perceptual_roughness: 0.9, ..default() });
        // Logs laid across each other, and the flame over them in three cubes, as a torch's is.
        for (at, size, turn) in [(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.9, 0.2, 0.2), 0.3f32), (Vec3::new(0.05, 0.1, 0.0), Vec3::new(0.2, 0.2, 0.9), -0.2), (Vec3::new(0.0, 0.28, 0.05), Vec3::new(0.7, 0.18, 0.18), 1.1)] {
            commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(wood.clone()), Transform { translation: open.4 + at, rotation: Quat::from_rotation_y(turn), scale: size }));
        }
        for (at, size, colour, glow) in [(Vec3::new(0.0, 0.5, 0.0), 0.36, [1.0, 0.58, 0.2], 10.0), (Vec3::new(0.06, 0.78, -0.04), 0.24, [1.0, 0.46, 0.12], 8.0), (Vec3::new(-0.04, 0.98, 0.03), 0.14, [1.0, 0.36, 0.07], 5.0)] {
            let colour = Color::srgb(colour[0], colour[1], colour[2]);
            let lit = materials.add(StandardMaterial { base_color: colour, emissive: colour.to_linear() * glow, perceptual_roughness: 0.7, ..default() });
            commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(lit), bevy::light::NotShadowCaster, Transform { translation: open.4 + at, scale: Vec3::splat(size), ..default() }));
        }
        let flame = |shadows: bool, range: f32| PointLight { color: Color::srgb(1.0, 0.52, 0.2), intensity: 0.0, range, radius: 0.15, shadow_maps_enabled: shadows, ..default() };
        commands.spawn((Fire(true), flame(true, 60.0), Transform::from_translation(open.4 + Vec3::Y * 0.8)));
        commands.spawn((Fire(false), flame(false, 12.0), Transform::from_translation(open.4 + Vec3::Y * 1.6)));
    }
    let t = time.elapsed_secs();
    let waver = 1.0 + 0.12 * (t * 9.0).sin() + 0.07 * (t * 21.0 + 0.7).sin() + 0.05 * (t * 4.1).sin();
    let seen = crate::player::seen(live.0.when, roofed.0);
    for (fire, mut light) in &mut fires {
        light.intensity = if fire.0 { FIRE } else { EMBERS } * waver * seen;
    }
}

/// The rock's unevenness at a place, -1 to 1, rolling smoothly from one metre to the next:
/// the same wherever it is asked from.
fn uneven(p: Vec3) -> f32 {
    let (cell, part) = (p.floor(), p - p.floor());
    let ease = part * part * (Vec3::splat(3.0) - 2.0 * part);
    let corner = |dx: i32, dy: i32, dz: i32| {
        let c = cell.as_ivec3() + IVec3::new(dx, dy, dz);
        roll((c.x.wrapping_mul(73_856_093) ^ c.y.wrapping_mul(19_349_663) ^ c.z.wrapping_mul(83_492_791)) as u32) * 2.0 - 1.0
    };
    let along = |dy: i32, dz: i32| corner(0, dy, dz) + (corner(1, dy, dz) - corner(0, dy, dz)) * ease.x;
    let across = |dz: i32| along(0, dz) + (along(1, dz) - along(0, dz)) * ease.y;
    across(0) + (across(1) - across(0)) * ease.z
}

/// A number from 0 to 1 for the nth thing strewn, the same every time.
fn roll(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// Digs the cave once, when the land round the player is made.
fn dig(mut commands: Commands, args: Res<crate::Args>, loading: Res<crate::loading::Loading>, mut done: Local<bool>, mut live: ResMut<Live>, terrain: Option<ResMut<Terrain>>, mut dirty: ResMut<Dirty>, forms: Res<crate::flora::Forms>, players: Query<&Transform, With<Controlled>>) {
    let (Some(mut terrain), Ok(player)) = (terrain, players.single()) else { return };
    if !args.cave || *done || loading.up {
        return;
    }
    let sea = SEA as f32 * CELL;
    let rises = |from: Vec3, to: Vec3, terrain: &Terrain| {
        let (low, high) = (terrain.ground(from.x, from.z), terrain.ground((from + to * IN).x, (from + to * IN).z));
        // (A hillside all the way: the roof of the passage must not come out into the air.)
        let covered = (2..=IN as i32).all(|k| terrain.ground((from + to * k as f32).x, (from + to * k as f32).z) > low + 0.25 * k as f32);
        (low > sea + 2.0 && covered).then_some(high - low)
    };
    // The nearest hillside to where the player stands, whichever way it faces, with no tree
    // standing in its mouth or on the way to it: of places four metres apart on the world's
    // own lines, so that it is the same one from wherever near it he starts.
    let trunks: Vec<Vec3> = forms.0.values().flat_map(|f| f.limbs.iter().filter(|l| l.roots > 0).map(|l| l.a.with_y(0.0))).collect();
    let clear = |from: Vec3, to: Vec3| trunks.iter().all(|t| {
        let along = (*t - from).dot(to).clamp(-7.0, 10.0);
        (*t - from - to * along).length() > 3.0
    });
    let here = player.translation.with_y(0.0);
    let middle = (here / 4.0).round();
    let found = (-40..=40).flat_map(|z| (-40..=40).map(move |x| middle + Vec3::new(x as f32, 0.0, z as f32)))
        .flat_map(|at| (0..12).map(move |way| (at * 4.0, way as f32 * std::f32::consts::TAU / 12.0)))
        .filter_map(|(from, way)| {
            let to = Vec3::new(way.sin(), 0.0, way.cos());
            rises(from, to, &terrain).filter(|rise| *rise >= RISE && clear(from, to)).map(|_| (from, to))
        })
        .min_by(|a, b| a.0.distance(here).total_cmp(&b.0.distance(here)));
    let Some((mouth, ahead)) = found else {
        println!("FAIL cave: no hillside within 160 m (the ground must rise {RISE} m in {IN})");
        *done = true;
        return;
    };
    // Not until the land it goes through is made: a cut into a region still to come does
    // nothing (the hall is a region or two from where he stands).
    let made = [0.0, IN * 0.5, IN, IN + 8.0].iter().flat_map(|along| [-8.0, 0.0, 20.0].map(|aside| mouth + ahead * *along + Vec3::new(ahead.z, 0.0, -ahead.x) * aside)).all(|p| {
        let under = Vec3::new(p.x, terrain.ground(p.x, p.z) - 0.6, p.z);
        terrain.0.get((under / CELL).floor().as_ivec3()) != Block::Air
    });
    if !made {
        return;
    }
    *done = true;
    let floor = terrain.ground(mouth.x, mouth.z);
    let left = Vec3::new(ahead.z, 0.0, -ahead.x);
    // A point of the cave: so far in, so far to the left, so high over the mouth's floor.
    let at = |along: f32, aside: f32, up: f32| mouth + ahead * along + left * aside + Vec3::Y * (floor + up);
    let mut changed = std::collections::HashSet::new();
    let mut balls = Vec::new();
    // Every ball of the cave, set out first and cut afterwards.
    let mut cut = |_: &mut Terrain, _: &mut Live, middle: Vec3, radius: f32| balls.push((middle, radius));
    // The passage: in from the hill's face and down a little. Not a bore: it bends, its
    // walls are lumps and hollows (two smaller cuts to each half metre, each off to a side
    // and up or down by its own chance), there is an alcove every few metres on one side or
    // the other, and the roof goes up in two places. A straight line down its middle is
    // clear all the same, for whoever walks in by a script. And a broad cut before it.
    let bend = |k: f32| 0.8 * (k * 0.33 + 0.4).sin() * ((k - 3.0) / 4.0).clamp(0.0, 1.0);
    let way = |k: f32| -0.14 * k.max(0.0);
    for half in -8..=38 {
        let k = half as f32 * 0.5;
        cut(&mut terrain, &mut live, at(k, bend(k), way(k) + if k < 2.0 { 2.0 } else { 1.35 }), if k < 2.0 { 2.1 } else { 1.45 });
        if k >= 3.0 {
            for lobe in 0..2u32 {
                let n = (half as u32) * 7 + lobe * 3;
                cut(&mut terrain, &mut live, at(k, bend(k) + (roll(n) - 0.5) * 3.2, way(k) + 0.9 + roll(n + 1) * 2.0), 0.7 + roll(n + 2) * 0.75);
            }
        }
    }
    for (k, side, radius) in [(6.0, 1.0, 1.7), (10.5, -1.0, 2.0), (15.0, 1.0, 1.6)] {
        cut(&mut terrain, &mut live, at(k, bend(k) + side * 2.3, way(k) + 1.2), radius);
    }
    for k in [8.5, 13.5] {
        cut(&mut terrain, &mut live, at(k, bend(k) + 0.3, way(k) + 2.7), 1.5);
    }
    // The hall, lower than the passage and four times as high: a few great cuts, and
    // twenty-odd small ones round them so that its walls are not the insides of balls.
    let hall = -1.6;
    for (along, aside, up, radius) in [(0.0, 0.0, 3.0, 4.6), (4.0, 1.5, 3.4, 4.2), (-2.5, -2.5, 2.6, 3.6), (2.0, -4.0, 2.8, 3.8), (6.5, -2.0, 2.4, 3.2), (1.0, 3.5, 2.2, 3.2), (3.0, 0.0, 6.0, 3.0)] {
        cut(&mut terrain, &mut live, at(IN + along, aside, hall + up), radius);
    }
    for n in 0..24u32 {
        let (turn, far) = (roll(n * 11 + 400) * std::f32::consts::TAU, 3.5 + roll(n * 11 + 401) * 4.0);
        cut(&mut terrain, &mut live, at(IN + 1.5 + turn.cos() * far, -0.5 + turn.sin() * far, hall + 0.6 + roll(n * 11 + 402) * 6.5), 0.9 + roll(n * 11 + 403) * 1.1);
    }
    // A hole in its roof to the sky, along the line the sun is on at some hour today: the
    // day pours down it, and at that hour a beam of the sun with it, onto the hall's floor.
    // The hour is the one with the least rock to go through (a hill that faces north has
    // the noon sun behind all of it); with none under forty-five metres it goes straight
    // up, and there is no beam. Wide, and no pipe: each cut of it off the line a little.
    let foot = at(IN + 2.0, 0.0, hall + 1.0);
    let over = |p: Vec3, terrain: &Terrain| p.y > terrain.ground(p.x, p.z) + 1.5;
    // (Out into the open, and with nothing in the way from there on: the first shaft came out
    // in the side of a gorge, facing its other wall.)
    let through = |way: Vec3, terrain: &Terrain| (5..=45).map(|k| k as f32).find(|far| over(foot + way * *far, terrain)).filter(|far| (1..=60).all(|k| over(foot + way * (*far + k as f32 * 4.0), terrain)));
    let beam = (18..=32).map(|half| half as f32 * 0.5).filter_map(|hour| {
        let way = cube_sim::sky::sun(live.0.when.floor() + hour / 24.0).normalize();
        through(way, &terrain).map(|far| (far, hour, way))
    }).min_by(|a, b| a.0.total_cmp(&b.0));
    let (long, beam, to_sun) = beam.map_or((through(Vec3::Y, &terrain).unwrap_or(45.0), None, Vec3::Y), |(far, hour, way)| (far, Some(hour), way));
    for k in 4..=long as i32 + 2 {
        let n = k as u32 * 13 + 900;
        let off = Vec3::new(roll(n) - 0.5, roll(n + 1) - 0.5, roll(n + 2) - 0.5) * 1.3;
        cut(&mut terrain, &mut live, foot + to_sun * k as f32, 1.9);
        cut(&mut terrain, &mut live, foot + to_sun * k as f32 + off, 1.6 + roll(n + 3) * 0.9);
    }
    let top = foot + to_sun * long;
    // A low gallery off to the left, going down to a basin.
    for k in 0..=9 {
        let k = k as f32;
        cut(&mut terrain, &mut live, at(IN + 1.0 - k * 0.3, 4.0 + k * 1.3, hall + 1.3 - 0.22 * k), 1.7 + 0.5 * (k * 0.9).sin().abs());
        cut(&mut terrain, &mut live, at(IN + 1.0 - k * 0.3 + (roll(k as u32 * 3 + 700) - 0.5) * 2.0, 4.0 + k * 1.3, hall + 1.6 - 0.22 * k + roll(k as u32 * 3 + 701)), 0.9 + roll(k as u32 * 3 + 702) * 0.7);
    }
    let basin = at(IN - 2.0, 17.0, hall - 0.9);
    for (aside, radius) in [(0.0, 3.0), (2.2, 2.4), (-1.6, 2.2)] {
        cut(&mut terrain, &mut live, basin + left * aside + Vec3::Y * 1.6, radius);
    }
    for &(middle, radius) in &balls {
        // The land is kept as cells only to a few metres under its surface: deeper there is
        // nothing, and a cut there opened onto the void under the world. So what a ball
        // reaches, and a metre round it, is made stone first where it is under the ground and
        // empty (the stone of a broken boulder, for the reason below). (Straight into the cells and not by the journal: a cave is a set, and a
        // region dropped and made again from its seed has the cuts and not this stone.)
        let reach = Vec3::splat(radius * 1.45 + 1.0);
        let (lo, hi) = (((middle - reach) / CELL).floor().as_ivec3(), ((middle + reach) / CELL).floor().as_ivec3());
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let top = (terrain.ground(x as f32 * CELL, z as f32 * CELL) / CELL) as i32 - 3;
                for y in lo.y..=hi.y.min(top) {
                    let cell = IVec3::new(x, y, z);
                    if terrain.0.get(cell) == Block::Air {
                        terrain.0.set(cell, Block::Shard);
                    }
                }
            }
        }
    }
    // The cutting. Not ball by ball with a shovel's cut (`loose::cut`, as it first was): a
    // ball on a grid of cells is rings of terraces, and a row of them a bore, "like they got
    // hit with a railgun" (the developer, 2026-11-14). Each ball's reach is swelled and
    // shrunk by up to two fifths, by a rolling unevenness of the rock itself that is the
    // same for every ball that comes to a place: so the walls are lumps, ledges and hollows
    // and no ball's outline is left. (Straight into the cells, as the fill is: not in the
    // journal.)
    for &(middle, radius) in &balls {
        let reach = radius * 1.45 + CELL;
        let (lo, hi) = (((middle - reach) / CELL).floor().as_ivec3(), ((middle + reach) / CELL).floor().as_ivec3());
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let cell = IVec3::new(x, y, z);
                    let p = (cell.as_vec3() + 0.5) * CELL;
                    let far = p.distance(middle);
                    if far < radius * 1.45 && far < radius * (1.0 + 0.3 * uneven(p * 0.55) + 0.14 * uneven(p * 1.7 + 31.0)) && terrain.0.get(cell).solid() {
                        terrain.0.set(cell, Block::Air);
                    }
                }
            }
        }
        let (lo, hi) = (lo.div_euclid(IVec3::splat(cube_voxel::CHUNK)), hi.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        changed.extend((lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))));
    }
    // (And a plain way through the middle of the passage, whatever the rock did there: for
    // whoever walks in by a script.)
    for half in 0..=44 {
        let (k, r) = (half as f32 * 0.5, 1.2);
        let middle = at(k, 0.0, way(k).max(hall - 1.2) + 1.35);
        let (lo, hi) = (((middle - r) / CELL).floor().as_ivec3(), ((middle + r) / CELL).floor().as_ivec3());
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let cell = IVec3::new(x, y, z);
                    if ((cell.as_vec3() + 0.5) * CELL).distance(middle) < r && terrain.0.get(cell).solid() {
                        terrain.0.set(cell, Block::Air);
                    }
                }
            }
        }
    }
    // Floors. A cut is a ball, and a row of balls leaves a gutter for a floor and rings up
    // the walls; a cave's floor is what has fallen and washed onto it. So the bottom of the
    // passage and of the hall is filled to a level that steps by a cell here and there and
    // banks up toward the walls. (Into air that has rock not far under it: not out over the
    // hillside in front of the mouth.)
    let mut lay = |terrain: &mut Terrain, p: Vec3, even: bool| {
        let top = (p / CELL).floor().as_ivec3();
        let rough = if !even && roll((top.x.div_euclid(3) * 73 + top.z.div_euclid(3) * 151) as u32) < 0.3 { 1 } else { 0 };
        if (1..=10).all(|down| terrain.0.get(top - IVec3::Y * down) == Block::Air) {
            return;
        }
        for y in (top.y - 10..=top.y + rough).rev() {
            let cell = IVec3::new(top.x, y, top.z);
            match terrain.0.get(cell) {
                Block::Air => terrain.0.set(cell, Block::Shard),
                _ => break,
            }
            changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
    };
    for step in 4..=152 {
        let k = step as f32 * 0.125;
        for side in -26..=26 {
            let off = side as f32 * 0.125;
            // (Even where he walks, down the middle: a step of a cell stops a walker.)
            let p = at(k, bend(k) + off, way(k) + 0.05 + 0.55 * (off.abs() - 1.0).max(0.0));
            lay(&mut terrain, p + Vec3::Y * 0.2 * uneven(p.with_y(0.0) * 0.6), (bend(k) + off).abs() < 0.9);
        }
    }
    for step in -56..=88 {
        for side in -72..=64 {
            let (along, aside) = (step as f32 * 0.125, side as f32 * 0.125);
            // (A swell or two across it, so that it is no table.)
            let swell = 0.3 * ((along * 0.45).sin() * (aside * 0.38 + 1.0).cos());
            let p = at(IN + along, aside, hall - 1.2 + swell);
            lay(&mut terrain, p + Vec3::Y * 0.3 * uneven(p.with_y(0.0) * 0.5), aside.abs() < 0.9);
        }
    }
    // The land is drawn as ground: the top of each column, eased and stepped, with a riser
    // down to the column beside it. That has no way to draw a roof, or a floor under one (a
    // cave so cut was walls hung over the void), and a column of ground beside one with air
    // in it is drawn with its riser hanging six metres down, across the passage. So every
    // column the cave is in or within a metre of is made, from the hill's turf to as far
    // down as there is anything, the stone a broken boulder is, which is drawn as the cells
    // it is: the hill over the cave is bare rock.
    let mut columns = std::collections::HashSet::new();
    for &(middle, radius) in &balls {
        let reach = radius * 1.45 + 1.0;
        let (lo, hi) = (((middle - reach) / CELL).floor().as_ivec3(), ((middle + reach) / CELL).floor().as_ivec3());
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                if (Vec3::new(x as f32 + 0.5, 0.0, z as f32 + 0.5) * CELL).distance(middle.with_y(0.0)) < reach {
                    columns.insert((x, z, lo.y));
                }
            }
        }
    }
    for (x, z, low) in columns {
        let mut y = (terrain.ground(x as f32 * CELL, z as f32 * CELL) / CELL) as i32 + 4;
        while y > low - 200 && (y > low || terrain.0.get(IVec3::new(x, y, z)) != Block::Air) {
            let cell = IVec3::new(x, y, z);
            if terrain.0.get(cell).ground() {
                terrain.0.set(cell, Block::Shard);
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
            y -= 1;
        }
    }
    // Water in the basin, to a level: a cell at a time into what is air below it.
    let level = ((basin.y - 0.2) / CELL).floor() as i32;
    let middle = (basin / CELL).floor().as_ivec3();
    let mut wet = Vec::new();
    for y in level - 6..=level {
        for z in -14..=14 {
            for x in -14..=14 {
                let cell = IVec3::new(middle.x + x, y, middle.z + z);
                if terrain.0.get(cell) == Block::Air {
                    wet.push(cell);
                }
            }
        }
    }
    for cell in wet {
        terrain.0.set(cell, Block::Water);
        changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
    }
    // What stands on the floor and hangs from the roof: stone put a piece on a piece, each
    // thinner than the last. And rubble, moss and mushrooms on the floor.
    let mut put = |terrain: &mut Terrain, live: &mut Live, middle: Vec3, radius: f32, block: Block| {
        changed.extend(live.0.loose.put(&mut terrain.0, middle, radius, block));
    };
    for n in 0..26u32 {
        let (turn, far) = (roll(n * 7) * std::f32::consts::TAU, 1.5 + roll(n * 7 + 1) * 6.0);
        let over = at(IN + 1.5 + turn.cos() * far, -0.5 + turn.sin() * far, hall + 3.0);
        // (None in the way of whoever walks in down the middle.)
        if (turn.sin() * far - 0.5).abs() < 1.4 && turn.cos() * far < 1.0 {
            continue;
        }
        let (Some(down), Some(above)) = (terrain.0.raycast(over, Vec3::NEG_Y, 8.0), terrain.0.raycast(over, Vec3::Y, 12.0)) else { continue };
        let (foot, roof) = (over - Vec3::Y * down, over + Vec3::Y * above);
        match n % 5 {
            // A stalagmite, and often its stalactite over it.
            0 | 1 => {
                let tall = 0.8 + roll(n * 7 + 2) * 1.8;
                let steps = (tall / 0.22) as i32;
                for k in 0..steps {
                    let thin = 1.0 - k as f32 / steps as f32;
                    put(&mut terrain, &mut live, foot + Vec3::Y * (0.1 + k as f32 * 0.22), 0.14 + 0.3 * thin, Block::Stone);
                }
                if roll(n * 7 + 3) < 0.7 && roof.y - foot.y > tall + 1.5 {
                    let long = 0.6 + roll(n * 7 + 4) * 1.4;
                    let steps = (long / 0.22) as i32;
                    for k in 0..steps {
                        let thin = 1.0 - k as f32 / steps as f32;
                        put(&mut terrain, &mut live, roof - Vec3::Y * (0.05 + k as f32 * 0.22), 0.12 + 0.24 * thin, Block::Stone);
                    }
                }
            }
            2 => put(&mut terrain, &mut live, foot + Vec3::Y * 0.3, 0.3 + roll(n * 7 + 2) * 0.35, Block::Gravel),
            3 => {
                for k in 0..5 {
                    put(&mut terrain, &mut live, foot + Vec3::new(roll(n * 7 + k) - 0.5, 0.12, roll(n * 9 + k) - 0.5) * Vec3::new(1.6, 1.0, 1.6), 0.12, Block::Moss);
                }
            }
            _ => {
                for k in 0..3 {
                    put(&mut terrain, &mut live, foot + Vec3::new(roll(n * 5 + k) - 0.5, 0.12, roll(n * 3 + k) - 0.5) * Vec3::new(1.2, 1.0, 1.2), 0.12, Block::Mushroom);
                }
            }
        }
    }
    if std::env::var_os("CAVE_TRACE").is_some() {
        // A plan of it a metre over the hall's floor, and a section along the way in: `.` air,
        // `~` water, `#` what is drawn as cells, `%` ground.
        let mark = |p: Vec3| match terrain.0.get((p / CELL).floor().as_ivec3()) { Block::Air => '.', Block::Water => '~', b if b.ground() => '%', _ => '#' };
        for along in (-4..=36).rev() {
            println!("{}", (-12..=24).map(|aside| mark(at(along as f32, aside as f32, hall + 1.0))).collect::<String>());
        }
        println!();
        for up in (-5..=14).rev() {
            println!("{}", (-4..=36).map(|along| mark(at(along as f32, 0.0, up as f32))).collect::<String>());
        }
    }
    terrain.touched(&mut dirty, changed);
    // The lamps of its two openings: six metres out from the mouth, and well up the line of
    // the shaft, so that neither is much stronger at its opening than at the hall's floor.
    commands.insert_resource(Open([(at(-6.0, 0.0, 1.6), (ahead - Vec3::Y * 0.14).normalize(), MOUTH, 0.5), (top + to_sun * 14.0, -to_sun, SHAFT, 0.2)], foot - to_sun * 2.5 + Vec3::Y * 0.5, {
        // Its rooms: the passage from where the hill is over it, the hall, the gallery and
        // its basin. Each a box square to the way in, with its edges eased into the next.
        let turn = Quat::from_mat3(&Mat3::from_cols(left, Vec3::Y, ahead));
        let room = |along: f32, aside: f32, up: f32, size: Vec3, falloff: Vec3| (Transform { translation: at(along, aside, up), rotation: turn, scale: size }, falloff);
        vec![
            room(13.5, 0.0, -0.4, Vec3::new(7.5, 6.0, 13.0), Vec3::new(0.1, 0.1, 0.45)),
            room(IN + 1.8, -0.5, hall + 3.5, Vec3::new(16.5, 13.0, 17.5), Vec3::new(0.08, 0.08, 0.12)),
            room(IN - 1.5, 13.5, hall + 0.75, Vec3::new(17.0, 8.0, 10.0), Vec3::new(0.12, 0.1, 0.1)),
        ]
    }, at(6.0, 0.0, 0.7), {
        // Where a fire would be laid: on the hall's floor, off the way through it.
        let over = at(IN + 5.0, 2.6, hall + 2.0);
        over - Vec3::Y * terrain.0.raycast(over, Vec3::NEG_Y, 8.0).unwrap_or(3.0)
    }));
    // Where to start so as to walk straight in: five metres before the mouth, facing it
    // (a scene's player is put down a metre and a half east of `--at`, and `--yaw` 0 is north).
    let (start, hall) = (mouth - ahead * 5.0, at(IN, 0.0, hall + 1.0));
    println!("cave: mouth {:.1},{:.1} · walk in from --at {:.1},{:.1} --yaw {:.2} · hall {:.1},{:.1},{:.1} · shaft's head {:.1},{:.1},{:.1}, the sun down it at {:.1} h · basin {:.1},{:.1},{:.1} · the way in {:.2},{:.2}", mouth.x, mouth.z, start.x - 1.5, start.z, (-ahead.x).atan2(-ahead.z), hall.x, hall.y, hall.z, top.x, top.y, top.z, beam.unwrap_or(-1.0), basin.x, basin.y, basin.z, ahead.x, ahead.z);
}
