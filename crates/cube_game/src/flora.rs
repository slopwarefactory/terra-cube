//! What grows, drawn as what it is and not as the cubes that stand for it: trunks that swell
//! into their roots, limbs that wander and taper, and leaf in masses, sprigs of leaflets
//! near to and cards far off, that the wind moves. The simulation still has every tree as
//! cells (a body is stopped by a trunk, a squirrel will not climb a crown);
//! `cube_sim::plants` says, as it grows each thing, what shape it grew (`plants::Form`: its
//! limbs and its clumps of leaf), and that is drawn here in place of the cells.
//!
//! That is `--shapes` now, kept to compare. The default is the same forms as small cubes
//! of many sizes (the end of this file): the world is cuboids, all of it.

use crate::world::{Look, Terrain};
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::{plants::Form, terrain};
use cube_voxel::{
    Block, CELL,
    texture::{Grain, MEAN, SPAN},
};
use std::collections::{HashMap, HashSet};

pub struct FloraPlugin;

impl Plugin for FloraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Forms>().init_resource::<Grown>().add_systems(Update, grow);
    }
}

/// The shapes of what grows in each region whose cells are made: put here when the region is
/// made and taken away when it is dropped (`world::regions`).
#[derive(Resource, Default)]
pub struct Forms(pub HashMap<(i32, i32), std::sync::Arc<Form>>);

/// The meshes made from them.
#[derive(Resource, Default)]
pub struct Grown(HashMap<(i32, i32), (Entity, u8)>, HashMap<(i32, i32), (u8, bevy::tasks::Task<Shape>)>, HashSet<(i32, i32)>);

impl Grown {
    /// A region's shapes have changed (a tree of it was cut into and is cells now): its mesh
    /// is made again, and what it showed stays until that is ready.
    pub fn again(&mut self, region: (i32, i32)) {
        self.1.remove(&region);
        self.2.insert(region);
    }
    /// Whether a region's trees and rocks have their meshes.
    pub fn has(&self, region: (i32, i32)) -> bool {
        self.0.contains_key(&region)
    }
}

/// How much of a leaf mass's surface its cards cover; how big a card is.
const COVER: f32 = 1.3;
const CARD: f32 = 0.8;
/// Metres from the camera within which leaf is drawn as sprigs.
const FINE: f32 = 5.0;
/// And beyond this, as a few broad cards and the largest masses' bodies: a tenth of the points.
const MID: f32 = 46.0;
/// And beyond this, as its bare outline: a trunk, and one box to each great mass of leaf.
/// What is made is made out to 208 m so the land is seen far; at the third level's faces
/// that was half as many again as everything nearer.
const FAR: f32 = 104.0;

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E3779B1) ^ b.wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

#[derive(Default)]
pub struct Shape {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    /// How high each corner stands over the ground under it, metres (in x): what a whole
    /// tree bends by in the wind, its top most. Nothing for a stone.
    high: Vec<[f32; 2]>,
    indices: Vec<u32>,
    /// A notch to be cut out of the wood drawn: `cube_sim::loose::Thing::notch`.
    notch: Option<Notch>,
}

/// The middle of a trunk where an axe is at it, which way the blows go in, how thick it is
/// there and how deep they have got.
pub type Notch = (Vec3, Vec3, f32, f32);

/// Whether a place is in the wedge an axe has cut: open on the side the blows come from, as
/// high as a hand and a half at its mouth, closing to nothing as deep as they have got.
fn wedge((middle, into, thick, deep): Notch, p: Vec3) -> bool {
    let d = p - (middle + Vec3::Y * 0.09);
    // (From outside the bark, which swells past the trunk's own thickness.)
    let (inward, reach) = (d.dot(into) + thick * 1.6, deep * 1.2 + thick * 0.6);
    inward < reach && d.y.abs() <= 0.05 + 0.17 * (1.0 - (inward / reach).clamp(0.0, 1.0))
}

impl Shape {
    /// `free`: how far it gives to the wind, 0 held to 1 loose (the mesher's convention, in
    /// the colour's alpha).
    fn point(&mut self, at: Vec3, normal: Vec3, colour: [f32; 3], uv: [f32; 2], free: f32) -> u32 {
        self.positions.push(at.to_array());
        self.normals.push(normal.to_array());
        self.colors.push([colour[0] / MEAN, colour[1] / MEAN, colour[2] / MEAN, 1.0 - free]);
        self.uvs.push(uv);
        self.tangents.push([0.0, 1.0, 0.0, 1.0]);
        self.positions.len() as u32 - 1
    }

