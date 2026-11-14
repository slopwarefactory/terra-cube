//! Clouds to look at: fair-weather cumulus as volumes of cubes. Each cloud is a field of
//! density (a heap of soft puffs over a level foot) cut into cells, and every cell that has
//! cloud in it is a cube: 10 m ones near to, standing in steps of 20 and 40 m, since
//! everything in this world is cuboids (docs/art-guide.md) and detail comes from many sizes
//! of them, not from soft shapes.
//!
//! What makes a heap of cubes a cloud is how it takes light, and that comes from the volume,
//! not from the cubes' faces: each cube is lit as the cloud's surface there would be (it
//! faces the way the density falls away), so one side of a cloud is bright and the other in
//! shade whichever way its faces point; cubes with much cloud over them are greyer, which is
//! the dark level foot of a cumulus; and the cloud glows a little all through with the sun
//! scattered inside it, so its shaded side is not the colour of a shaded rock.
//!
//! They are dressing, as what is in the air is (`air`): the simulation does not know of
//! them, and the shade that crosses the land is still the sun's own tile (`world::clouds`),
//! which drifts the same way. Some hundreds stand in a field round the camera and go
//! downwind; one that leaves by one side comes in by the other, and each dwindles to nothing
//! toward the field's edge so none is seen to arrive. Nothing here colours them: they are
//! white, and the sun through the air and the sky light them as they light the land, so they
//! are white over grey-blue at noon, lit amber from under at sunset and dim under the moon.

use crate::air::roll;
use bevy::{
    asset::RenderAssetUsages,
    camera::Exposure,
    light::{NotShadowCaster, NotShadowReceiver, light_consts::lux},
    mesh::Indices,
    material::OpaqueRendererMethod,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use cube_sim::terrain::SEA;
use cube_voxel::CELL;

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Cloudy>::default()).add_systems(Startup, setup).add_systems(Update, (drift, lit));
    }
}

/// How high over the sea the clouds' feet are overhead, metres: well over the mountain
/// (`scale::MASSIF`, 170 m), all at one level as the feet of cumulus are.
const FOOT: f32 = 430.0;
/// And at the field's edge: they come down toward the horizon faster than a level layer
/// would, so the sky is filled to its foot without a field tens of kilometres across.
const EDGE_FOOT: f32 = 130.0;
/// How far across the field is, metres, and how many places for a cloud along each side.
const FIELD: f32 = 14_000.0;
const PLACES: i32 = 26;
/// How much of the field has cloud: 0 none, 1 a cloud at every place.
const COVER: f32 = 0.74;
/// How many shapes there are; each cloud is one of them, turned one of four ways and sized.
const KINDS: usize = 16;
/// A cloud's smallest cube, metres (times the cloud's own size, 0.8 to 1.2); its larger steps are twice and four times that. Beyond
/// `NEAR` metres a cloud is drawn with none smaller than twice it, for a quarter of the faces.
const CUBE: f32 = 10.0;
const NEAR: f32 = 2600.0;
/// How much density makes cloud: lower and every puff is fatter.
const LEVEL: f32 = 0.3;
/// Which way they go and how fast, metres a second: the way the cloud shadows and the pollen
/// go (`world::sky`, `air::WIND`), and faster than the shadows, as the day here is.
const WIND: Vec3 = Vec3::new(0.8, 0.0, 0.6);
/// Where in the field the world begins: under a fair sky with a bank standing off to the
/// south-east, not in one of its clear patches or under a bank (both come over in time).
const START: Vec3 = Vec3::new(6000.0, 0.0, -1000.0);
const DRIFT: f32 = 5.0;

/// A cloud's material: Bevy's own, matt white, with the light a cloud has inside it added
/// by the fragment shader (`assets/shaders/cloud.wesl`).
type Cloudy = ExtendedMaterial<StandardMaterial, Within>;

