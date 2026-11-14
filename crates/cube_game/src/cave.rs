//! A cave to look at light in (`--cave`): the land is a height map and has none of its own, so
//! one is dug into the hillside nearest where the player starts, by the same cuts and puts a
//! shovel and a hand make (`cube_sim::loose`), so it is in the journal like any digging and
//! the world is still its seed and its journal. A passage in, a hall with a shaft up to the
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
        app.add_systems(Update, (dig, daylight));
    }
}

/// How strong the sky's light is through the mouth and down the shaft at full day, lumens:
/// some five thousand lux in the passage and a thousand on the hall's floor (the cave's
/// stone is dark: at two thousand the passage was black to an eye inside).
const MOUTH: f32 = 1.4e7;
const SHAFT: f32 = 2.5e7;
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
struct Open([(Vec3, Vec3, f32, f32); 2], Vec3, Vec<(Transform, Vec3)>, Vec3);

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
        // The beam itself, seen in the cave's air: drawn, sixteen shafts of faint light one in
        // the other, so that it has no edge, along the sun's line down to the patch. (The engine's lit mist shows
        // nothing here at any thickness, tried to 0.3; why was not found.)
        let from = open.0[1].1;
        for (radius, share) in (1..=16).map(|k| (k as f32 * 0.08, 1.0 / 16.0)) {
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
    // The sun is on the shaft's line for half an hour or so of the day; while it is, the
    // patch it lights gives its light back to the hall.
    let down = cube_sim::sky::sun(live.0.when).normalize().dot(-open.0[1].1).max(0.0);
    let aligned = ((down - 0.9965) / 0.003).clamp(0.0, 1.0);
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
            let glow = BEAM * beam.0 * aligned * day;
            lit.base_color = Color::linear_rgb(glow, glow * 0.86, glow * 0.62);
        }
    }
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
    // The passage: in from the hill's face, down a little, winding once it is in, wider and
    // narrower; and a broad cut before it, so that whoever walks at it goes in.
    for k in -4..=18 {
        let k = k as f32;
        let wind = (k * 0.45).sin() * 0.6 * ((k - 4.0) / 4.0).clamp(0.0, 1.0);
        cut(&mut terrain, &mut live, at(k, wind, 1.3 - 0.07 * k.max(0.0)), if k < 2.0 { 2.1 } else { 1.6 + 0.25 * (k * 0.8).sin() });
    }
    // The hall, a floor lower than the passage and four times as high.
    let hall = -1.6;
    for (along, aside, up, radius) in [(0.0, 0.0, 3.0, 4.6), (4.0, 1.5, 3.4, 4.2), (-2.5, -2.5, 2.6, 3.6), (2.0, -4.0, 2.8, 3.8), (6.5, -2.0, 2.4, 3.2), (1.0, 3.5, 2.2, 3.2), (3.0, 0.0, 6.0, 3.0)] {
        cut(&mut terrain, &mut live, at(IN + along, aside, hall + up), radius);
    }
    // A shaft from its roof to the sky, along the line the sun is on at some hour today: a
    // beam of the day comes down it then, onto the hall's floor. The hour is the one with
    // the least rock to go through (a hill that faces north has the noon sun behind all of
    // it); with none under forty-five metres it goes straight up, and there is no beam.
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
    for k in 5..=long as i32 {
        cut(&mut terrain, &mut live, foot + to_sun * k as f32, 1.5);
    }
    let top = foot + to_sun * long;
    // A low gallery off to the left, going down to a basin.
    for k in 0..=9 {
        let k = k as f32;
        cut(&mut terrain, &mut live, at(IN + 1.0 - k * 0.3, 4.0 + k * 1.3, hall + 1.3 - 0.22 * k), 1.7 + 0.5 * (k * 0.9).sin().abs());
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
        let reach = Vec3::splat(radius + 1.0);
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
    for &(middle, radius) in &balls {
        changed.extend(live.0.loose.cut(&mut terrain.0, middle, radius, Vec3::Y, &[], None).changed);
        // (And every chunk the ball reached is drawn again: a cut names only those in which
        // something came loose.)
        let reach = Vec3::splat(radius + 1.0 + CELL);
        let (lo, hi) = (((middle - reach) / CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)), ((middle + reach) / CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        changed.extend((lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))));
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
        let reach = radius + 1.0;
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
    commands.insert_resource(Open([(at(-6.0, 0.0, 1.6), (ahead - Vec3::Y * 0.07).normalize(), MOUTH, 0.5), (top + to_sun * 14.0, -to_sun, SHAFT, 0.16)], foot - to_sun * 2.5 + Vec3::Y * 0.5, {
        // Its rooms: the passage from where the hill is over it, the hall, the gallery and
        // its basin. Each a box square to the way in, with its edges eased into the next.
        let turn = Quat::from_mat3(&Mat3::from_cols(left, Vec3::Y, ahead));
        let room = |along: f32, aside: f32, up: f32, size: Vec3, falloff: Vec3| (Transform { translation: at(along, aside, up), rotation: turn, scale: size }, falloff);
        vec![
            room(13.5, 0.0, 0.5, Vec3::new(5.0, 4.4, 13.0), Vec3::new(0.1, 0.1, 0.45)),
            room(IN + 1.8, -0.5, hall + 3.5, Vec3::new(16.5, 13.0, 17.5), Vec3::new(0.08, 0.08, 0.12)),
            room(IN - 1.5, 13.5, hall + 0.75, Vec3::new(17.0, 8.0, 10.0), Vec3::new(0.12, 0.1, 0.1)),
        ]
    }, at(6.0, 0.0, 1.4)));
    // Where to start so as to walk straight in: five metres before the mouth, facing it
    // (a scene's player is put down a metre and a half east of `--at`, and `--yaw` 0 is north).
    let (start, hall) = (mouth - ahead * 5.0, at(IN, 0.0, hall + 1.0));
    println!("cave: mouth {:.1},{:.1} · walk in from --at {:.1},{:.1} --yaw {:.2} · hall {:.1},{:.1},{:.1} · shaft's head {:.1},{:.1},{:.1}, the sun down it at {:.1} h · basin {:.1},{:.1},{:.1} · the way in {:.2},{:.2}", mouth.x, mouth.z, start.x - 1.5, start.z, (-ahead.x).atan2(-ahead.z), hall.x, hall.y, hall.z, top.x, top.y, top.z, beam.unwrap_or(-1.0), basin.x, basin.y, basin.z, ahead.x, ahead.z);
}