    /// A limb from `a` to `b`, tapering: not a pipe but wood, which wanders a little along its
    /// length, is ridged along the grain (more the thicker it is, and most where a trunk
    /// meets the ground), and ends in a face, cut wood where nothing grows on from it.
    fn limb(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block, n: u32, cut: [bool; 2], roots: u8) {
        let along = (b - a).normalize_or(Vec3::Y);
        let side = along.any_orthonormal_vector();
        let other = along.cross(side);
        let colour = bark.colors().1;
        let grain = if bark == Block::Birch { Grain::BirchBark } else { Grain::Bark };
        let (ra, rb) = (ra.max(0.03), rb.max(0.02));
        let (thick, length) = (ra.max(rb), a.distance(b));
        let sides = if roots > 0 { 24 } else if thick > 0.28 { 12 } else if thick > 0.1 { 8 } else { 5 };
        // (Never more than a tile of bark between two rings: see where the picture is laid.)
        let rings = ((length / (thick * 2.5).max(0.7)).ceil() as usize).clamp(1, 7).max((length / 1.9).ceil() as usize);
        // Where its rings are, along it. A rooted trunk has more of them low down, where it
        // spreads: it begins half a metre under the ground (`plants::bole`).
        let mut stations: Vec<f32> = (0..=rings).map(|j| j as f32 / rings as f32).collect();
        const UNDER: f32 = cube_sim::plants::Limb::UNDER;
        if roots > 0 {
            let low: Vec<f32> = [0.0f32, 0.25, 0.6, 1.1, 1.8, 2.8, 4.2].iter().map(|h| (UNDER - 0.12 + h * ra) / length).filter(|t| *t < 0.7).collect();
            let last = low.last().copied().unwrap_or(0.0);
            stations.retain(|t| *t == 0.0 || *t > last + 0.05);
            stations.splice(1..1, low);
        }
        let rings = stations.len() - 1;
        // The bark's picture once round; each limb begins somewhere of its own in it.
        let from = hash(n, 1) * SPAN as f32;
        // A twig gives a little to the wind; a trunk none.
        let free = |r: f32| ((0.16 - r) / 0.16).clamp(0.0, 1.0) * 0.25;
        // Ridges: three to five along a thick limb, deepest at the foot of an upright one.
        let (lobes, phase) = (if roots > 0 { roots as u32 } else { 3 + (hash(n, 2) * 3.0) as u32 }, hash(n, 3) * std::f32::consts::TAU);
        let upright = along.y > 0.8 && thick > 0.15;
        let start = self.positions.len() as u32;
        let ring = sides as u32 + 1;
        let mut middles = [a, b];
        // The bark's picture at its true size along the limb, whatever its length: each
        // stretch between two rings takes its own piece of the tile, so every ring but the
        // ends is laid twice, once as the top of one stretch and once as the foot of the
        // next. One tile pulled the length of a pine was bark drawn out five times too long.
        let cells = length / CELL;
        let room = SPAN as f32 - 0.04;
        let piece = |s: usize| {
            let (lo, long) = ((stations[s] * cells + from).rem_euclid(room), ((stations[s + 1] - stations[s]) * cells).min(room));
            let lo = if lo + long > room { room - long } else { lo };
            (lo + 0.02, lo + long + 0.02)
        };
        for (j, t) in stations.iter().copied().enumerate() {
          for along in [(j > 0).then(|| piece(j - 1).1), (j < rings).then(|| piece(j).0)].into_iter().flatten() {
            let hold = (t * std::f32::consts::PI).sin();
            let wander = (side * (hash(n, 20 + j as u32) - 0.5) + other * (hash(n, 40 + j as u32) - 0.5)) * thick.min(0.35) * 0.7 * hold;
            let at = a.lerp(b, t) + wander;
            let r = ra + (rb - ra) * t;
            if j == 0 {
                middles[0] = at;
            } else if j == rings {
                middles[1] = at;
            }
            // How far up from its low end, in its own thicknesses: the flare dies out by two.
            let foot = if upright { (1.0 - t * length / (thick * 4.0)).clamp(0.0, 1.0) } else { 0.0 };
            let deep = (0.05 + 0.22 * foot * foot) * (thick / 0.2).min(1.0);
            for k in 0..=sides {
                let turn = (k % sides) as f32 / sides as f32 * std::f32::consts::TAU;
                let out = side * turn.cos() + other * turn.sin();
                let mut ridge = 1.0 + deep * (turn * lobes as f32 + phase).cos() + 0.04 * (hash(n, 60 + (k % sides) as u32 + 16 * j as u32) - 0.5);
                if roots > 0 {
                    // The foot: the whole trunk swells toward the ground, and along each root
                    // it runs out in a sharp-backed buttress, each to its own length, the
                    // bark between them drawn in.
                    let high = ((t * length - UNDER + 0.12) / ra).max(0.0);
                    let wave = (turn * lobes as f32 + phase + 0.5 * (turn * 2.0 + phase).sin()).cos();
                    let back = wave.max(0.0).powf(2.2);
                    let which = ((turn * lobes as f32 + phase) / std::f32::consts::TAU).round() as i32;
                    // (A slender trunk's roots are slight: a birch does not stand on a star.)
                    let reach = (1.5 + 1.6 * hash(n, 300 + which.rem_euclid(lobes as i32) as u32)) * (ra / 0.35).clamp(0.3, 1.0);
                    ridge = 1.0 + 0.45 * (-high / 1.6).exp() + reach * back * (-high / 0.75).exp() - 0.1 * (1.0 - back) * (-high / 2.0).exp();
                }
                // Darker in the furrows and toward the ground, where it is damp.
                let shade = (0.8 + 0.25 * hash(n, 7 + (k % sides) as u32)) * (0.86 + 0.6 * (ridge - 1.0).clamp(-0.2, 0.2)) * (1.0 - 0.25 * foot);
                self.point(at + out * r * ridge, out, colour.map(|c| c * shade), grain.uv(k as f32 / sides as f32 * (SPAN as f32 - 0.02) + 0.01, along), free(r));
            }
          }
        }
        for j in 0..rings as u32 {
            for k in 0..sides as u32 {
                let (lo, hi) = (start + 2 * j * ring + k, start + (2 * j + 1) * ring + k);
                self.indices.extend([lo, hi, hi + 1, lo, hi + 1, lo + 1]);
            }
        }
        // The ends: bark grown over where a limb goes on, pale cut wood where it does not.
        for (end, r) in [(0usize, ra), (1, rb)] {
            if r < 0.07 {
                continue;
            }
            let (face, wood) = (if end == 0 { -along } else { along }, cut[end] && r > 0.12);
            let (paint, tint) = if wood { (Grain::EndGrain, bark.colors().0.map(|c| c * 1.5)) } else { (grain, colour.map(|c| c * 0.8)) };
            // Where another limb goes on from here the end is rounded over, so a bend has no gap.
            let dome = if cut[end] { 0.08 } else { 0.6 };
            let across = (2.0 * r / CELL).min(SPAN as f32 - 0.1);
            let first = start + if end == 0 { 0 } else { (2 * rings as u32 - 1) * ring };
            let hub = self.point(middles[end] + face * r * dome, face, tint, paint.uv(0.05 + across / 2.0, 0.05 + across / 2.0), 0.0);
            let rim = self.positions.len() as u32;
            for k in 0..=sides as u32 {
                let turn = (k % sides as u32) as f32 / sides as f32 * std::f32::consts::TAU;
                let at = Vec3::from(self.positions[(first + k) as usize]);
                self.point(at, face, tint.map(|c| c * 0.9), paint.uv(0.05 + across * (0.5 + 0.5 * turn.cos()), 0.05 + across * (0.5 + 0.5 * turn.sin())), 0.0);
            }
            for k in 0..sides as u32 {
                if end == 0 {
                    self.indices.extend([hub, rim + k, rim + k + 1]);
                } else {
                    self.indices.extend([hub, rim + k + 1, rim + k]);
                }
            }
        }
    }