#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
struct Within {
    /// Which way the sun is (xyz).
    #[uniform(100)]
    sun: Vec4,
    /// The sun's light as a white thing facing it gives it back at a cloud's height, at the
    /// camera's exposure (rgb).
    #[uniform(101)]
    glow: Vec4,
    /// What the side of a cloud away from a low sun takes from the sky: its blue (rgb, as
    /// the picture has it).
    #[uniform(102)]
    shade: Vec4,
}

impl MaterialExtension for Within {
    fn fragment_shader() -> ShaderRef {
        "shaders/cloud.wesl".into()
    }
}

/// A cloud: where it stands in the field when the wind has not moved it, how big it is
/// (under nought: one of the high thin kind, and that big), and which shape.
#[derive(Component)]
struct Cloud(Vec3, f32, usize);

/// The shapes, each as fine cubes and as coarse, and the one material.
#[derive(Resource)]
struct Kinds(Vec<(Handle<Mesh>, Handle<Mesh>)>, Handle<Cloudy>);

/// A puff of a cloud: where its middle is, how far it reaches each way, how much it counts.
struct Puff(Vec3, Vec3, f32);

/// One cloud's puffs, from a foot at 0, and how far it reaches (half its length and width,
/// and its height). Big ones strewn over the foot with their middles low, so the foot cuts
/// them level and they are tallest toward the middle; lesser ones high on them, which are
/// the turrets; small ones out at the rim. `long` is how far across it is.
fn puffs(state: &mut u32, long: f32) -> (Vec<Puff>, Vec3) {
    let wide = long * (0.5 + 0.3 * roll(state));
    let tall = long * (0.28 + 0.16 * roll(state));
    let mut puffs = Vec::new();
    let body = 5 + (long / 55.0) as usize;
    let bend = roll(state) * std::f32::consts::TAU;
    for _ in 0..body {
        // Along a bent band, not over a round patch: from under, a cloud is not a disc.
        let u = roll(state) * 2.0 - 1.0;
        let v = ((roll(state) * 2.0 - 1.0) * 0.6 + 0.4 * (u * 2.6 + bend).sin()).clamp(-1.0, 1.0);
        let mid = (1.0 - (u * u + v * v)).max(0.0);
        let across = wide * (0.3 + 0.3 * mid + 0.12 * roll(state));
        let up = tall * (0.55 + 0.65 * mid) * (0.8 + 0.4 * roll(state));
        let at = Vec3::new(u * long * 0.34, up * (0.05 + 0.2 * roll(state)), v * wide * 0.34);
        puffs.push(Puff(at, Vec3::new(across * (0.9 + 0.5 * roll(state)), up, across * (0.9 + 0.3 * roll(state))), 1.0));
        // Turrets: one or two on its shoulders, reaching over its top.
        for _ in 0..1 + (roll(state) * 2.0) as usize {
            let reach = across * (0.3 + 0.25 * roll(state));
            let turn = roll(state) * std::f32::consts::TAU;
            let on = at + Vec3::new(turn.cos() * across * 0.45, up * (0.55 + 0.35 * roll(state)), turn.sin() * across * 0.45);
            // (Some stand well up: a tower.)
            puffs.push(Puff(on, Vec3::new(reach, reach * (0.8 + 1.1 * roll(state) * roll(state)), reach), 0.9));
        }
    }
    // The rim: small low ones round the foot, where a cloud frays.
    for _ in 0..body {
        let turn = roll(state) * std::f32::consts::TAU;
        let out = 0.36 + 0.14 * roll(state);
        let reach = wide * (0.1 + 0.1 * roll(state));
        puffs.push(Puff(Vec3::new(turn.cos() * long * out, reach * 0.2, turn.sin() * wide * out), Vec3::new(reach * 1.4, reach * 0.8, reach), 0.8));
    }
    let reach = puffs.iter().fold(Vec3::ZERO, |r, p| r.max(p.0.abs() + p.1));
    (puffs, reach)
}

