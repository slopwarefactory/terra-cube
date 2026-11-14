//! Feet are put on the ground that is there. A clip is made for flat ground and the ground
//! is blocks and slopes: a foot the clip sets down on the level is in the air on the downhill
//! side and in the hill on the other. After the clip has posed the body and before the pose
//! is used, each leg is bent (two bones, the knee the way it already bends) so its foot is as
//! far above the ground under it as the clip had it above the level, and the body is moved
//! up or down so that no leg has to be longer than it is. What goes on four legs is first
//! pitched to the slope it stands on, nose up a rise and down a fall, so its legs have only
//! the unevenness left to take up.
//!
//! Which bones are a leg is forge's to say, for every creature alike (`leg0`, `leg1`… in its
//! `meta.toml`, from the roles it reads off the skeleton): nothing here knows a biped from a
//! deer.

use crate::creatures::{Creature, Registry};
use crate::motion::{Live, Mover};
use crate::world::Terrain;
use bevy::math::Affine3A;
use bevy::prelude::*;

/// How steeply the ground may fall between a body's hind feet and its fore feet and still be
/// a slope it pitches to, as a rise over a run: steeper is an edge it stands at.
const LEDGE: f32 = 0.8;

pub struct FeetPlugin;

impl Plugin for FeetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, find).add_systems(PostUpdate, plant.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate));
    }
}

/// A creature's legs: each its hip, its knee and the joint that stands on the ground; the bone
/// its whole body hangs from; and how much of the planting is on, 0 to 1 (eased, so leaving
/// the ground and coming back to it are not a snap).
#[derive(Component)]
pub struct Legs {
    pub legs: Vec<[Entity; 3]>,
    pub(crate) root: Option<Entity>,
    on: f32,
    /// How far the body was pitched to the slope last frame: it comes after the slope's own.
    pitch: f32,
    /// How far the body was moved up or down last frame: it comes after what the feet ask,
    /// which changes at a stroke as a foot swings over the edge of a block. (The feet
    /// themselves do not: eased after the ground, they were in it.)
    shift: f32,
    /// Which way the body is long, once read off its feet.
    long: Option<Vec3>,
    /// Every bone this moved last frame: how the clip had it, and how it was left. A clip
    /// does not pose every bone every frame (a standing animal's root has no track at all),
    /// and a bone the clip has not posed again is still as it was left: moved again from
    /// there, a deer standing on a slope sank a little further into it each frame.
    moved: Vec<(Entity, Transform, Transform)>,
}

/// How far a foot may be moved up or down from where the clip has it, in the creature's heights.
const REACH: f32 = 0.17;
/// Feet are planted for what is this near the eye, metres: further off nobody can tell.
const NEAR: f32 = 30.0;

/// Finds each new creature's leg bones by the names forge wrote.
fn find(mut commands: Commands, registry: Res<Registry>, creatures: Query<(Entity, &Creature), (With<Mover>, Without<Legs>)>, children: Query<&Children>, names: Query<&Name>) {
    for (entity, creature) in &creatures {
        let bones = &registry.0[creature.def].meta.bones;
        if children.iter_descendants(entity).next().is_none() {
            continue;
        }
        // (A name that is on two bones is no use: forge leaves such a leg out.)
        let named = |name: &str| {
            let mut found = children.iter_descendants(entity).filter(|e| names.get(*e).is_ok_and(|n| n.as_str() == name));
            found.next().filter(|_| found.next().is_none())
        };
        let legs: Vec<[Entity; 3]> = (0..8)
            .filter_map(|i| bones.get(&format!("leg{i}")))
            .filter_map(|chain| {
                let mut parts = chain.split('>').map(named);
                Some([parts.next()??, parts.next()??, parts.next()??])
            })
            .collect();
        commands.entity(entity).insert(Legs { legs, root: bones.get("root").and_then(|n| named(n)), on: 0.0, pitch: 0.0, shift: 0.0, long: None, moved: Vec::new() });
    }
}