    /// A mass of leaf: cards over the outside of an ellipsoid and a few within, each a piece
    /// of the leaf picture with its gaps, lit as the mass it is part of and not as a flat card.
    fn clump(&mut self, middle: Vec3, radii: Vec3, leaf: Block, n: u32, ball: &[[Vec3; 3]], level: u8) {
        let level = level.min(2);
        let (top, under) = leaf.colors();
        // No two masses the one green: each a little yellower or bluer, lighter or darker.
        let warm = hash(n, 90) - 0.5;
        let tint = |c: [f32; 3], w: f32| [c[0] * (1.0 + 0.5 * w), c[1] * (1.0 + 0.16 * w), c[2] * (1.0 - 0.4 * w)];
        let (top, under) = (tint(top, warm), tint(under, warm));
        // A spray of needles lies flat along its bough; a mass of leaf is round.
        let spray = radii.y < radii.x.min(radii.z) * 0.6;
        let grain = if leaf == Block::Needles { Grain::Needles } else { Grain::Leaves };
        // The body of it: a dark faceted mass inside, so a crown has depth and the sky does
        // not show through its middle. The cards are its ragged outside.
        let fine = level == 0;
        // (Far off, only a mass big enough to be seen as one has a body.)
        if radii.min_element() > if level == 2 { 1.1 } else { 0.45 } {
            let lump = |d: Vec3| middle + d * radii * (0.62 + 0.16 * hash(n, 200 + ((d.x * 7.0 + d.y * 13.0 + d.z * 29.0) * 9.0) as i32 as u32));
            for corners in ball {
                let at = corners.map(lump);
                let normal = (at[1] - at[0]).cross(at[2] - at[0]).normalize_or(Vec3::Y);
                let shade = 0.42 + 0.3 * (normal.y * 0.5 + 0.5);
                let start = self.positions.len() as u32;
                for (i, p) in at.into_iter().enumerate() {
                    self.point(p, normal, under.map(|c| c * shade), grain.uv(1.0 + 2.5 * (i & 1) as f32, 1.0 + 2.5 * (i >> 1) as f32), 0.3);
                }
                self.indices.extend([start, start + 1, start + 2]);
            }
        }
        // The surface of an ellipsoid, near enough.
        let area = 4.0 * std::f32::consts::PI * ((radii.x * radii.y).powf(1.6) + (radii.x * radii.z).powf(1.6) + (radii.y * radii.z).powf(1.6)).powf(1.0 / 1.6) / 3.0f32.powf(1.0 / 1.6);
        let size = CARD.min(radii.min_element() * 1.1).max(0.3);
        let cards = ((area * COVER * [0.7, 0.28, 0.09][level as usize] / (size * size)) as u32).clamp([5, 2, 1][level as usize], 300);
        for i in 0..cards {
            let roll = |k: u32| hash(n.wrapping_mul(977).wrapping_add(i), k);
            // A point on the ball, pushed out to the surface or a little inside it.
            let (turn, up) = (roll(1) * std::f32::consts::TAU, roll(2) * 2.0 - 1.0);
            let flat = (1.0 - up * up).sqrt();
            let out = Vec3::new(turn.cos() * flat, up, turn.sin() * flat);
            let deep = 0.62 + 0.38 * roll(3).sqrt();
            let at = middle + out * radii * deep;
            // Facing mostly outward, tipped any way; lit as if it faced outward and a little up.
            let lean = Vec3::new(roll(4) - 0.5, roll(5) - 0.5, roll(6) - 0.5);
            let face = if spray { (Vec3::Y + out * Vec3::new(0.7, 0.0, 0.7) + lean * 0.7).normalize_or(Vec3::Y) } else { (out + lean * 1.2).normalize_or(Vec3::Y) };
            let (u, v) = (face.any_orthonormal_vector(), face.cross(face.any_orthonormal_vector()));
            let spin = roll(7) * std::f32::consts::TAU;
            let (u, v) = (u * spin.cos() + v * spin.sin(), v * spin.cos() - u * spin.sin());
            let lit = (out * (radii.max_element() / radii) + Vec3::Y * 0.6).normalize_or(Vec3::Y);
            // Deeper in and lower down is in the crown's own shade.
            let shade = (0.5 + 0.6 * (deep - 0.62) / 0.38) * (0.82 + 0.22 * (up * 0.5 + 0.5)) * (0.9 + 0.2 * roll(8));
            let colour = tint(if up > -0.2 { top } else { under }, (roll(12) - 0.5) * 0.7).map(|c| c * shade);
            // A piece of the picture as big as the card, anywhere in the tile.
            let span = (size / CELL).min(SPAN as f32 * 0.5);
            let (ou, ov) = (roll(9) * (SPAN as f32 - span - 0.02) + 0.01, roll(10) * (SPAN as f32 - span - 0.02) + 0.01);
            let half = size * (0.75 + 0.5 * roll(11)) * 0.5;
            if !fine {
                // Far off: one pointed card, larger, for the few there are.
                let start = self.positions.len() as u32;
                for (su, sv) in [(-1.3f32, 0.0f32), (0.0, -0.62), (1.3, 0.0), (0.0, 0.62)] {
                    self.point(at + (u * su + v * sv) * half * [1.0, 1.8, 2.8][level as usize], lit, colour, grain.uv(ou + (su / 2.6 + 0.5) * span, ov + (sv / 1.24 + 0.5) * span), 0.45);
                }
                self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                continue;
            }
            // Near to: a sprig. Leaflets fan from where it joins its twig, each pointed at
            // both ends and a little cupped; needles in a wider, narrower fan, as a bough's
            // end is. One flat card was a shard of paper however it was painted.
            let (blades, fan, wide) = if leaf == Block::Needles { (5, 1.9f32, 0.13f32) } else { (3, 1.25, 0.3) };
            let root = at - u * half * 1.1;
            for blade in 0..blades {
                let turn = (blade as f32 / (blades - 1) as f32 - 0.5) * fan + (roll(20 + blade) - 0.5) * 0.25;
                let (along, across) = (u * turn.cos() + v * turn.sin(), v * turn.cos() - u * turn.sin());
                let long = half * 2.3 * (1.0 - 0.3 * (turn / fan * 2.0).abs()) * (0.85 + 0.3 * roll(30 + blade));
                let cup = face * long * 0.08;
                let glow = 0.88 + 0.24 * roll(40 + blade);
                let start = self.positions.len() as u32;
                for (k, (sa, sc, up)) in [(0.0f32, 0.0f32, 0.0f32), (0.42, -wide, 1.0), (1.0, 0.0, 0.0), (0.42, wide, 1.0)].into_iter().enumerate() {
                    let _ = k;
                    self.point(root + along * sa * long + across * sc * long + cup * up, lit, colour.map(|c| c * glow), grain.uv(ou + sa * span, ov + (sc / wide * 0.5 + 0.5) * span * 0.5), 0.3 + 0.3 * sa);
                }
                self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
    }
}

impl Shape {
    /// A piece of rock as the shape it is: a ball of points, each drawn in to the nearest of
    /// the faces struck off the stone, and every facet lit flat, as broken rock is.
    fn stone(&mut self, stone: &cube_sim::plants::Stone, ball: &[[Vec3; 3]], n: u32) {
        let (sin, cos) = stone.turn.sin_cos();
        let (top, side) = stone.kind.colors();
        // Where the stone's surface is along a direction (in its own unit space), in the world.
        let surface = |d: Vec3| {
            let most = stone.faces.iter().filter(|(normal, _)| d.dot(*normal) > 1e-4).map(|(normal, far)| far / d.dot(*normal)).fold(1.0f32, f32::min);
            let q = d * most * stone.radii;
            stone.middle + Vec3::new(q.x * cos - q.z * sin, q.y, q.x * sin + q.z * cos)
        };
        for (f, corners) in ball.iter().enumerate() {
            let at = corners.map(surface);
            let normal = (at[1] - at[0]).cross(at[2] - at[0]).normalize_or(Vec3::Y);
            // Each facet a little of its own shade; tops paler, as weathered stone is.
            let shade = 0.85 + 0.3 * hash(n.wrapping_mul(131).wrapping_add(f as u32 / 4), 3);
            let colour = if normal.y > 0.55 { top } else { side }.map(|c| c * shade);
            let grain = if normal.y > 0.55 { Grain::Rock } else { Grain::Strata };
            let start = self.positions.len() as u32;
            for p in at {
                // The stone's picture by where the point is, kept inside its tile.
                let (u, v) = if normal.y.abs() > 0.55 { (p.x, p.z) } else if normal.x.abs() > normal.z.abs() { (p.z, -p.y) } else { (p.x, -p.y) };
                let within = |m: f32| ((m - (if normal.y.abs() > 0.55 { at[0].x } else { at[0].y }).floor()) / CELL).rem_euclid(SPAN as f32 - 0.1) + 0.05;
                self.point(p, normal, colour, grain.uv(within(u), within(v)), 0.0);
            }
            self.indices.extend([start, start + 1, start + 2]);
        }
    }
}

/// The faces of a ball of 320 triangles, each its three corners as directions.
fn ball() -> Vec<[Vec3; 3]> {
    ball_of(1)
}

/// The faces of a ball divided so many times.
fn ball_of(times: u32) -> Vec<[Vec3; 3]> {
    let mesh = Sphere::new(1.0).mesh().ico(times).unwrap();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return Vec::new() };
    let Some(Indices::U32(index)) = mesh.indices() else { return Vec::new() };
    index.chunks(3).map(|t| [Vec3::from(at[t[0] as usize]).normalize(), Vec3::from(at[t[1] as usize]).normalize(), Vec3::from(at[t[2] as usize]).normalize()]).collect()
}

/// Keeps the meshes in step with the regions that are made.
fn grow(mut commands: Commands, args: Res<crate::Args>, loading: Res<crate::loading::Loading>, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, forms: Res<Forms>, mut grown: ResMut<Grown>, mut meshes: ResMut<Assets<Mesh>>, mut cost: ResMut<crate::world::Cost>, cameras: Query<&GlobalTransform, With<Camera3d>>) {
    let _slow = crate::Slow::new("grow");
    let (Some(terrain), Some(look)) = (terrain, look) else { return };
    let began = std::time::Instant::now();
    grown.0.retain(|region, (entity, _)| {
        let keep = forms.0.contains_key(region);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    let size = terrain::REGION as f32 * CELL;
    // Near the camera every leaf mass is sprigs of leaflets; farther off, plain cards, a
    // fifth of the points; far off, a few broad ones. Three levels, as the ground has.
    let eye = cameras.iter().next().map(|t| t.translation()).unwrap_or_default();
    // (How far the camera is from the nearest of a region's ground, nothing if it is over it:
    // measured to a region's middle, a rock at a person's feet could be the far kind.)
    let away = |r: &(i32, i32)| Vec2::new((eye.x - (r.0 as f32 + 0.5) * size).abs() - size / 2.0, (eye.z - (r.1 as f32 + 0.5) * size).abs() - size / 2.0).max(Vec2::ZERO).length();
    let level = |r: &(i32, i32)| {
        let far = away(r);
        let now = if far < FINE { 0u8 } else if far < MID { 1 } else if far < FAR { 2 } else { 3 };
        match grown.0.get(r) {
            // Next to the line it is on, it stays as it is.
            Some((_, was)) if was.abs_diff(now) == 1 && (far - [FINE, MID, FAR][(*was).min(now) as usize]).abs() < 8.0 => *was,
            _ => now,
        }
    };
    // Nearest first, each worked out on another thread (a region's trees are some tens of
    // thousands of points); what a region showed before stays until its new mesh is ready.
    let mut wanted: Vec<((i32, i32), u8)> = forms.0.keys().map(|r| (*r, level(r))).filter(|(r, level)| grown.2.contains(r) || grown.0.get(r).is_none_or(|(_, was)| was != level)).collect();
    wanted.sort_by(|a, b| away(&a.0).total_cmp(&away(&b.0)));
    grown.1.retain(|r, (level, _)| wanted.iter().any(|w| w.0 == *r && w.1 == *level));
    // (The nearest first, and in play two a frame: each is a big mesh for the card to take.)
    let mut ready: Vec<(i32, i32)> = grown.1.iter().filter(|(_, (_, task))| task.is_finished()).map(|(r, _)| *r).collect();
    ready.sort_by(|a, b| away(a).total_cmp(&away(b)));
    for region in ready.into_iter().take(if loading.up || args.scene.is_some() { 8 } else { 2 }) {
        let Some((level, shape)) = grown.1.get_mut(&region).and_then(|(level, task)| bevy::tasks::futures::check_ready(task).map(|s| (*level, s))) else { continue };
        grown.1.remove(&region);
        grown.2.remove(&region);
        if let Some((old, _)) = grown.0.remove(&region) {
            commands.entity(old).despawn();
        }
        let origin = Vec3::new(region.0 as f32 * size, 0.0, region.1 as f32 * size);
        // Counted with the ground's faces in a picture's `pace`.
        let faces = crate::world::Faces((shape.indices.len() / 6) as u32);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, shape.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, shape.high)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, shape.tangents)
            .with_inserted_indices(Indices::U32(shape.indices));
        // (As cubes, the whole paint; as shapes, leaf is cards with gaps cut in them.)
        let entity = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(if args.shapes { look.solid.clone() } else { look.whole.clone() }), Transform::from_translation(origin), faces)).id();
        // (What is far off throws no shadow of its own: its shadow would be a few texels.)
        if level >= 2 {
            commands.entity(entity).insert(bevy::light::NotShadowCaster);
        }
        grown.0.insert(region, (entity, level));
        cost.grows += 1;
    }
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    for (region, level) in wanted {
        if grown.1.len() >= 8 {
            break;
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = grown.1.entry(region) {
            let (form, land, shapes) = (forms.0[&region].clone(), terrain.1.clone(), args.shapes);
            let origin = Vec3::new(region.0 as f32 * size, 0.0, region.1 as f32 * size);
            slot.insert((level, pool.spawn(async move { shape_of(&form, &land, origin, level, shapes, None) })));
        }
    }
    cost.grow += began.elapsed().as_secs_f32();
}