/// How much cloud there is at a point: every puff's share, most at its middle and none at
/// its reach.
fn mass(puffs: &[Puff], at: Vec3) -> f32 {
    puffs.iter().map(|p| {
        let k = (1.0 - ((at - p.0) / p.1).length_squared()).max(0.0);
        p.2 * k * k
    }).sum()
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77) ^ (z as u32).wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// A cloud's puffs as cubes of `cube` metres. A cell is cloud by the density at
/// the middle of the 32 m block it is in, of the 16 m one and at its own, so the cloud
/// stands in big steps with lesser ones on them and small ones worn off their corners.
///
/// Each cube's corners all face one way, and it is not the way of any of its faces: it is
/// the way the density falls off there, leaning a little to each face's own way so that a
/// cube is still seen to be one. And each carries how much cloud is over it, as a greyer
/// white: the foot is the darkest of a cloud, and no two cubes are quite one white.
fn shape(puffs: &[Puff], reach: Vec3, cube: f32) -> Shape {
    let big = CUBE * 4.0;
    // The density at the middle of the block of `to` metres a point is in. Many cells share
    // a block, so each block's is found once.
    let mut found: [std::collections::HashMap<IVec3, f32>; 2] = default();
    let mut blocked = |at: Vec3, to: f32, which: usize| {
        let block = (at / to).floor();
        *found[which].entry(block.as_ivec3()).or_insert_with(|| mass(puffs, (block + 0.5) * to))
    };
    let mut cloud = |at: Vec3| {
        // Nor is its edge the puffs' own round one: it is pushed in and out by the block
        // and by blocks of three.
        let grain = |to: f32, n: i32| hash((at.x / to).floor() as i32, (at.y / to).floor() as i32 + n, (at.z / to).floor() as i32) - 0.5;
        let rough = 0.1 * grain(cube, 500) + 0.16 * grain(CUBE * 2.0, 0) + 0.2 * grain(CUBE * 6.0, 1000);
        at.y > 0.0 && 0.35 * blocked(at, big, 0) + 0.3 * blocked(at, (big / 2.0).max(cube), 1) + 0.35 * mass(puffs, at) + rough > LEVEL
    };
    // The cells, in a box that holds every puff with a block to spare.
    let cells = ((reach + Vec3::splat(big)) / cube).ceil();
    let (nx, ny, nz) = (cells.x as i32 * 2, cells.y as i32, cells.z as i32 * 2);
    let middle = |x: i32, y: i32, z: i32| Vec3::new((x - nx / 2) as f32 + 0.5, y as f32 + 0.5, (z - nz / 2) as f32 + 0.5) * cube;
    let index = |x: i32, y: i32, z: i32| ((x * ny + y) * nz + z) as usize;
    let mut full = vec![false; (nx * ny * nz) as usize];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                full[index(x, y, z)] = cloud(middle(x, y, z));
            }
        }
    }
    let is = |x: i32, y: i32, z: i32| x >= 0 && y >= 0 && z >= 0 && x < nx && y < ny && z < nz && full[index(x, y, z)];
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for x in 0..nx {
        for z in 0..nz {
            // From the top of the column down, counting the cloud passed through.
            let mut over = 0.0;
            for y in (0..ny).rev() {
                if !is(x, y, z) {
                    continue;
                }
                let at = middle(x, y, z);
                let step = cube * 0.75;
                let fall = Vec3::new(
                    mass(puffs, at - Vec3::X * step) - mass(puffs, at + Vec3::X * step),
                    mass(puffs, at - Vec3::Y * step) - mass(puffs, at + Vec3::Y * step),
                    mass(puffs, at - Vec3::Z * step) - mass(puffs, at + Vec3::Z * step),
                )
                .normalize_or_zero();
                let block = (at / big).floor();
                let own = 0.86 + 0.09 * hash(x, y, z) + 0.05 * hash(block.x as i32, block.y as i32 + 77, block.z as i32);
                let white = own * (0.6 + 0.4 * (-over / 45.0f32).exp());
                over += cube;
                for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                    if is(x + dx, y + dy, z + dz) {
                        continue;
                    }
                    let face = Vec3::new(dx as f32, dy as f32, dz as f32);
                    // The face's two other ways, in an order that winds it to face outward.
                    let (a, b) = if dx != 0 { (Vec3::Y * dx as f32, Vec3::Z) } else if dy != 0 { (Vec3::Z * dy as f32, Vec3::X) } else { (Vec3::X * dz as f32, Vec3::Y) };
                    let normal = (fall + face * 0.9).normalize_or(face);
                    // And each face a little its own shade, as the land's cubes are: the top
                    // lightest, the sides less, the underside least, so a cube is a cube in
                    // flat light too.
                    let white = white * if dy > 0 { 1.0 } else if dy < 0 { 0.84 } else { 0.92 };
                    let first = positions.len() as u32;
                    for (u, v) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                        positions.push((at + (face * 0.5 + a * u + b * v) * cube).to_array());
                        normals.push(normal.to_array());
                        // Greyer is also bluer: what is deep in a cloud is lit by the sky more than the sun.
                        colors.push([white * (0.66 + 0.34 * white), white * (0.84 + 0.16 * white), white, 1.0]);
                    }
                    indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                }
            }
        }
    }
    Shape { positions, normals, colors, indices }
}