/// Where a bone is in the world this frame: its own place under each of its parents' in turn,
/// up to the creature, whose place is already this frame's (the bones' own are last frame's
/// until the engine works them out, after this).
pub(crate) fn world_of(bone: Entity, top: Entity, top_at: &Transform, parents: &Query<&ChildOf>, poses: &Query<&mut Transform, Without<Mover>>) -> Option<Affine3A> {
    let mut chain = Vec::new();
    let mut at = bone;
    while at != top {
        chain.push(poses.get(at).ok()?.compute_affine());
        at = parents.get(at).ok()?.parent();
        if chain.len() > 64 {
            return None;
        }
    }
    Some(chain.iter().rev().fold(top_at.compute_affine(), |world, local| world * *local))
}

fn turn_of(world: &Affine3A) -> Quat {
    Quat::from_mat3a(&bevy::math::Mat3A::from_cols(world.matrix3.x_axis.normalize(), world.matrix3.y_axis.normalize(), world.matrix3.z_axis.normalize()))
}

/// Turns a bone by `turn`, a turn in the world, from where its pose has it.
pub(crate) fn turn_bone(bone: Entity, turn: Quat, top: Entity, top_at: &Transform, parents: &Query<&ChildOf>, poses: &mut Query<&mut Transform, Without<Mover>>) -> Option<()> {
    let parent = parents.get(bone).ok()?.parent();
    let frame = if parent == top { top_at.rotation } else { turn_of(&world_of(parent, top, top_at, parents, poses)?) };
    let mut pose = poses.get_mut(bone).ok()?;
    pose.rotation = (frame.inverse() * turn * frame * pose.rotation).normalize();
    Some(())
}