/// The mesh of everything that grows in a region, at one level of detail.
/// One tree alone, as it is drawn up close, about `origin`: for a tree that an axe has
/// felled, which is drawn as the tree it was wherever it now is (`loose::felled`). Nothing
/// of it bends in the wind: it is not standing.
pub fn alone(form: &Form, land: &cube_sim::terrain::Land, origin: Vec3, notch: Option<Notch>) -> Mesh {
    let mut shape = shape_of(form, land, origin, 0, false, notch);
    for high in &mut shape.high {
        high[0] = 0.0;
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, shape.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, shape.high)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, shape.tangents)
        .with_inserted_indices(Indices::U32(shape.indices))
}

fn shape_of(form: &Form, land: &cube_sim::terrain::Land, origin: Vec3, level: u8, shapes: bool, notch: Option<Notch>) -> Shape {
    let ground = |x: f32, z: f32| land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL;
    let mut shape = Shape { notch, ..default() };
    let mut still = (0, 0);
    // An end is cut wood if no other limb starts or stops there.
    let key = |p: Vec3| p.to_array().map(f32::to_bits);
    let mut ends: HashMap<[u32; 3], u32> = HashMap::new();
    for limb in &form.limbs {
        *ends.entry(key(limb.a)).or_default() += 1;
        *ends.entry(key(limb.b)).or_default() += 1;
    }
    if shapes {
        for (n, limb) in form.limbs.iter().enumerate() {
            shape.limb(limb.a - origin, limb.b - origin, limb.ra, limb.rb, limb.bark, n as u32, [ends[&key(limb.a)] == 1, ends[&key(limb.b)] == 1], limb.roots);
        }
        let facets = ball();
        let coarse = ball_of(0);
        still.0 = shape.positions.len();
        for (n, stone) in form.stones.iter().enumerate() {
            let mut stone = stone.clone();
            stone.middle -= origin;
            shape.stone(&stone, &facets, n as u32);
        }
        still.1 = shape.positions.len();
        for (n, clump) in form.clumps.iter().enumerate() {
            shape.clump(clump.middle - origin, clump.radii, clump.leaf, n as u32 + 100_000, &coarse, level);
        }
    } else {
        for (n, limb) in form.limbs.iter().enumerate() {
            // (Shaded by which tree it is, not by where it comes in the list: a tree drawn
            // alone once an axe is at it (`loose::felled`) is then the bark it was in its
            // region's shape, and does not change its pattern at the first blow.)
            let _ = n;
            shape.limb_cubes(limb, origin, &ground, limb.tree.wrapping_mul(2_654_435_761), [ends[&key(limb.a)] == 1, ends[&key(limb.b)] == 1], level);
        }
        let wood = shape.indices.len() / 6;
        still.0 = shape.positions.len();
        for (n, stone) in form.stones.iter().enumerate() {
            let mut stone = stone.clone();
            stone.middle -= origin;
            shape.stone_cubes(&stone, n as u32, level);
        }
        let stone = shape.indices.len() / 6 - wood;
        still.1 = shape.positions.len();
        for (n, clump) in form.clumps.iter().enumerate() {
            shape.clump_cubes(clump.middle - origin, clump.radii, clump.leaf, n as u32 + 100_000, level);
        }
        // `FLORA_TRACE=1`: what each region's trees and rocks cost, by level and part, a
        // line a region as it is made: add them up to find where the faces are.
        if std::env::var_os("FLORA_TRACE").is_some() {
            println!("flora level={level} wood={wood} stone={stone} leaf={} limbs={} clumps={} stones={}", shape.indices.len() / 6 - wood - stone, form.limbs.len(), form.clumps.len(), form.stones.len());
        }
    }
    shape.high = shape.positions.iter().enumerate().map(|(i, p)| [if (still.0..still.1).contains(&i) { 0.0 } else { (p[1] + origin.y - ground(p[0] + origin.x, p[2] + origin.z)).max(0.0) }, 0.0]).collect();
    shape
}

