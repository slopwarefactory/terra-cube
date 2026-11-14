//! What has been cut loose is seen to fall (`cube_sim::loose`): each piece is drawn as the
//! cells it is, moved as the simulation moves it, until it lands and is the world's again.
//! The simulation owns all of it; this only draws, and sounds it: a cut is heard and throws
//! chips, a tree cracks as it begins to go, and what lands is heard by its size and throws
//! up what it is made of.

use crate::motion::Live;
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::loose::Part;
use cube_voxel::{Block, SPLIT};

pub struct LoosePlugin;

impl Plugin for LoosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fall, felled, told, chips).after(crate::motion::tick)).add_systems(PostUpdate, carried.after(bevy::transform::TransformSystems::Propagate));
    }
}

/// A felled tree, drawn as the tree it was: which ([`cube_sim::loose::Thing::id`]), and as
/// what: with its boughs and leaf (0), bare (1), its stump (2), or the length of its trunk an
/// axe is at as it stands, with the notch so many blows deep (3 and that many).
#[derive(Component)]
struct Felled(u32, u8);

/// A tree an axe has cut through is drawn as the tree it grew as, where the simulation has
/// it: going over, lying, and bare once its boughs are off, until it is broken into logs
/// and is cells (`Thing::cells`); and its stump where it stands, until an axe is taken to
/// that (`Thing::stump`). Its cells are in the world all the while and are not drawn
/// (`Block::whole`). Its shape is made on another thread from its first blow, so it is
/// ready when it goes.
fn felled(mut commands: Commands, live: Option<Res<Live>>, look: Option<Res<crate::world::Look>>, terrain: Option<Res<crate::world::Terrain>>, mut drawn: Query<(Entity, &Felled, &mut Transform, &mut Visibility)>, mut meshes: ResMut<Assets<Mesh>>, mut making: Local<std::collections::HashMap<(u32, u8), bevy::tasks::Task<Mesh>>>) {
    let (Some(live), Some(look), Some(terrain)) = (live, look, terrain) else { return };
    let loose = &live.0.loose;
    // (An axe's notch is a wedge cut out of the trunk's own cubes, pale inside: the tree
    // stands drawn alone from its first blow, in three lengths, and the one the axe is at
    // is made again a blow deeper each time. Until 2026-11-14 it was a pale box with dark
    // slabs over and under it put on the bark: "this weird rendering glitch", the developer.)
    let notch = |t: &cube_sim::loose::Thing| t.notch.filter(|_| !t.felled && !t.cells);
    // What is to be drawn of each tree: itself as it now is, and its stump.
    let wanted: Vec<(&cube_sim::loose::Thing, u8)> = loose.things.iter().filter(|t| !t.limbs.is_empty()).flat_map(|t| [(!t.cells).then_some((t, t.limbed as u8)), (t.stump || !t.felled && !t.cells).then_some((t, 2)), notch(t).map(|_| (t, 3 + t.blows.min(200) as u8))].into_iter().flatten()).collect();
    // Where it is: where it grew, moved as it lies and then as the piece it is while it
    // falls or settles moves it. Its stump is where it grew.
    let place = |t: &cube_sim::loose::Thing, what: u8| {
        let origin = t.limbs[0].a;
        if what >= 2 {
            return Transform::from_translation(origin);
        }
        let lain = |p: Vec3| t.lies.map_or(p, |(pivot, shift, turn)| pivot + shift + turn * (p - pivot));
        let piece = loose.pieces.iter().find(|p| p.of() == Some(t.id));
        let at = piece.map_or(lain(origin), |p| p.now(lain(origin)));
        let turn = piece.map_or(Quat::IDENTITY, |p| p.turn) * t.lies.map_or(Quat::IDENTITY, |l| l.2);
        Transform { translation: at, rotation: turn, scale: Vec3::ONE }
    };
    let shown = |t: &cube_sim::loose::Thing| if t.felled || t.notch.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    // (As it was drawn, until it is ready as it now is: bare, once its boughs are off.)
    let have: std::collections::HashSet<(u32, u8)> = drawn.iter().map(|(_, f, ..)| (f.0, f.1)).collect();
    for (entity, felled, mut at, mut seen) in &mut drawn {
        let still = wanted.iter().find(|(t, what)| (t.id, *what) == (felled.0, felled.1)).or_else(|| wanted.iter().find(|(t, what)| t.id == felled.0 && felled.1 == 0 && *what == 1 && !have.contains(&(t.id, 1)))).or_else(|| wanted.iter().find(|(t, what)| t.id == felled.0 && felled.1 >= 3 && *what >= 3 && !have.contains(&(t.id, *what))));
        match still {
            Some((tree, _)) => (*at, *seen) = (place(tree, felled.1), shown(tree)),
            None => commands.entity(entity).despawn(),
        }
    }
    making.retain(|key, _| wanted.iter().any(|(t, what)| (t.id, *what) == *key));
    for (tree, what) in &wanted {
        let key = (tree.id, *what);
        if have.contains(&key) {
            continue;
        }
        match making.get_mut(&key).map(bevy::tasks::futures::check_ready) {
            Some(Some(mesh)) => {
                making.remove(&key);
                commands.spawn((Felled(key.0, key.1), Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.whole.clone()), place(tree, *what), shown(tree)));
            }
            Some(None) => {}
            None => {
                let (form, land, origin) = (match *what { 2 => tree.stumped(), 0 | 1 => tree.form(), _ => tree.band() }, terrain.1.clone(), tree.limbs[0].a);
                let cut = notch(tree).filter(|_| *what >= 3);
                making.insert(key, bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { crate::flora::alone(&form, &land, origin, cut) }));
            }
        }
    }
}