/// A cloud's cubes, ready to be a mesh.
struct Shape {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Shape {
    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// How much cloud a place in the field has, 0 to 1: broad patches of it with clear sky
/// between, the same at both edges of the field so it joins where it wraps.
fn cover(x: i32, z: i32) -> f32 {
    // Two layers: patches some three kilometres across, and half that.
    [(5, 0.6), (11, 0.4)].iter().enumerate().map(|(n, &(knots, weight))| {
        let (fx, fz) = (x as f32 * knots as f32 / PLACES as f32, z as f32 * knots as f32 / PLACES as f32);
        let (x0, z0) = (fx.floor(), fz.floor());
        let (tx, tz) = (fx - x0, fz - z0);
        let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
        let at = |dx: i32, dz: i32| hash((x0 as i32 + dx).rem_euclid(knots), n as i32, (z0 as i32 + dz).rem_euclid(knots));
        weight * ((at(0, 0) * (1.0 - sx) + at(1, 0) * sx) * (1.0 - sz) + (at(0, 1) * (1.0 - sx) + at(1, 1) * sx) * sz)
    }).sum()
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<Cloudy>>) {
    let mut state = 0x51ED_270Bu32;
    // White and matt. Not hazed by the land's fog, which is for what is a kilometre off: the
    // air itself hazes them. Drawn forward, where a shader can add to what a thing is lit
    // by; the light within is set from the sun every frame (`lit`).
    let white = materials.add(ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, reflectance: 0.0, fog_enabled: false, opaque_render_method: OpaqueRendererMethod::Forward, ..default() },
        extension: Within::default(),
    });
    // The shapes, small to large.
    let longs: Vec<f32> = (0..KINDS).map(|k| 150.0 + 610.0 * (k as f32 / (KINDS - 1) as f32).powf(1.5)).collect();
    // Cutting a cloud into cubes is some millions of sums: each shape on a thread of its own,
    // all of them done in the time of the largest (nothing long on the frame's thread).
    let heaps: Vec<(Vec<Puff>, Vec3)> = longs.iter().map(|&long| puffs(&mut state, long)).collect();
    let shapes: Vec<(Shape, Shape)> = std::thread::scope(|scope| {
        let cut: Vec<_> = heaps.iter().map(|(puffs, reach)| scope.spawn(move || (shape(puffs, *reach, CUBE), shape(puffs, *reach, CUBE * 2.0)))).collect();
        cut.into_iter().map(|c| c.join().expect("a cloud's shape")).collect()
    });
    let kinds: Vec<(Handle<Mesh>, Handle<Mesh>)> = shapes.into_iter().map(|(fine, coarse)| (meshes.add(fine.mesh()), meshes.add(coarse.mesh()))).collect();
    let place = FIELD / PLACES as f32;
    for x in 0..PLACES {
        for z in 0..PLACES {
            // The thicker the patch, the bigger its cloud; the thinnest have none of their own.
            // And small ones stand anywhere, in half the places, so no part of the sky is bare.
            let thick = ((cover(x, z) - (1.0 - COVER)) / 0.4).clamp(0.0, 1.0);
            // (And squared: most of the sky thin, a few places a bank.)
            let thick = thick * thick;
            // (Small ones in clumps of up to three, more of them where it is thick, strewn
            // well beyond their own place: one to a place apiece was a sky of polka dots.)
            let small = (roll(&mut state) < 0.12 + 0.6 * thick) as usize + (roll(&mut state) < 0.6 * thick) as usize + (roll(&mut state) < 0.12) as usize;
            for nth in 0..1 + small {
                if nth == 0 && thick <= 0.0 {
                    continue;
                }
                let size = if nth == 0 { thick * (0.35 + 0.65 * roll(&mut state)) } else { 0.3 * roll(&mut state) * roll(&mut state) };
                let kind = ((size * KINDS as f32) as usize).min(KINDS - 1);
                let strewn = if nth == 0 { 1.0 } else { 2.6 };
                let at = Vec3::new((x as f32 + 0.5 + (roll(&mut state) - 0.5) * strewn) * place, 0.0, (z as f32 + 0.5 + (roll(&mut state) - 0.5) * strewn) * place) + START;
                // One small one in eleven is of a higher, thinner kind, twice as far up.
                let high = nth > 0 && roll(&mut state) < 0.09;
                // (The largest are wider than a place and lie into their neighbours': a bank.)
                let scale = (0.8 + 0.4 * roll(&mut state) + if nth == 0 { 0.5 * thick } else { 0.0 }) * if high { -1.0 } else { 1.0 };
                // (The high kind all lie one way, drawn out along the wind.)
                let turned = Quat::from_rotation_y(if high { -WIND.z.atan2(WIND.x) } else { (roll(&mut state) * 4.0).floor() * std::f32::consts::FRAC_PI_2 });
                // (And stand in a row of three or four along it, not one plank alone.)
                for nth in 0..if high { 3 + (roll(&mut state) * 2.0) as usize } else { 1 } {
                    let at = at + WIND * (nth as f32 * longs[kind] * 2.3) + Vec3::new(-WIND.z, 0.0, WIND.x) * (if nth == 0 { 0.0 } else { (roll(&mut state) - 0.5) * 160.0 });
                    // (Each its own shape: three of one shape in a row were seen to be.)
                    let kind = if nth == 0 { kind } else { (kind + 1 + (roll(&mut state) * 4.0) as usize) % 6 };
                    commands.spawn((Cloud(at, scale * if nth == 0 { 1.0 } else { 0.7 + 0.5 * roll(&mut state) }, kind), Mesh3d(kinds[kind].1.clone()), MeshMaterial3d(white.clone()), Transform::from_translation(Vec3::new(0.0, -5000.0, 0.0)).with_rotation(turned), NotShadowCaster, NotShadowReceiver));
                }
            }
        }
    }
    commands.insert_resource(Kinds(kinds, white));
}