/// The faces of a cube: which way each looks, and the two ways along it.
const SIDES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Y, IVec3::Z),
    (IVec3::NEG_X, IVec3::Z, IVec3::Y),
    (IVec3::Y, IVec3::Z, IVec3::X),
    (IVec3::NEG_Y, IVec3::X, IVec3::Z),
    (IVec3::Z, IVec3::X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::Y, IVec3::X),
];

fn hashed(c: IVec3, k: u32) -> f32 {
    hash((c.x.wrapping_mul(73_856_093) ^ c.y.wrapping_mul(19_349_663) ^ c.z.wrapping_mul(83_492_791)) as u32, k)
}

/// What grows, as cubes (the default: `--shapes` draws the low-poly forms above instead).
///
/// The world is cubes and so is what grows on it; what kept cube trees from being
/// Minecraft's is how small the cubes are and that no two are alike. Wood is cubes of 5 and
/// 10 cm up close (the bit, and two of them), so a trunk is round, swells into its roots and
/// has furrows a cube deep; each cube is its own shade of bark, darker down the furrows.
/// Leaf is boxes of many sizes, a hand to half a metre, each its own green, lit on top and
/// dark beneath, over a dark heart. Stone is cubes in beds of differing shade, mossed on top.
/// Farther off the cubes are the world's cells (25 cm), and far off twice that.
impl Shape {
    /// The cubes of `set`, each `g` metres a side: every face that no other cube of the set
    /// covers, painted as `paint` says for that cube and face.
    fn cubes(&mut self, set: &HashSet<IVec3>, g: f32, shift: Vec3, free: f32, paint: impl Fn(IVec3, IVec3) -> ([f32; 3], Grain)) {
        let span = g / CELL;
        let room = SPAN as f32 - span - 0.04;
        for &c in set {
            for (n, u, v) in SIDES {
                if set.contains(&(c + n)) {
                    continue;
                }
                let (colour, grain) = paint(c, n);
                let corner = (c + n.max(IVec3::ZERO)).as_vec3() * g - shift;
                // The grain by where the cube is, each face a piece of the tile its own size.
                let (u0, v0) = ((c.dot(u) as f32 * span).rem_euclid(room) + 0.02, (c.dot(v) as f32 * span).rem_euclid(room) + 0.02);
                let start = self.positions.len() as u32;
                for (su, sv) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    // A corner is in shade by how many cubes stand over it beside this face
                    // (two along its edges, one across the corner): what gives a furrow, the
                    // crook of a root or a ledge of stone its depth, at no cost to draw.
                    let (du, dv) = (if su > 0.5 { u } else { -u }, if sv > 0.5 { v } else { -v });
                    let over = set.contains(&(c + n + du)) as usize + set.contains(&(c + n + dv)) as usize + set.contains(&(c + n + du + dv)) as usize;
                    let shade = [1.0, 0.8, 0.64, 0.5][over];
                    self.point(corner + (u.as_vec3() * su + v.as_vec3() * sv) * g, n.as_vec3(), colour.map(|k| k * shade), grain.uv(u0 + su * span, v0 + sv * span), free);
                }
                self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
    }

    /// One box from `lo` to `hi`, its top, sides and underside each a colour; the face that
    /// looks most nearly along `hidden` is left out (it is against something).
    #[allow(clippy::too_many_arguments)]
    fn boxed(&mut self, lo: Vec3, hi: Vec3, top: [f32; 3], side: [f32; 3], under: [f32; 3], grain: Grain, hidden: Vec3, free: f32, n: u32) {
        let size = hi - lo;
        for (f, (face, u, v)) in SIDES.into_iter().enumerate() {
            if face.as_vec3().dot(hidden) > 0.75 {
                continue;
            }
            let colour = if face == IVec3::Y { top } else if face == IVec3::NEG_Y { under } else { side.map(|c| c * (0.9 + 0.1 * (face.x + face.z) as f32)) };
            let corner = lo + face.max(IVec3::ZERO).as_vec3() * size;
            let (wide, tall) = ((u.as_vec3() * size).length(), (v.as_vec3() * size).length());
            let (su, sv) = ((wide / CELL).min(SPAN as f32 - 0.1), (tall / CELL).min(SPAN as f32 - 0.1));
            let (u0, v0) = (hash(n, 50 + f as u32) * (SPAN as f32 - su - 0.04) + 0.02, hash(n, 60 + f as u32) * (SPAN as f32 - sv - 0.04) + 0.02);
            let start = self.positions.len() as u32;
            for (a, b) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                self.point(corner + u.as_vec3() * size * a + v.as_vec3() * size * b, face.as_vec3(), colour, grain.uv(u0 + a * su, v0 + b * sv), free);
            }
            self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        }
    }