/// The hunk in somebody's hands, drawn before them as the cells it will be when it is put
/// down: whose it is, and of what.
#[derive(Component)]
struct Carried(u32, Block);

fn carried(mut commands: Commands, live: Option<Res<Live>>, look: Option<Res<crate::world::Look>>, bearers: Query<(&crate::motion::Mover, &Transform, Option<&crate::stride::Strode>), Without<Carried>>, places: Query<&GlobalTransform, Without<Carried>>, mut drawn: Query<(Entity, &Carried, &mut Transform, &mut GlobalTransform)>, mut meshes: ResMut<Assets<Mesh>>, mut made: Local<std::collections::HashMap<Block, Handle<Mesh>>>) {
    let (Some(live), Some(look)) = (live, look) else { return };
    let side = cube_sim::loose::HUNK as f32 * cube_voxel::CELL;
    // On his hands, where they are drawn, and turned as he is: its underside a little under
    // his palms, its middle between them. (Where the simulation has him is not where his
    // body is drawn on a climb, which the walker takes a step behind: put there, it rode
    // at his head.) Whoever has no hands the walker knows holds it before their middle.
    let held = |at: &Transform, tall: f32, strode: Option<&crate::stride::Strode>| {
        let hands = strode.map(|s| s.hands()).and_then(|[r, l]| Some((places.get(r).ok()?.translation() + places.get(l).ok()?.translation()) / 2.0));
        let middle = hands.map_or(at.translation + at.rotation * Vec3::new(0.0, tall * 0.44 + side / 2.0, 0.45), |h| h + Vec3::Y * (side / 2.0 - 0.06) + at.rotation * Vec3::Z * 0.12);
        Transform { translation: middle - at.rotation * Vec3::splat(side / 2.0), rotation: at.rotation, scale: Vec3::ONE }
    };
    let carries = |m: &crate::motion::Mover| live.agent(m).filter(|a| a.dead.is_none()).and_then(|a| a.carries).zip(live.kind(m).map(|k| k.height));
    for (entity, carried, mut at, mut seen) in &mut drawn {
        match bearers.iter().find(|(m, ..)| m.id == carried.0).and_then(|(m, t, s)| carries(m).filter(|c| c.0 == carried.1).map(|c| held(t, c.1, s))) {
            // (Both: this runs once every body has been placed for the frame.)
            Some(to) => (*at, *seen) = (to, GlobalTransform::from(to)),
            None => commands.entity(entity).despawn(),
        }
    }
    for (m, at, strode) in &bearers {
        let Some((block, tall)) = carries(m) else { continue };
        if drawn.iter().any(|(_, c, ..)| c.0 == m.id) {
            continue;
        }
        let mesh = made.entry(block).or_insert_with(|| {
            let cells = (0..cube_sim::loose::HUNK.pow(3)).map(|i| (IVec3::new(i % 2, i / 2 % 2, i / 4), Part::Whole(block))).collect();
            let data = shape(&cells);
            meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.tangents)
                .with_inserted_indices(Indices::U32(data.indices)))
        }).clone();
        commands.spawn((Carried(m.id, block), Mesh3d(mesh), MeshMaterial3d(look.solid.clone()), held(at, tall, strode)));
    }
}

/// A piece being drawn: which; and, once it has landed, the seconds it is still drawn (the
/// chunks it went into are meshed on other threads, and for the frames that takes it was
/// nowhere: a felled tree blinked as it came to rest).
#[derive(Component)]
struct Falling(u32, Option<f32>);