/// The field goes downwind and stays round the camera. A cloud's foot is highest overhead
/// and comes down toward the field's edge, where the cloud has dwindled to nothing; near
/// ones are drawn in their fine cubes.
fn drift(time: Res<Time>, live: Option<Res<crate::motion::Live>>, kinds: Res<Kinds>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut clouds: Query<(&Cloud, &mut Transform, &mut Mesh3d)>, mut turn: Local<usize>) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation()) else { return };
    // The weather (`sky::cloud`): on a clear day the clouds are small and far between, and
    // under an overcast they have grown into each other. They are the same clouds, each as
    // big as the sky is clouded: the field does not change, so nothing is made or lost.
    let weather = 0.55 + 1.0 * crate::world::overcast(live.map_or(cube_sim::sky::START, |l| l.0.when));
    // An eighth of them each frame: a cloud goes 5 m in a second half a kilometre off, and
    // moved eight times a second it is not seen to step; moved every frame, a thousand of
    // them are a thousand things changed for the engine to carry to the picture.
    *turn += 1;
    let blown = WIND * (time.elapsed_secs() * DRIFT);
    let sea = SEA as f32 * CELL;
    for (nth, (cloud, mut at, mut mesh)) in clouds.iter_mut().enumerate() {
        if nth % 8 != *turn % 8 {
            continue;
        }
        let away = |home: f32, eye: f32| (home - eye + FIELD / 2.0).rem_euclid(FIELD) - FIELD / 2.0;
        let (dx, dz) = (away(cloud.0.x + blown.x, eye.x), away(cloud.0.z + blown.z, eye.z));
        let far = (dx * dx + dz * dz).sqrt();
        let out = (far / (FIELD / 2.0)).min(1.0);
        let fade = 1.0 - ((out - 0.8) / 0.2).clamp(0.0, 1.0);
        // (A size under nought marks one of the high thin kind.)
        let (size, lift, long, thin) = if cloud.1 < 0.0 { (-cloud.1, 2.2, 1.7, 0.3) } else { (cloud.1, 1.0, 1.0, 1.0) };
        at.translation = Vec3::new(eye.x + dx, sea + (FOOT - (FOOT - EDGE_FOOT) * out * out) * lift, eye.z + dz);
        // (Never quite nothing: a size of nought has no way back.)
        let size = (size * weather * fade * fade * (3.0 - 2.0 * fade)).max(1e-4);
        at.scale = Vec3::new(size * long, size * thin, size);
        let want = if far < NEAR { &kinds.0[cloud.2].0 } else { &kinds.0[cloud.2].1 };
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}