/// The height of the ground as it is drawn under a point, looked for a little above and below
/// the height given.
pub fn ground_at(terrain: &Terrain, x: f32, z: f32, near: f32) -> Option<f32> {
    let level = (near / cube_voxel::CELL).round() as i32;
    [0, 1, -1, 2, -2].into_iter().find_map(|d| {
        let cell = IVec3::new((x / cube_voxel::CELL).floor() as i32, level + d, (z / cube_voxel::CELL).floor() as i32);
        (terrain.0.get(cell - IVec3::Y).ground() && !terrain.0.get(cell).ground()).then(|| cube_voxel::mesh::eased_under(&terrain.0, Vec3::new(x, (level + d) as f32 * cube_voxel::CELL, z))).flatten()
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plant(
    time: Res<Time>,
    args: Res<crate::Args>,
    live: Option<Res<Live>>,
    terrain: Option<Res<Terrain>>,
    registry: Res<Registry>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut creatures: Query<(Entity, &Mover, &Creature, &Transform, &mut Legs)>,
    parents: Query<&ChildOf>,
    mut poses: Query<&mut Transform, Without<Mover>>,
) {
    let (Some(live), Some(terrain)) = (live, terrain) else { return };
    if args.loose_feet {
        return;
    }
    let eye = cameras.iter().next().map(|c| c.translation());
    for (entity, mover, creature, at, mut legs) in &mut creatures {
        // What the clip has not posed again since it was moved is put back as the clip had it.
        for (bone, clip, left) in std::mem::take(&mut legs.moved) {
            if let Ok(mut pose) = poses.get_mut(bone) {
                if pose.translation == left.translation {
                    pose.translation = clip.translation;
                }
                if pose.rotation == left.rotation {
                    pose.rotation = clip.rotation;
                }
            }
        }
        let Some(agent) = live.agent(mover) else { continue };
        if legs.legs.is_empty() || eye.is_some_and(|e| e.distance(at.translation) > NEAR) {
            continue;
        }
        // Only what stands on the ground has its feet put on it; the change is eased.
        let standing = agent.body.grounded && !agent.body.swimming && agent.dead.is_none();
        legs.on = (legs.on + (if standing { 1.0 } else { -1.0 }) * time.delta_secs() * 6.0).clamp(0.0, 1.0);
        if legs.on <= 0.0 {
            continue;
        }
        let touched: Vec<Entity> = legs.legs.iter().flat_map(|[hip, knee, _]| [*hip, *knee]).chain(legs.root).collect();
        let before: Vec<(Entity, Transform)> = touched.iter().filter_map(|b| poses.get(*b).ok().map(|p| (*b, *p))).collect();
        let four = legs.legs.len() >= 4;
        let reach = registry.0[creature.def].meta.height * REACH * at.scale.y * if four { 1.8 } else { 1.0 };
        let ease = 1.0 - (-12.0 * time.delta_secs()).exp();
        let chains = legs.legs.clone();
        let tips = |poses: &Query<&mut Transform, Without<Mover>>| -> Option<Vec<Vec3>> { chains.iter().map(|[.., tip]| world_of(*tip, entity, at, &parents, poses).map(|w| Vec3::from(w.translation))).collect() };
        // How high the clip has each foot above the level it was made for (the height the
        // creature itself is drawn at): what is to be kept.
        let Some(clip) = tips(&poses) else { continue };
        let above: Vec<f32> = clip.iter().map(|t| t.y - at.translation.y).collect();
        let ground = |p: Vec3| ground_at(&terrain, p.x, p.z, at.translation.y).map_or(0.0, |g| g - at.translation.y);
        // On four legs: the slope from the feet behind to the feet ahead, and the body
        // turned to it about the bone it hangs from.
        if let (true, Some(root)) = (four, legs.root) {
            // (Which way it is long is read off the feet themselves: fore and hind are further
            // apart than left and right.)
            let local: Vec<Vec3> = clip.iter().map(|t| at.rotation.inverse() * (*t - at.translation)).collect();
            let spread = |of: fn(&Vec3) -> f32| local.iter().map(of).fold(f32::MIN, f32::max) - local.iter().map(of).fold(f32::MAX, f32::min);
            // (Once: a rabbit's feet in a hop are now further apart one way and now the other,
            // and it was pitched about one and then the other, twenty degrees in a frame.)
            let long = *legs.long.get_or_insert_with(|| if spread(|v| v.x) > spread(|v| v.z) { Vec3::X } else { Vec3::Z });
            let along: Vec<f32> = local.iter().map(|v| v.dot(long)).collect();
            let middle = along.iter().sum::<f32>() / along.len() as f32;
            let side = |ahead: bool| -> Option<(f32, f32)> {
                let of: Vec<(f32, f32)> = clip.iter().zip(&along).filter(|(_, z)| (**z > middle) == ahead).map(|(t, z)| (*z, ground(*t))).collect();
                (!of.is_empty()).then(|| (of.iter().map(|o| o.0).sum::<f32>() / of.len() as f32, of.iter().map(|o| o.1).sum::<f32>() / of.len() as f32))
            };
            if let (Some((za, ga)), Some((zb, gb))) = (side(true), side(false)) && (za - zb).abs() > 1e-3 {
                // (A ledge is not a slope: where the ground under its fore feet is further
                // below that under its hind feet than they are apart by four fifths and
                // more, it stands at an edge, and stands level. Pitched to that as to a
                // hillside, a rabbit at the rim of a terrace stood nose down at 24 degrees
                // with its fore feet reaching down the wall, for as long as it stood there.)
                let rise = (ga - gb) / (za - zb);
                // (Less and less of it from there to half as steep again, not all or none: on
                // blocks a climb is now a slope and now an edge from one stride to the next,
                // and a moose nodded 280 degrees a second between them. And it comes to
                // the new pitch over a third of a second.)
                let edge = 1.0 - ((rise.abs() - LEDGE) / (LEDGE * 0.5)).clamp(0.0, 1.0);
                let slope = (-rise.atan()).clamp(-0.42, 0.42) * edge * legs.on;
                legs.pitch += (slope - legs.pitch) * (1.0 - (-5.0 * time.delta_secs()).exp());
                turn_bone(root, Quat::from_axis_angle(Vec3::Y.cross(at.rotation * long), legs.pitch), entity, at, &parents, &mut poses);
            }
        }
        // How far each foot must rise or fall from where it now is.
        let Some(now) = tips(&poses) else { continue };
        let mut rises = Vec::with_capacity(now.len());
        for (i, tip) in now.iter().enumerate() {
            let want = ground(*tip).clamp(-reach, reach) * legs.on;
            rises.push(at.translation.y + above[i] + want - tip.y);
        }
        // The body goes up or down by the least of them: then every leg has only to bend,
        // and none is asked to be longer than it is.
        // (Down until the foot over the highest ground stands on it, and no further: the body
        // stands on the highest block under any part of it, which on a slope or at an edge
        // is higher than the ground under either foot, and its feet hung a hand over the
        // hill. Let down as far as the lowest foot wanted, as it was until 2026-11-12, a
        // body with one foot past the edge of a block went down 30 cm onto that foot and the
        // other leg folded under it. A foot its leg cannot reach down to hangs, as one does
        // over a kerb.)
        let (least, most) = (rises.iter().copied().fold(f32::MAX, f32::min), rises.iter().copied().fold(f32::MIN, f32::max));
        let shift = (if least > 0.0 { least } else { most.min(0.0) }).clamp(-reach, reach * 0.6);
        legs.shift += (shift - legs.shift) * ease;
        let shift = legs.shift;
        if std::env::var_os("FEET_TRACE").is_some() {
            let grounds: Vec<f32> = now.iter().map(|t| ground(*t)).collect();
            println!("plant reach={reach:.2} on={:.2} above={above:.2?} ground={grounds:.2?} rises={rises:.2?} shift={shift:.2}", legs.on);
        }
        if let Some(root) = legs.root.filter(|_| shift.abs() > 1e-4)
            && let Some(parent) = parents.get(root).ok().map(|p| p.parent())
            && let Some(frame) = if parent == entity { Some(at.compute_affine()) } else { world_of(parent, entity, at, &parents, &poses) }
            && let Ok(mut pose) = poses.get_mut(root)
        {
            pose.translation += Vec3::from(frame.matrix3.inverse() * bevy::math::Vec3A::from(Vec3::Y * shift));
        }
        for ([hip, knee, tip], rise) in chains.iter().copied().zip(rises) {
            let rise = rise - shift;
            if rise.abs() < 1e-4 {
                continue;
            }
            let (Some(a), Some(b), Some(p)) = (
                world_of(hip, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
                world_of(knee, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
                world_of(tip, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
            ) else {
                continue;
            };
            let target = p + Vec3::Y * rise;
            let (upper, lower) = (a.distance(b), b.distance(p));
            if upper < 1e-5 || lower < 1e-5 {
                continue;
            }
            // The knee bends, the way it is bent already, until the foot is as far from the
            // hip as it has to be; then the whole leg swings to put it there.
            let far = a.distance(target).clamp((upper - lower).abs() + 1e-4, (upper + lower) * 0.999);
            let now = (a - b).angle_between(p - b);
            let want = ((upper * upper + lower * lower - far * far) / (2.0 * upper * lower)).clamp(-1.0, 1.0).acos();
            let axis = (a - b).cross(p - b).try_normalize().unwrap_or(at.rotation * Vec3::X);
            let bend = Quat::from_axis_angle(axis, want - now);
            if turn_bone(knee, bend, entity, at, &parents, &mut poses).is_none() {
                continue;
            }
            let bent = b + bend * (p - b);
            let (Some(from), Some(to)) = ((bent - a).try_normalize(), (target - a).try_normalize()) else { continue };
            turn_bone(hip, Quat::from_rotation_arc(from, to), entity, at, &parents, &mut poses);
        }
        legs.moved = before.into_iter().filter_map(|(bone, clip)| poses.get(bone).ok().map(|left| (bone, clip, *left))).filter(|(_, clip, left)| clip != left).collect();
    }
}