fn fall(mut commands: Commands, live: Option<Res<Live>>, look: Option<Res<crate::world::Look>>, mut drawn: Query<(Entity, &mut Falling, &mut Transform)>, time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut making: Local<std::collections::HashMap<u32, bevy::tasks::Task<cube_voxel::mesh::MeshData>>>) {
    let (Some(live), Some(look)) = (live, look) else { return };
    let pieces = &live.0.loose.pieces;
    for (entity, mut falling, mut at) in &mut drawn {
        match pieces.iter().find(|p| p.id == falling.0) {
            // (A point of it that was at `p` is at pivot + shift + turn (p - pivot).)
            // (About what it turns about now: that changes as a tree's top strikes and it
            // settles about its top. Drawn about where it first stood, it was gone for the
            // second it took to settle.)
            Some(piece) => *at = Transform { translation: piece.pivot + piece.shift - piece.turn * piece.pivot, rotation: piece.turn, scale: Vec3::ONE },
            // (Come to rest as itself, it is drawn on as it lies: `Rested`. It is not in
            // the world to be drawn there.)
            None if let Some(lies) = live.0.loose.rested.iter().find(|r| r.id == falling.0) => *at = Transform { translation: lies.pivot + lies.shift - lies.turn * lies.pivot, rotation: lies.turn, scale: Vec3::ONE },
            None => {
                let left = falling.1.unwrap_or(0.25) - time.delta_secs();
                falling.1 = Some(left);
                if left <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
    // Each new piece's mesh is made on another thread (a tree is ten thousand cells, and made
    // here it was a frame of 40 ms as it came loose) and drawn when it is ready, a frame or
    // two on: the chunks it was cut from are still drawn as they were for as long.
    // (And each thing lying at rest as itself that is not drawn yet: it came to rest before
    // its shape was ready.)
    let rested = &live.0.loose.rested;
    let all: Vec<(u32, &cube_sim::loose::Cells<Part>, Vec3, Vec3, Quat)> = pieces.iter().map(|p| (p.id, &p.cells, p.pivot, p.shift, p.turn)).chain(rested.iter().map(|r| (r.id, &r.cells, r.pivot, r.shift, r.turn))).collect();
    making.retain(|id, _| all.iter().any(|p| p.0 == *id));
    for (id, cells, pivot, shift, turn) in all.iter().filter(|p| !drawn.iter().any(|(_, f, _)| f.0 == p.0)).copied() {
        match making.get_mut(&id).map(bevy::tasks::futures::check_ready) {
            Some(Some(data)) => {
                making.remove(&id);
                let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.tangents)
                    .with_inserted_indices(Indices::U32(data.indices));
                commands.spawn((Falling(id, None), Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.solid.clone()), Transform { translation: pivot + shift - turn * pivot, rotation: turn, scale: Vec3::ONE }));
            }
            Some(None) => {}
            None => {
                let cells = cells.clone();
                making.insert(id, bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { shape(&cells) }));
            }
        }
    }
}

/// A piece drawn as the world draws its cells, where it stood: a world of its own holding
/// only the piece, meshed by the world's own mesher, so that what falls is what stood and
/// what will lie. (Drawn first as plain cubes of one colour, a birch went grey as it fell.)
fn shape(cells: &cube_sim::loose::Cells<Part>) -> cube_voxel::mesh::MeshData {
    let mut alone = cube_voxel::World::default();
    for (cell, part) in cells {
        match part {
            Part::Whole(block) => alone.set(*cell, *block),
            Part::Bits(bits) => {
                for (i, block) in bits.iter().enumerate().filter(|(_, b)| **b != Block::Air) {
                    let i = i as i32;
                    alone.set_bit(*cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT), *block);
                }
            }
        }
    }
    let mut all = cube_voxel::mesh::MeshData::default();
    let chunks: Vec<IVec3> = alone.chunks.keys().copied().collect();
    for chunk in chunks {
        let data = cube_voxel::mesh::mesh_chunk(&alone, chunk, false);
        let from = all.positions.len() as u32;
        all.positions.extend(data.positions);
        all.normals.extend(data.normals);
        all.colors.extend(data.colors);
        all.uvs.extend(data.uvs);
        all.tangents.extend(data.tangents);
        all.indices.extend(data.indices.into_iter().map(|i| i + from));
    }
    all
}

/// A chip thrown up: how it is going, and the seconds it has left.
#[derive(Component)]
struct Chip(Vec3, f32);