    /// A limb as cubes: round, tapering, wandering a little, ridged along its grain, and a
    /// rooted trunk swelling into buttresses at the ground, all to the cube.
    #[allow(clippy::too_many_arguments)]
    fn limb_cubes(&mut self, limb: &cube_sim::plants::Limb, origin: Vec3, ground: &dyn Fn(f32, f32) -> f32, n: u32, cut: [bool; 2], level: u8) {
        let floor = ground(limb.a.x, limb.a.z);
        let (a, b, ra, rb, bark) = (limb.a, limb.b, limb.ra, limb.rb, limb.bark);
        let thick = ra.max(rb);
        // What is dropped away from the camera: only what is hidden or is nothing at that
        // distance. A bough that droops (a conifer's) lies under its own needles; a twig far
        // off is under a pixel. Every other limb is drawn at every distance, as one line of
        // cubes if it is thinner than a cube: with the lesser limbs dropped, a young tree was
        // a crown in the air, and the developer saw floating leaves.
        let droops = (b - a).normalize_or(Vec3::Y).y < -0.05;
        if (level >= 1 && droops && thick < 0.3) || (level == 1 && thick < 0.07) || (level == 2 && thick < 0.2) || (level == 3 && thick < 0.3) {
            return;
        }
        // Up close a trunk or a heavy limb is cubes of 10 cm, round to the cube. A lesser
        // limb is one line of cubes as thick as it is: a hundred boughs of a pine drawn
        // round were more faces than the rest of the wood, and unseen under their needles.
        let g = match level {
            // (Near, the size the simulation fills it at for a body to meet: `Limb::near`.)
            0 => limb.near(),
            1 => 0.25,
            _ => 0.5,
        };
        // Its shape is the simulation's (`Limb::fill`), on the world's own grid and finer
        // than a cell: what is drawn is what a body is stopped by.
        let along = (b - a).normalize_or(Vec3::Y);
        const UNDER: f32 = cube_sim::plants::Limb::UNDER;
        // Far off a trunk is one box, as tall as it stands and as thick as its middle: its
        // cubes there were three hundred faces a tree and half a million in a view.
        if level == 3 {
            if along.y > 0.8 {
                let half = ((ra + rb) / 2.0).max(0.25);
                let (lo, hi) = (a.min(b) - origin, a.max(b) - origin);
                let shade = 0.85 + 0.2 * hash(n, 7);
                let colour = bark.colors().1.map(|c| c * shade);
                self.boxed(Vec3::new(lo.x - half, lo.y + UNDER, lo.z - half), Vec3::new(hi.x + half, hi.y, hi.z + half), colour, colour, colour, if bark == Block::Birch { Grain::BirchBark } else { Grain::Bark }, Vec3::NEG_Y, 0.0, n);
            }
            return;
        }
        // Up close, the fine cubes are for what is at hand: wood within reach of the ground
        // is cut fine, and whatever is higher (a tall trunk's upper part, an oak's great
        // limbs) in cubes twice the size, where no one is near enough to tell. Fine all the
        // way up, the wood of one region was half a million faces.
        // (And so a ring out, where the cubes are the world's cells: cells to six metres up, twice that above.)
        let fine_to = if level == 0 && thick >= 0.16 { floor + 4.5 } else if level == 1 && thick >= 0.25 { floor + 6.0 } else { f32::MAX };
        let mut set = HashSet::new();
        limb.fill(g, |c| {
            if (c.y as f32 + 0.5) * g < fine_to {
                set.insert(c);
            }
        });
        // Its roots go down to the ground under each of them, as the simulation's do
        // (`Limb::feet`): the land is there first, and a root angles with it.
        for foot in limb.feet(g, None) {
            let to = (ground((foot.x as f32 + 0.5) * g, (foot.z as f32 + 0.5) * g) / g).floor() as i32 - 1;
            for y in to.max(foot.y - (3.0 / g) as i32)..foot.y {
                set.insert(IVec3::new(foot.x, y, foot.z));
            }
        }
        if fine_to < f32::MAX && a.y.max(b.y) > fine_to {
            let (wide, mut above) = (g * 2.0, HashSet::new());
            limb.fill(wide, |c| {
                if (c.y as f32 + 0.5) * wide >= fine_to {
                    above.insert(c);
                }
            });
            self.wood(&above, wide, origin, limb, n, cut, 0.0);
        }
        // (An axe's notch is those cubes gone: a cut into the trunk, not a thing put on it.)
        if let Some(notch) = self.notch {
            set.retain(|c| !wedge(notch, (c.as_vec3() + 0.5) * g));
        }
        let free = ((0.16 - thick) / 0.16).clamp(0.0, 1.0) * 0.25;
        self.wood(&set, g, origin, limb, n, cut, free);
    }