/// The light a cloud has within it, for its shader: which way the sun is, and the sun's
/// light as a white thing facing it would give it back up there, the colour the air has
/// left it at that height. Set for the camera's exposure here.
fn lit(live: Option<Res<crate::motion::Live>>, kinds: Res<Kinds>, cameras: Query<&Exposure, With<Camera3d>>, mut materials: ResMut<Assets<Cloudy>>, mut was: Local<Vec3>) {
    let Some(exposure) = cameras.iter().next() else { return };
    let to_sun = cube_sim::sky::sun(live.map_or(cube_sim::sky::START, |l| l.0.when));
    // The sun's colour up there (`vault::through`). A cloud is high enough to have a last
    // deep red for a little after the land has none.
    let through = crate::vault::through(to_sun.y) * ((to_sun.y + 0.09) / 0.08).clamp(0.0, 1.0);
    let glow = through * (lux::RAW_SUNLIGHT / std::f32::consts::PI * exposure.exposure());
    if (glow - *was).abs().max_element() < 0.002 {
        return;
    }
    *was = glow;
    if let Some(mut material) = materials.get_mut(&kinds.1) {
        material.extension.sun = to_sun.extend(0.0);
        material.extension.glow = glow.extend(0.0);
        // The sky's blue is on a cloud's shaded side while the sun is low and still up or
        // just under: by day the engine's own sky light is enough, and at night there is none.
        let low = (1.0 - ((to_sun.y - 0.02) / 0.4).abs()).clamp(0.0, 1.0) * ((to_sun.y + 0.12) / 0.06).clamp(0.0, 1.0);
        material.extension.shade = (Vec3::new(0.10, 0.15, 0.36) * (0.75 * low)).extend(0.0);
    }
}