/// What the simulation noted since last frame is heard and seen: each cut, each thing come
/// loose, each landing.
#[allow(clippy::too_many_arguments)]
fn told(mut commands: Commands, live: Option<ResMut<Live>>, bank: Option<ResMut<crate::audio::Bank>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut cube: Local<Option<Handle<Mesh>>>, mut paints: Local<std::collections::HashMap<[u8; 3], Handle<StandardMaterial>>>, mut n: Local<u32>, time: Res<Time>, mut clattered: Local<f32>) {
    // (No sound bank where nothing is heard: a sheet of a creature's clips.)
    let (Some(mut live), Some(mut bank)) = (live, bank) else { return };
    let loose = &mut live.0.loose;
    let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
    // (Nothing here matters to the world: where a chip goes is only seen.)
    let mut odd = || {
        *n = n.wrapping_mul(1664525).wrapping_add(1013904223);
        (*n >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let mut throw = |commands: &mut Commands, at: Vec3, block: Block, many: usize, reach: f32, up: f32| {
        let colour = block.colors().1;
        let key = colour.map(|c| (c * 255.0) as u8);
        let paint = paints.entry(key).or_insert_with(|| materials.add(StandardMaterial { base_color: Color::linear_rgb(colour[0], colour[1], colour[2]), perceptual_roughness: 0.95, ..default() })).clone();
        for _ in 0..many {
            let (out, size) = (Vec3::new(odd(), 0.0, odd()), 0.03 + 0.04 * odd().abs());
            commands.spawn((Chip(out * reach * 2.5 + Vec3::Y * up * (0.5 + odd().abs()), 0.7 + 0.5 * odd().abs()), Mesh3d(cube.clone()), MeshMaterial3d(paint.clone()), Transform { translation: at + out * reach * 0.3 + Vec3::Y * 0.1, scale: Vec3::splat(size), ..default() }, bevy::light::NotShadowCaster));
        }
    };
    // A blow that knocked something off (limbs, a trunk into logs, a log into hunks) is the
    // crack of wood parting, not the knock of an axe going in: the developer, of a tree
    // being worked, "when it breaks it should kind of be more obvious".
    let broke = !loose.shed.is_empty();
    for (at, taken) in loose.cuts.drain(..) {
        // (A blow that took nothing is not heard as wood, which nothing of each is.)
        if taken == [0, 0, 0] {
            continue;
        }
        let (name, block) = if taken[1] >= taken[0] && taken[1] >= taken[2] { (if broke { "split" } else { "chop" }, Block::Wood) } else if taken[2] > taken[0] { ("pick", Block::Stone) } else { ("dig", Block::Dirt) };
        bank.play(&mut commands, name, Some(at), 0.8);
        throw(&mut commands, at, block, 7, 0.4, 2.2);
    }
    // What a blow knocked off a fallen tree: thrown a little way, and gone as chips go.
    for (at, block) in loose.shed.drain(..).take(80) {
        throw(&mut commands, at, block, 2, 0.5, 1.6);
    }
    loose.shed.clear();
    for (at, cells, over) in loose.begun.drain(..) {
        if over && cells > 200 {
            let from = bank.carried(at);
            bank.play(&mut commands, "crack", Some(from), 1.0);
        }
    }
    for landed in loose.heard.drain(..) {
        let size = (landed.cells as f32).cbrt() * cube_voxel::CELL;
        if std::env::var_os("LOOSE_TRACE").is_some() {
            println!("loose: landed {} cells at {:.1} m/s, {} put", landed.cells, landed.speed, landed.put);
        }
        // A heavy thing is a crash, a lump a clatter; louder the more it weighs and the
        // faster it came (`matter`: a crown of leaf is as many cells as a boulder, and was
        // as loud).
        let loud = (landed.mass * landed.speed / 6000.0).sqrt().clamp(0.15, 1.0);
        // (A crown that breaks is ninety limbs landing in half a second: a clatter is heard a
        // few times in that, not ninety.)
        let now = time.elapsed_secs();
        // (The crash is a tree's: its trunk striking and its branches breaking. A log of
        // half a tonne played it too, three times as a trunk was bucked.)
        if landed.along.is_some() {
            let from = bank.carried(landed.at);
            bank.play(&mut commands, "crash", Some(from), loud);
        } else if now - *clattered > 0.12 {
            *clattered = now;
            // (Wood come down is a knock, stone a clatter; lower the bigger it is.)
            let from = bank.carried(landed.at);
            bank.pitched(&mut commands, if landed.wood { "thud" } else { "rubble" }, Some(from), loud * if landed.wood { 0.9 } else { 0.6 }, (1.6 - size * 0.5).clamp(0.8, 1.5));
        }
        throw(&mut commands, landed.at.with_y(landed.at.y - size * 0.3), Block::Dirt, (6.0 + size * 10.0).min(40.0) as usize, size.max(0.5), 2.5);
    }
}

/// Chips fly, fall and are gone.
fn chips(mut commands: Commands, time: Res<Time>, mut flying: Query<(Entity, &mut Chip, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut chip, mut at) in &mut flying {
        chip.0.y -= 14.0 * dt;
        at.translation += chip.0 * dt;
        at.rotate_local_x(6.0 * dt);
        chip.1 -= dt;
        if chip.1 < 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