    /// The cubes of a limb, painted as wood: every cube its own shade of bark, a line of them
    /// along the grain sharing one, moss at an old trunk's foot, cut wood on a free end.
    #[allow(clippy::too_many_arguments)]
    fn wood(&mut self, set: &HashSet<IVec3>, g: f32, origin: Vec3, limb: &cube_sim::plants::Limb, n: u32, cut: [bool; 2], free: f32) {
        let (a, b, ra, rb, bark, roots) = (limb.a, limb.b, limb.ra, limb.rb, limb.bark, limb.roots);
        let (thick, along, length) = (ra.max(rb), (b - a).normalize_or(Vec3::Y), a.distance(b));
        const UNDER: f32 = cube_sim::plants::Limb::UNDER;
        let colour = bark.colors().1;
        let grain = if bark == Block::Birch { Grain::BirchBark } else { Grain::Bark };
        let moss = Block::Moss.colors().0;
        let notch = self.notch;
        self.cubes(set, g, origin, free, |c, face| {
            let p = (c.as_vec3() + 0.5) * g;
            // (What looks into an axe's notch is cut wood.)
            if notch.is_some_and(|n| wedge(n, p + face.as_vec3() * g)) {
                return (cube_voxel::mesh::CUT.map(|v| v * (0.9 + 0.2 * hashed(c, 4))), Grain::Split);
            }
            let axial = (p - a).dot(along);
            let end = face.as_vec3().dot(along);
            // Cut wood, where nothing grows on from an end.
            if thick > 0.12 && ((end > 0.9 && cut[1] && axial > length - g * 1.5) || (end < -0.9 && cut[0] && axial < g * 1.5)) {
                // (The colour and grain of cut wood wherever it is cut, `mesh::CUT`: a half
                // again of the bark's first colour was near black on a birch, whose first
                // colour is its dark marks, and a felled birch's stump and butt were black.)
                return (cube_voxel::mesh::CUT.map(|v| v * (0.9 + 0.2 * hashed(c, 4))), Grain::Split);
            }
            // Every cube its own shade, and a line of them along the grain shares one: furrows.
            let line = ((p - a - along * axial) / g).round().as_ivec3();
            let shade = (0.74 + 0.38 * hashed(line, n)) * (0.9 + 0.2 * hashed(c, 7)) * if face == IVec3::Y { 1.12 } else { 1.0 };
            if bark == Block::Birch {
                // Pale, with a birch's dark dashes lying across it.
                let dash = face.y == 0 && hashed(c, 5) < 0.035;
                return (if dash { [0.16, 0.15, 0.14] } else { colour.map(|v| v * (0.92 + 0.14 * hashed(c, 7))) }, grain);
            }
            // Moss on the roots and the foot of an old trunk, on what looks up and some of what does not.
            let low = roots > 0 && axial < UNDER + 1.4;
            if low && hashed(c.div_euclid(IVec3::splat(3)), 11) > 0.5 && (face == IVec3::Y || hashed(c, 9) > 0.7) {
                return (moss.map(|v| v * (0.8 + 0.4 * hashed(c, 7))), Grain::Moss);
            }
            (colour.map(|v| v * shade), grain)
        });
    }

    /// A mass of leaf as boxes: many, of many sizes, each its own green, over a dark heart.
    /// A conifer's spray is thin plates lying one over another.
    fn clump_cubes(&mut self, middle: Vec3, radii: Vec3, leaf: Block, n: u32, level: u8) {
        let (top, under) = leaf.colors();
        let warm = hash(n, 90) - 0.5;
        let tint = |c: [f32; 3], w: f32| [c[0] * (1.0 + 0.5 * w), c[1] * (1.0 + 0.16 * w), c[2] * (1.0 - 0.4 * w)];
        let (top, under) = (tint(top, warm), tint(under, warm));
        let spray = radii.y < radii.x.min(radii.z) * 0.6;
        let grain = if leaf == Block::Needles { Grain::Needles } else { Grain::Leaves };
        let snap = |v: Vec3, to: f32| (v / to).round() * to;
        // The body of the mass: three boxes crossed, a broad one, a tall one and a long one,
        // which together are as near an egg as boxes come. The boxes below sit on it, half in
        // and half out, so the mass is one rough lump. Without a body of its own shape the
        // boxes hung in the air round a slab, and from under a tree they were confetti.
        if level == 3 {
            if spray && !n.is_multiple_of(3) || !spray && radii.min_element() < 0.8 {
                return;
            }
            let half = snap(radii * if spray { Vec3::new(1.2, 1.0, 1.2) } else { Vec3::new(0.85, 0.7, 0.85) }, 0.5).max(Vec3::new(0.5, 0.25, 0.5));
            let at = snap(middle, 0.5);
            let shade = 0.8 + 0.25 * hash(n, 8);
            self.boxed(at - half, at + half, top.map(|c| c * shade), top.map(|c| c * shade * 0.74), under.map(|c| c * 0.5), grain, Vec3::ZERO, 0.2, n);
            return;
        }
        if !spray && radii.min_element() > 0.3 {
            let grid = if level == 0 { 0.05 } else { 0.25 };
            let at = snap(middle, grid);
            for (k, part) in [Vec3::new(0.8, 0.5, 0.8), Vec3::new(0.56, 0.78, 0.56), Vec3::new(0.94, 0.3, 0.6), Vec3::new(0.6, 0.3, 0.94)].into_iter().enumerate() {
                let half = snap(radii * part, grid).max(Vec3::splat(grid));
                // Lit as leaf is: its top the leaf's own green, darker down its sides, dark under.
                let shade = 0.78 + 0.1 * k as f32;
                self.boxed(at - half, at + half, top.map(|c| c * shade), top.map(|c| c * shade * 0.72), under.map(|c| c * 0.5), grain, Vec3::ZERO, 0.2, n * 7 + k as u32);
            }
        }
        // A spray away from the camera is one plate, as wide as the spray: a few small
        // boxes for each left a far pine bare.
        if spray && level > 0 {
            let half = snap(Vec3::new(radii.x * 0.8, 0.125, radii.z * 0.8), 0.25).max(Vec3::new(0.25, 0.125, 0.25));
            let at = snap(middle, 0.25);
            let shade = 0.8 + 0.3 * hash(n, 8);
            self.boxed(at - half, at + half, top.map(|c| c * shade), top.map(|c| c * shade * 0.75), under.map(|c| c * 0.55), grain, Vec3::ZERO, 0.3, n);
            return;
        }
        let area = 4.0 * std::f32::consts::PI * ((radii.x * radii.y).powf(1.6) + (radii.x * radii.z).powf(1.6) + (radii.y * radii.z).powf(1.6)).powf(1.0 / 1.6) / 3.0f32.powf(1.0 / 1.6);
        let unit = [0.45f32, 1.0, 1.6][level as usize].max(radii.min_element() * [0.2, 0.34, 0.5][level as usize]).min(radii.x.min(radii.z) * 0.9).max(0.15);
        // (Capped by level: a grown oak's masses are five metres through, and by area alone a
        // wood of them was three million faces.)
        let count = ((area * [0.17, 0.1, 0.05][level as usize] / (unit * unit)) as u32).clamp([4, 5, 0][level as usize], [90, 14, 0][level as usize]);
        for i in 0..count {
            let roll = |k: u32| hash(n.wrapping_mul(977).wrapping_add(i), k);
            let (turn, up) = (roll(1) * std::f32::consts::TAU, roll(2) * 2.0 - 1.0);
            let round = (1.0 - up * up).sqrt();
            let out = Vec3::new(turn.cos() * round, up, turn.sin() * round);
            // (On the body's surface, a little in or out: each touches it.)
            let deep = if spray { 0.7 + 0.3 * roll(3).sqrt() } else { 0.8 + 0.16 * roll(3) };
            let at = middle + out * radii * deep;
            // Sizes from a hand to the unit and half again, more small than large.
            let size = unit * (0.45 + 1.0 * roll(4) * roll(4));
            let dims = snap(Vec3::new(size * (0.8 + 0.5 * roll(5)), size * if spray { 0.3 } else { 0.55 + 0.5 * roll(6) }, size * (0.8 + 0.5 * roll(7))), 0.05).max(Vec3::splat(0.1));
            let lo = snap(at - dims / 2.0, 0.05);
            // Deeper in and lower down is in the crown's own shade; no two boxes the one green.
            let shade = (0.8 + 0.3 * roll(13)) * (0.8 + 0.25 * (up * 0.5 + 0.5)) * (0.88 + 0.24 * roll(8));
            let own = (roll(12) - 0.5) * 0.8;
            let lit = tint(if up > -0.3 { top } else { under }, own).map(|c| c * shade);
            self.boxed(lo, lo + dims, lit.map(|c| c * 1.18), lit.map(|c| c * 0.86), tint(under, own).map(|c| c * shade * 0.62), grain, -out, 0.4, n.wrapping_mul(31).wrapping_add(i));
        }
    }

    /// A stone as cubes: its true shape to the cube, lying in beds of differing shade, with
    /// moss and lichen on what looks up.
    fn stone_cubes(&mut self, stone: &cube_sim::plants::Stone, n: u32, level: u8) {
        let big = stone.radii.max_element();
        let g = match level {
            0 if big > 2.4 => 0.2,
            0 if big < 0.3 => 0.05,
            0 => 0.1,
            // (A boulder a few steps off, in the next region, is still seen close.)
            1 if big <= 2.4 => 0.15,
            1 => 0.25,
            _ => 0.5,
        };
        if (level == 2 && big < 0.5) || (level == 3 && big < 1.2) {
            return;
        }
        let (sin, cos) = stone.turn.sin_cos();
        let reach = Vec3::new(stone.radii.x.max(stone.radii.z), stone.radii.y, stone.radii.x.max(stone.radii.z));
        let (lo, hi) = (((stone.middle - reach) / g).floor().as_ivec3(), ((stone.middle + reach) / g).ceil().as_ivec3());
        let mut set = HashSet::new();
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let c = IVec3::new(x, y, z);
                    let d = (c.as_vec3() + 0.5) * g - stone.middle;
                    let q = Vec3::new((d.x * cos + d.z * sin) / stone.radii.x, d.y / stone.radii.y, (d.z * cos - d.x * sin) / stone.radii.z);
                    if q.length_squared() < 1.0 && stone.faces.iter().all(|(normal, far)| q.dot(*normal) < *far) {
                        set.insert(c);
                    }
                }
            }
        }
        let (top, side) = stone.kind.colors();
        let (moss, mossy) = (Block::Moss.colors().0, hash(n, 5) < 0.6);
        self.cubes(&set, g, Vec3::ZERO, 0.0, |c, face| {
            // Beds a hand or two thick, each its own shade and tipped a little.
            let bed = ((c.y as f32 * g + (c.x as f32 * 0.12 + c.z as f32 * 0.07) * g) / 0.3).floor() as i32;
            let shade = (0.8 + 0.34 * hash(bed as u32, n)) * (0.92 + 0.16 * hashed(c, 7));
            if face == IVec3::Y {
                let patch = hashed(IVec3::new((c.x as f32 * g / 0.4) as i32, 0, (c.z as f32 * g / 0.4) as i32), n);
                if mossy && patch > 0.5 {
                    return (moss.map(|v| v * (0.8 + 0.4 * hashed(c, 9))), Grain::Moss);
                }
                // Lichen: pale flecks.
                if hashed(c, 13) < 0.06 {
                    return ([0.42, 0.44, 0.3], Grain::Rock);
                }
                return (top.map(|v| v * shade), Grain::Rock);
            }
            (side.map(|v| v * shade), Grain::Strata)
        });
    }
}
