//! The world's terrain (`cube_sim::terrain`), meshed a chunk at a time, with sun, sky, fog and
//! a sea that runs on to the horizon.

use bevy::{asset::RenderAssetUsages, light::CascadeShadowConfigBuilder, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::terrain::{self, Land, SEA};
use bevy::camera::{Exposure, visibility::VisibilityRange};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight, DirectionalLightTexture, FogVolume, SunDisk, VolumetricFog, VolumetricLight, atmosphere::ScatteringMedium, light_consts::lux};
use bevy::pbr::{AtmosphereSettings, ExtendedMaterial, MaterialExtension, ScreenSpaceAmbientOcclusion};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
use bevy::post_process::bloom::Bloom;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use cube_voxel::{
    Block, CELL, CHUNK, World as Voxels,
    mesh::{mesh_chunk, mesh_chunk_far},
};
use std::collections::HashSet;

pub struct WorldPlugin;

/// The voxel world, and the lie of the land it was made from (heights, biomes, trees).
#[derive(Resource)]
pub struct Terrain(pub Voxels, pub std::sync::Arc<Land>, pub cube_sim::ecology::Ecology, pub HashSet<(i32, i32)>);

/// The land's material: Bevy's own, with wind in what grows. The wind is in the two vertex
/// shaders (`assets/shaders/wind.wesl` for the picture, `wind_prepass.wesl` for depth and
/// shadows), which move each corner by how free the mesher said it is.
pub type Windy = ExtendedMaterial<StandardMaterial, Wind>;

#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct Wind {
    /// The season, for the shader: the tint of leaves and grass (rgb) and how much snow lies.
    #[uniform(100)]
    pub season: Vec4,
    /// How hard the wind blows, 0 to 1, in x.
    #[uniform(101)]
    pub wind: Vec4,
}

/// How much of the sky is cloud now, 0 to 1 (`cube_sim::sky::cloud`); `CLOUD=1` sets it, and
/// `RAIN` sets it to overcast with it, since it does not rain from a clear sky.
pub fn overcast(when: f32) -> f32 {
    let set = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<f32>().ok());
    set("CLOUD").or(set("RAIN").filter(|r| *r > 0.0).map(|_| 1.0)).unwrap_or_else(|| cube_sim::sky::cloud(when))
}

/// How hard it rains now, 0 to 1, and whether what falls is snow (`cube_sim::sky::rain`,
/// `snows`): for what draws it and what sounds it. `RAIN=0.8` (and `SNOW=1`) on any
/// `cargo sandbox` sets it, to be looked at and listened to.
pub fn falling(when: f32) -> (f32, bool) {
    let rain = std::env::var("RAIN").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| cube_sim::sky::rain(when));
    (rain, std::env::var_os("SNOW").is_some() || cube_sim::sky::snows(when))
}

/// How hard the wind blows now, 0 to 1 (`cube_sim::sky::wind`, or `--wind`): the one wind,
/// for the leaves, for what is heard and for what drifts.
#[derive(Resource, Default)]
pub struct Breeze(pub f32);

impl MaterialExtension for Wind {
    fn vertex_shader() -> ShaderRef {
        "shaders/wind.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/wind_prepass.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/wind_prepass.wesl".into()
    }
}

/// How many faces a chunk's mesh has: what a scene's `pace` adds up for the ones in view.
#[derive(Component)]
pub struct Faces(pub u32);

/// The chunks whose cells have changed since they were last meshed.
#[derive(Resource, Default)]
pub struct Dirty(pub HashSet<IVec3>);

/// The regions of the world whose cells are made (`terrain::REGION` a side), and those that
/// someone has dug or built in, which are kept however far off the player goes: nothing is
/// saved yet, and a hole must not heal behind its digger.
#[derive(Resource, Default)]
pub struct Regions {
    made: HashSet<(i32, i32)>,
    /// Regions being made on other threads: their cells, and the shapes of what grew.
    pending: std::collections::HashMap<(i32, i32), bevy::tasks::Task<(Voxels, cube_sim::plants::Form)>>,
    pub kept: HashSet<(i32, i32)>,
    /// Changed regions that have been dropped and made again from the seed, and have not
    /// yet had what was done to them done again (`cube_sim::loose::replay`).
    stale: HashSet<(i32, i32)>,
    /// A group of them having its edits done again, a few milliseconds a frame: by the
    /// least of its regions.
    replaying: std::collections::HashMap<(i32, i32), cube_sim::loose::Replay>,
    /// (A script's `forget`: every changed region is to be dropped now, however near, to
    /// try that it comes back the same; and what it all came to before, to hold it to.)
    pub forget: bool,
    held: Option<usize>,
    was: Vec<(IVec3, std::sync::Arc<cube_voxel::Chunk>)>,
    /// Regions wanted and not yet made, and which.
    pub waiting: usize,
    unmade: HashSet<(i32, i32)>,
}

/// What making the world has cost so far, seconds and how many: regions' cells, chunks'
/// meshes (and how many of those were thrown away and made again), regions' trees and rocks.
/// A scripted strip says it (`shot`), so where a slow load or a slow frame comes from is
/// read, not guessed.
#[derive(Resource, Default, Clone, Copy)]
pub struct Cost {
    pub fill: f32,
    pub fills: u32,
    pub mesh: f32,
    pub meshes: u32,
    pub remade: u32,
    pub grow: f32,
    pub grows: u32,
}

impl Cost {
    /// What was spent since `then`, in a few words.
    pub fn since(&self, then: &Cost) -> String {
        format!("cells {:.2}s/{} · meshes {:.2}s/{} ({} remade) · trees {:.2}s/{}", self.fill - then.fill, self.fills - then.fills, self.mesh - then.mesh, self.meshes - then.meshes, self.remade - then.remade, self.grow - then.grow, self.grows - then.grows)
    }
}

/// A region's cells are made when the camera comes within this of its middle, metres, and
/// dropped when it is further than the other. Creatures are lived to 110 m
/// (`cube_sim::life::AWAKE`), well inside.
const MAKE: f32 = 208.0;
const DROP: f32 = 272.0;

impl Regions {
    pub fn is_made(&self, region: (i32, i32)) -> bool {
        self.made.contains(&region)
    }
}

pub(crate) fn region_of(cell: IVec3) -> (i32, i32) {
    (cell.x.div_euclid(terrain::REGION), cell.z.div_euclid(terrain::REGION))
}

/// The two materials every chunk is drawn with.
#[derive(Resource)]
pub struct Look {
    pub solid: Handle<Windy>,
    /// The same paint with nothing cut out of it and only its outsides drawn: for what is
    /// made of whole cubes (trees and rocks as `flora` draws them). The cut-out one is far
    /// dearer to draw where large faces lie over one another, as a crown's boxes do.
    pub whole: Handle<Windy>,
    water: Handle<StandardMaterial>,
    /// The sea, and far water: not seen through.
    pub sea: Handle<StandardMaterial>,
}

impl Terrain {
    /// How well the spot at `x`, `z` suits a kind of life at this hour, 0 to 1 (`ecology`).
    pub fn fit(&self, niche: &cube_sim::ecology::Niche, x: f32, z: f32, hour: f32) -> f32 {
        niche.fit(&self.2.place(&self.1, x, z), hour)
    }

    /// Marks chunks that have changed to be drawn again, and those beside them (a face at a
    /// chunk's edge is its neighbour's to hide or show); the regions they are in are kept.
    pub fn touched(&mut self, dirty: &mut Dirty, chunks: impl IntoIterator<Item = IVec3>) {
        for chunk in chunks {
            self.3.insert(region_of(chunk * CHUNK));
            dirty.0.insert(chunk);
            for side in [IVec3::X, IVec3::Y, IVec3::Z] {
                dirty.0.insert(chunk + side);
                dirty.0.insert(chunk - side);
            }
        }
    }

    /// The ground's height at a point, metres: the land itself, not a tree over it.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.1.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL
    }
}

/// What every camera that looks at the world carries: the sky as real air scattering the
/// sun (so the light's colour, the sky's and the haze's all come from where the sun is), that
/// sky lighting what the sun does not reach, an exposure for full sunlight, a film-like roll
/// off of the brightest light, a glow round it, and haze that warms toward the sun.
pub fn look() -> impl Bundle {
    (
        DistanceFog {
            color: Color::srgba(0.62, 0.74, 0.88, 1.0),
            directional_light_color: Color::srgba(1.0, 0.86, 0.62, 0.6),
            directional_light_exponent: 14.0,
            falloff: FogFalloff::from_visibility_colors(900.0, Color::srgb(0.42, 0.50, 0.58), Color::srgb(0.78, 0.82, 0.86)),
        },
        AtmosphereSettings::default(),
        // The sky fills shadows, but not so far that they stop being shadows.
        AtmosphereEnvironmentMapLight { intensity: 1.0, ..default() },
        Exposure { ev100: DAY_EV },
        Tonemapping::TonyMcMapface,
        Bloom { intensity: 0.06, ..Bloom::NATURAL },
        // Richer than the tonemapper leaves it, a little warm, with more between light and dark.
        ColorGrading {
            global: ColorGradingGlobal::default(),
            shadows: ColorGradingSection::default(),
            midtones: ColorGradingSection::default(),
            highlights: ColorGradingSection::default(),
        },
        // Corners, the feet of trunks and the undersides of crowns are darker: depth that the
        // cubes' own baked shade cannot give between one thing and another.
        Msaa::Off,
        ScreenSpaceAmbientOcclusion::default(),
        // The mist is lit: shafts of sun between trunks.
        VolumetricFog { ambient_intensity: 0.0, step_count: 48, ..default() },
    )
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        if app.world().resource::<crate::Args>().reflect {
            // What screen-space reflections need, on every camera the moment it is made: a
            // camera a frame without its prepasses, with deferred materials about, is a panic.
            app.insert_resource(bevy::pbr::DefaultOpaqueRendererMethod::deferred()).add_observer(|add: On<Add<Camera3d>>, mut commands: Commands| {
                commands.entity(add.entity).insert((// A lake's far bank is forty metres off: more steps than the default ten, a thicker
                // world, and no lower limit on how smooth a thing may be and still reflect.
                bevy::pbr::ScreenSpaceReflections { min_perceptual_roughness: 0.0..0.0, linear_steps: 48, thickness: 1.5, ..default() }, bevy::core_pipeline::prepass::DepthPrepass, bevy::core_pipeline::prepass::DeferredPrepass, bevy::anti_alias::taa::TemporalAntiAliasing::default()));
            });
        }
        app.add_plugins(MaterialPlugin::<Windy>::default()).insert_resource(ClearColor(Color::BLACK)).init_resource::<Dirty>().init_resource::<Drawn>().init_resource::<Cost>().init_resource::<Breeze>().add_systems(Startup, setup).add_systems(Update, (regions, stream).chain()).init_resource::<Roofed>().add_systems(Update, (roofed, sky, ripple, seasons));
    }
}

/// The sun and the moon of the world. Where they stand and how bright they are is set every
/// frame from the hour (`sky`); the sun's light is raw sunlight, and the air between (`look`)
/// is what colours it.
#[derive(Component)]
struct Sun;
#[derive(Component)]
struct Moon;

/// The water's materials, for `ripple` to slide their ripples.
#[derive(Resource)]
struct Waters(Vec<Handle<StandardMaterial>>);

/// Ripples: which way each point of the water's surface faces, as a tile that repeats. A few
/// wave trains crossing at angles, short and low.
fn ripples() -> Image {
    const SIDE: usize = 128;
    // Whole numbers of waves across the tile each way, how high, and where in its swing.
    let trains: [(f32, f32, f32, f32); 4] = [(3.0, 1.0, 0.5, 0.0), (-2.0, 4.0, 0.35, 1.3), (5.0, -3.0, 0.22, 2.1), (7.0, 6.0, 0.14, 0.7)];
    let mut level: Vec<Vec3> = (0..SIDE * SIDE).map(|i| {
        let (u, v) = ((i % SIDE) as f32 / SIDE as f32, (i / SIDE) as f32 / SIDE as f32);
        let (mut dx, mut dy) = (0.0, 0.0);
        for (kx, ky, high, at) in trains {
            let slope = (std::f32::consts::TAU * (kx * u + ky * v) + at).cos() * high;
            dx += slope * kx;
            dy += slope * ky;
        }
        Vec3::new(-dx * 0.012, -dy * 0.012, 1.0).normalize()
    }).collect();
    // Smaller copies, each four texels averaged, down to one: far off the ripples flatten to
    // still water. With only the full picture, distant water took a different ripple at every
    // pixel and the low sun made it a sheet of white sparkle.
    let (mut data, mut side, mut levels) = (Vec::new(), SIDE, 0);
    loop {
        data.extend(level.iter().flat_map(|n| {
            let n = n.normalize();
            [n.x, n.y, n.z].map(|c| ((c * 0.5 + 0.5) * 255.0) as u8).into_iter().chain([255])
        }));
        levels += 1;
        if side == 1 {
            break;
        }
        let half = side / 2;
        level = (0..half * half).map(|i| {
            let (x, y) = (i % half * 2, i / half * 2);
            (level[x + y * side] + level[x + 1 + y * side] + level[x + (y + 1) * side] + level[x + 1 + (y + 1) * side]) / 4.0
        }).collect();
        side = half;
    }
    let mut image = Image::new_uninit(Extent3d { width: SIDE as u32, height: SIDE as u32, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat, mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, anisotropy_clamp: 8, ..default() });
    image
}

/// The ripples run downwind.
fn ripple(time: Res<Time>, args: Res<crate::Args>, live: Option<Res<crate::motion::Live>>, waters: Option<Res<Waters>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let _slow = crate::Slow::new("ripple");
    let Some(waters) = waters else { return };
    // A low sun glances off every ripple at once and the lake breaks into white speckle:
    // the water is made duller as the sun drops, as haze over real water does.
    let up = cube_sim::sky::sun(live.map_or(cube_sim::sky::START, |l| l.0.when)).y;
    let dull = 1.0 - ((up - 0.1) / 0.45).clamp(0.0, 1.0);
    let reflecting = args.reflect;
    for handle in &waters.0 {
        if let Some(mut water) = materials.get_mut(handle) {
            // (Reflecting the land, it is kept as smooth as a mirror wants: `--reflect`.)
            let mirror = reflecting;
            water.perceptual_roughness = if mirror { 0.12 } else { 0.1 + 0.6 * dull };
            water.reflectance = if mirror { 1.0 } else { 0.5 - 0.3 * dull };
            water.uv_transform = bevy::math::Affine2::from_translation(Vec2::new(-0.8, -0.6) * (time.elapsed_secs() * 0.07));
        }
    }
}

/// How far across one tile of cloud shadow is on the ground, metres (the light's own size is
/// half of it: a tile is two of the light's units), and how fast it drifts.
const CLOUD_TILE: f32 = 420.0;
const CLOUD_DRIFT: f32 = 2.2;

/// The shadows of clouds, as how much of the sun gets through at each point of a tile that
/// repeats: mostly clear, with soft-edged patches of shade some sixty metres over. Laid in
/// the sun's light and drifted downwind, it is what makes a hillside go dim and bright again.
fn clouds() -> Image {
    const SIDE: usize = 256;
    let hash = |x: i32, y: i32, n: i32| {
        let mut h = (x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77) ^ (n as u32).wrapping_mul(0xC2B2AE3D);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B3C6D);
        h ^= h >> 12;
        (h & 0xFFFF) as f32 / 65535.0
    };
    // Noise that repeats at the tile's edge, in four layers.
    let noise = |x: f32, y: f32| {
        (0..4).map(|o| {
            let cells = 4 << o;
            let (fx, fy) = (x * cells as f32, y * cells as f32);
            let (x0, y0) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - x0, fy - y0);
            let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
            let at = |dx: i32, dy: i32| hash((x0 as i32 + dx).rem_euclid(cells), (y0 as i32 + dy).rem_euclid(cells), o);
            ((at(0, 0) * (1.0 - sx) + at(1, 0) * sx) * (1.0 - sy) + (at(0, 1) * (1.0 - sx) + at(1, 1) * sx) * sy) / (1 << o) as f32
        }).sum::<f32>() / 1.875
    };
    let mut data = Vec::with_capacity(SIDE * SIDE * 4);
    for i in 0..SIDE * SIDE {
        let cover = ((noise((i % SIDE) as f32 / SIDE as f32, (i / SIDE) as f32 / SIDE as f32) - 0.54) / 0.12).clamp(0.0, 1.0);
        let through = 1.0 - 0.62 * cover * cover * (3.0 - 2.0 * cover);
        let byte = (through * 255.0) as u8;
        data.extend([byte, byte, byte, 255]);
    }
    let mut image = Image::new(Extent3d { width: SIDE as u32, height: SIDE as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat, mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, ..default() });
    image
}

fn spawn_sun(commands: &mut Commands, mediums: &mut Assets<ScatteringMedium>, images: &mut Assets<Image>) {
    commands.insert_resource(GlobalAmbientLight::NONE);
    commands.spawn(Atmosphere::earth(mediums.add(ScatteringMedium::earth(256, 256))));
    let shadows = || CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 220.0, ..default() }.build();
    commands.spawn((Sun, DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: true, ..default() }, shadows(), SunDisk { angular_size: SunDisk::EARTH.angular_size * 2.0, intensity: 1.0 }, VolumetricLight, DirectionalLightTexture { image: images.add(clouds()), tiled: true }));
    // Mist: thin lit air lying in the low ground and under the trees, so the sun comes
    // through the crowns in shafts and far woods stand in haze. Low, so the sky stays clear.
    commands.spawn((Mist, FogVolume { density_factor: 0.0011, scattering: 0.4, absorption: 0.0, light_tint: Color::srgb(1.0, 0.95, 0.86), ..default() }, Transform::from_xyz(0.0, SEA as f32 * CELL + 11.0, 0.0).with_scale(Vec3::new(800.0, 24.0, 800.0))));
    commands.spawn((Moon, DirectionalLight { illuminance: 0.0, shadow_maps_enabled: false, color: Color::srgb(0.62, 0.74, 1.0), ..default() }, shadows()));
}

/// Exposure in full day and in full night, and the moon's light. A night that is truly as
/// dark as night cannot be played in: the camera opens up twelve stops, as an eye does, and
/// the moon is brighter than a real one, so the land is dim and blue but can be read.
const DAY_EV: f32 = 13.4;
const NIGHT_EV: f32 = 1.3;
const MOONLIGHT: f32 = 3.6;

/// A number read off a table of (the sine of the sun's height, the number) pairs, straight
/// between one pair and the next.
fn by_height(table: &[(f32, f32)], sun_up: f32) -> f32 {
    let next = table.partition_point(|p| p.0 < sun_up);
    match (next.checked_sub(1).map(|i| table[i]), table.get(next)) {
        (Some(lo), Some(hi)) => lo.1 + (hi.1 - lo.1) * (sun_up - lo.0) / (hi.0 - lo.0),
        (lo, hi) => lo.or(hi.copied()).map_or(0.0, |p| p.1),
    }
}

/// The camera's exposure for a sun this high: what light there is falls away fast in the last
/// few degrees over the horizon and goes on falling for an hour after, and a noon sun is
/// brighter than an afternoon one. Set by eye against pictures at each hour (`make sky`), as
/// a photographer would, and for the sky as much as the land: with the sun on the horizon
/// the sky round it is far brighter than anything it lights, and an exposure set for the
/// land alone leaves a sunset white.
const EXPOSURE: [(f32, f32); 8] = [(-0.24, NIGHT_EV), (-0.16, 4.0), (-0.105, 7.0), (-0.03, 10.8), (0.05, 12.2), (0.2, 12.4), (0.35, 13.4), (0.85, 14.4)];

/// The land's own mist: the one body of air the sky thins and tints by the hour (a cave has
/// its own, which is not the sky's to change).
#[derive(Component)]
struct Mist;

/// How much of the sky is hidden from the eye by the land over it: 0 in the open, 1 in a cave
/// (already eased: it changes over a second or two, as an eye takes to opening).
#[derive(Resource, Default)]
pub struct Roofed(pub f32);

/// The exposure under a roof of rock by day: four stops open from noon's. What comes down a
/// shaft from the sky is then too bright to look at, as it is. (At five the patch of sun on
/// a cave's floor was white with nothing in it.)
const CAVE_EV: f32 = 10.5;
/// How much of the sky's fill still shows under a roof of rock, as it looks in the picture.
/// (At a twentieth, the shadows a torch threw in a cave by day were blue.)
const CAVE_SKY: f32 = 0.025;

/// The exposure at an hour, with that much of the sky hidden ([`Roofed`]).
fn exposed(sun_up: f32, roofed: f32) -> f32 {
    let open = by_height(&EXPOSURE, sun_up);
    open + (open.min(CAVE_EV) - open) * roofed
}

/// How many stops the eye is shut from the night's at this hour and under this much roof:
/// what has a light of its own must be that much stronger to look as it does at night
/// (`player::torch` gives it most of them).
pub(crate) fn shut(hour: f32, roofed: f32) -> f32 {
    exposed(cube_sim::sky::sun(hour).y, roofed) - NIGHT_EV
}

/// Measures [`Roofed`]: thirteen lines from the eye up into the sky, each asked whether land
/// stops it within forty metres (land: not a tree, whose shade is the sun's own business).
/// Nothing happens until two thirds of the sky is hidden, so a gorge and the foot of a cliff
/// are open country, and the mouth of a cave is where the eye begins to open.
fn roofed(time: Res<Time>, terrain: Option<Res<Terrain>>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut roofed: ResMut<Roofed>) {
    let (Some(terrain), Some(eye)) = (terrain, cameras.iter().next().map(|c| c.translation())) else { return };
    let stopped = |way: Vec3| {
        (1..=80).any(|k| {
            let block = terrain.0.get(((eye + way * (k as f32 * 0.5)) / CELL).floor().as_ivec3());
            block.solid() && !block.foliage() && !block.wood()
        })
    };
    let ways = std::iter::once((Vec3::Y, 1.0)).chain((0..12).map(|n| {
        let (turn, up) = (n as f32 * std::f32::consts::TAU / 6.0 + (n / 6) as f32 * 0.5, if n < 6 { 1.05f32 } else { 0.45 });
        (Vec3::new(turn.cos() * up.cos(), up.sin(), turn.sin() * up.cos()), up.sin())
    }));
    let (mut hidden, mut all) = (0.0, 0.0);
    for (way, weight) in ways {
        hidden += if stopped(way) { weight } else { 0.0 };
        all += weight;
    }
    let under = ((hidden / all - 0.65) / 0.35).clamp(0.0, 1.0);
    let want = under * under * (3.0 - 2.0 * under);
    roofed.0 += (want - roofed.0) * (time.delta_secs() * 1.5).min(1.0);
}

/// The sky follows the hour: the sun goes round and under, the moon comes up opposite it, the
/// camera opens as the light goes, and the haze takes the colour of the hour.
fn sky(
    live: Option<Res<crate::motion::Live>>,
    time: Res<Time>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight, &mut SunDisk), (With<Sun>, Without<Moon>)>,
    mut moon: Query<(&mut Transform, &mut DirectionalLight), (With<Moon>, Without<Sun>)>,
    terrain: Option<Res<Terrain>>,
    mut cameras: Query<(&mut Exposure, &mut DistanceFog, &mut ColorGrading, &mut AtmosphereEnvironmentMapLight, Option<&mut bevy::light::EnvironmentMapLight>, &mut Bloom, &GlobalTransform)>,
    mut mists: Query<&mut FogVolume, With<Mist>>,
    torches: Query<(), With<crate::player::Torch>>,
    roofed: Res<Roofed>,
) {
    let _slow = crate::Slow::new("sky");
    let hour = live.map_or(cube_sim::sky::START, |l| l.0.when);
    let (to_sun, day) = (cube_sim::sky::sun(hour), cube_sim::sky::daylight(hour));
    if let Ok((mut at, mut light, mut disk)) = sun.single_mut() {
        // The disc itself is drawn as bright as the sun is, which any exposure leaves white:
        // true at noon, and at the land's edge a white dot where a red sun should be. It is
        // dimmed there, a great deal, until what is left is its colour.
        // (And under a quarter of the way up it is not drawn at all: the engine's disc has
        // no red in it at any brightness, and `vault` draws a low sun instead.)
        let high = ((to_sun.y - 0.12) / 0.28).clamp(0.0, 1.0);
        // (And under cloud it is a pale place in the grey, if that.)
        let clear = 1.0 - overcast(hour);
        disk.intensity = if high > 0.0 { 10f32.powf(-3.0 * (1.0 - high * high * (3.0 - 2.0 * high))) * clear * clear * clear } else { 0.0 };
        // Where it stands and how big it is are where the cloud shadows lie and how big they
        // are: they drift downwind.
        let drift = Vec3::new(0.8, 0.0, 0.6) * (time.elapsed_secs() * CLOUD_DRIFT);
        *at = Transform::from_translation(drift).with_scale(Vec3::splat(CLOUD_TILE / 2.0)).looking_to(-to_sun, Vec3::Y);
        // The air reddens and dims it toward the horizon; this only puts it out below.
        let up = ((to_sun.y + 0.03) / 0.06).clamp(0.0, 1.0);
        // And a low sun is made stronger than the air would leave it. True to life, the last
        // hour of sun is weak and the sky's flat fill takes the picture; the gold of evening
        // is wanted more than the truth.
        let low = 1.0 - ((to_sun.y - 0.05) / 0.4).clamp(0.0, 1.0);
        // Nor is it put out here as it goes under: the engine's own shading takes its light
        // off the land where the world is in the way, and the air over the horizon is still
        // lit by it, which is twilight. Only well under (fifteen degrees: night proper) is
        // it gone, so the last of the glow fades out and is not cut off.
        let glow = ((to_sun.y + 0.26) / 0.1).clamp(0.0, 1.0);
        // Under cloud the sun itself is mostly gone and the sky's own light is what is left:
        // a dull day is flat and without hard shadows. (`sky::cloud`; rain fell out of a
        // sunlit sky until this.)
        let cloud = overcast(hour);
        light.illuminance = lux::RAW_SUNLIGHT * glow * (1.0 + 0.8 * low * low) * (1.0 - (0.7 + 0.3 * low) * cloud * cloud);
        // The air here reddens a low sun only a little; an evening wants amber.
        light.color = Color::srgb(1.0, 1.0 - 0.08 * low * low, 1.0 - 0.18 * low * low);
        // Mist thins as the sun drops, or the low light floods it and whites the picture out,
        // and what is left takes the sun's colour.
        for mut mist in &mut mists {
            mist.density_factor = 0.0011 * (1.0 - 0.7 * low);
            mist.light_tint = Color::srgb(1.0, 0.95 - 0.04 * low, 0.86 - 0.08 * low);
        }
        light.shadow_maps_enabled = up > 0.0;
    }
    if let Ok((mut at, mut light)) = moon.single_mut() {
        // Opposite the sun and a little aside, so it is up for the whole night.
        // Where the calendar puts it, and as bright as it is full (`sky::moonlight`): a new
        // moon gives nothing, and the night is starlight alone.
        let to_moon = cube_sim::sky::moon(hour);
        *at = Transform::default().looking_to(-to_moon, Vec3::Y);
        light.illuminance = MOONLIGHT * cube_sim::sky::moonlight(hour) * (1.0 - day);
        light.shadow_maps_enabled = day < 0.4 && to_moon.y > 0.05;
    }
    // What the moon does not reach is not black.
    let fire = !torches.is_empty();
    // (Less blue where a torch is: on pale bark beside the fire's amber it was cobalt.)
    let cold = if fire { Color::srgb(0.6, 0.68, 0.86) } else { Color::srgb(0.45, 0.6, 1.0) };
    // Under a roof of rock the sky's fill is all but gone, and the eye opens (`Roofed`): what
    // is left of it is set by how it looks in the picture, whatever the exposure has become.
    let open = by_height(&EXPOSURE, to_sun.y);
    let sheltered = 1.0 + (CAVE_SKY * 2f32.powf(exposed(to_sun.y, 1.0) - open) - 1.0) * roofed.0;
    // Under an overcast with the sun low there is no sun on the land and the air's own light
    // is thin: the land at a dull dawn was black. The clouded sky fills it then, as the
    // night's floor does the night, as bright in the picture at whatever the exposure is.
    let dull = overcast(hour).powi(2) * (1.0 - ((to_sun.y - 0.05) / 0.3).clamp(0.0, 1.0)) * ((to_sun.y + 0.24) / 0.1).clamp(0.0, 1.0);
    let fill = 3.0 * dull * 2f32.powf(open - NIGHT_EV);
    let cold = if dull > 0.0 { Color::srgb(0.72, 0.76, 0.84) } else { cold };
    *ambient = GlobalAmbientLight { color: cold, brightness: ((0.55 + 0.1 * cube_sim::sky::moonlight(hour)) * (1.0 - day) + fill) * sheltered, ..default() };
    // The haze is what the far land and the far sea go into, so it has to be the colour the
    // sky has at its edge at that hour: where it was not, the sea was a bar under the sky.
    // It is as bright as the sky's edge is in the picture, which stays bright all through
    // twilight (the exposure follows the light down) and only then falls to the night's.
    let low = (1.0 - to_sun.y.abs() * 4.0).clamp(0.0, 1.0);
    let lit = ((to_sun.y + 0.24) / 0.14).clamp(0.0, 1.0);
    let lit = lit * lit * (3.0 - 2.0 * lit);
    // (These are the picture's numbers, as `Color::srgb` takes them. The night's were first
    // written as light, a twelfth as bright, and the sea was still a bar. And at night a
    // little under the sky's edge: the far mountain goes into it too, and at the sky's own
    // tone it stood lighter than the sky behind its top.)
    let dark = (Vec3::new(0.086, 0.106, 0.165) + Vec3::new(0.1, 0.095, 0.075) * cube_sim::sky::moonlight(hour)) * (1.0 - lit);
    // Away from the sun the day's pale blue, mauve at a low sun; toward it warm, and at a
    // low sun the orange the sky has there, spread wide.
    // (Not quite as bright once the sun is under: the land near to is dark by then, and haze
    // as bright as the sky lay on it like milk.)
    let lit = lit * (0.5 + 0.5 * day);
    // Under an overcast it is the grey of the clouded sky (`vault`), toward the sun too.
    let grey = overcast(hour);
    let grey = grey * grey;
    let clouded = Vec3::new(0.64, 0.67, 0.71);
    let away = Vec3::new(0.62, 0.74, 0.88).lerp(Vec3::new(0.72, 0.57, 0.57), low).lerp(clouded, grey) * lit + dark;
    let toward = Vec3::new(1.0, 0.86, 0.62).lerp(Vec3::new(1.0, 0.62, 0.4), low).lerp(clouded, grey);
    for (mut exposure, mut fog, mut grade, mut skylight, filled, mut bloom, eye) in &mut cameras {
        exposure.ev100 = exposed(to_sun.y, roofed.0);
        // What has a light of its own (a flame, the moon, a firefly) has more of a glow
        // round it in the dark, as to an eye opened for it.
        bloom.intensity = 0.06 + 0.1 * (1.0 - day);
        // (As the air gives it at every hour. A table that gave it three times over at a low
        // sun was written on 2026-11-12 and, set where the engine does not look, never did
        // anything; with it working the evening's land was noon-green under a sunset.)
        skylight.intensity = sheltered;
        // (The engine reads the strength from what it made of that when it was put on the
        // camera, and never again: set there alone, as it was until 2026-11-14, it did nothing.)
        if let Some(mut filled) = filled {
            filled.intensity = skylight.intensity;
        }
        // By moonlight there is little colour to see.
        // No more colour than the light gives: the grade only takes some away at night.
        // (Moonlight, not twilight: the hour after sunset has the most colour of the day.
        // Night proper begins with the sun fifteen degrees under.) And a little more than
        // the tonemapper leaves while the sun is at the horizon, which is when it takes most.
        let dusk = ((to_sun.y + 0.26) / 0.15).clamp(0.0, 1.0);
        let golden = (1.0 - (to_sun.y / 0.25).abs()).max(0.0);
        // (Nor by firelight: a torch's pool is amber, and with the night's colour drained
        // from it it was straw.)
        let fire = if fire { 0.35 } else { 0.0 };
        grade.global.post_saturation = (0.6 + 0.4 * dusk + 0.15 * golden).max(0.6 + fire);
        // With the sun under, the air's own twilight is a dull brown-red all the way up; the
        // hour is wanted blue over a red rim, as the eye has it. Cooler, until night proper.
        let blue = ((-0.02 - to_sun.y) / 0.1).clamp(0.0, 1.0) * ((to_sun.y + 0.26) / 0.1).clamp(0.0, 1.0);
        // (More so the further under: the air's twilight browns as it dims.)
        let deep = 1.0 + 0.7 * ((-0.1 - to_sun.y) / 0.1).clamp(0.0, 1.0);
        // (Under an overcast there is no blue hour: the grade turned the grey sky blue.)
        let blue = blue * (1.0 - 0.85 * grey);
        grade.global.temperature = -0.07 * blue * deep;
        grade.global.tint = 0.05 * blue * deep;
        fog.color = Color::srgba(away.x, away.y, away.z, 1.0);
        // (The glare toward the sun is the sun's own light times this, and the exposure
        // opens three stops for a low sun: left alone it grew with it into a custard that
        // outshone the sky. It is held to what it is at noon.)
        // (A seventh of it stays on a while after the sun is under: the sky toward a sun just
        // set is still its orange, and so must the sea under it be. All of it left on, the
        // land glowed tan toward a sun that had set, where it should be dark under the sky.)
        let after = 0.85 * ((to_sun.y + 0.02) / 0.04).clamp(0.0, 1.0) + 0.15 * ((to_sun.y + 0.14) / 0.1).clamp(0.0, 1.0);
        // (And it is given as how bright the haze straight toward the sun is in the picture,
        // whatever the sun's strength and the exposure are: three times white at noon, in a
        // narrow lobe; little over one at a low sun, whose lobe is wide. Left to follow the sun it
        // was ten at sunset, and a fifth of that, which is what sixty metres of air takes,
        // painted the crowns across a lake one flat tan, brighter than the sky.)
        let low_sun = 1.0 - ((to_sun.y - 0.05) / 0.4).clamp(0.0, 1.0);
        let sunlight = lux::RAW_SUNLIGHT * ((to_sun.y + 0.26) / 0.1).clamp(0.0, 1.0) * (1.0 + 0.8 * low_sun * low_sun);
        let shown = sunlight / (1.2 * 2f32.powf(exposure.ev100));
        let glare = if shown > 1e-4 { (3.0 - 1.8 * low) * after / shown } else { 0.0 };
        fog.directional_light_color = Color::srgba(toward.x, toward.y, toward.z, glare);
        fog.directional_light_exponent = 14.0 - 8.0 * low;
        // Under the water: green-blue murk a few metres deep, dimmer the further down.
        let eye = eye.translation();
        let under = terrain.as_ref().is_some_and(|t| eye.y < SEA as f32 * CELL - 0.05 && t.ground(eye.x, eye.z) < eye.y);
        if under {
            let down = (SEA as f32 * CELL - eye.y).min(6.0) / 6.0;
            fog.color = Color::srgba(0.05 * lit, (0.3 - 0.12 * down) * lit, (0.36 - 0.12 * down) * lit, 1.0);
            fog.directional_light_color = Color::NONE;
            fog.falloff = FogFalloff::Exponential { density: 0.22 };
        } else if matches!(fog.falloff, FogFalloff::Exponential { .. }) {
            fog.falloff = FogFalloff::from_visibility_colors(900.0, Color::srgb(0.42, 0.50, 0.58), Color::srgb(0.78, 0.82, 0.86));
        }
    }
}

/// Plain studio light, for contact sheets.
pub fn spawn_lights(commands: &mut Commands, shadows: bool) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.85, 1.0), brightness: 480.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3_800.0, shadow_maps_enabled: shadows, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.9, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 160.0, ..default() }.build(),
    ));
}

fn setup(mut commands: Commands, args: Res<crate::Args>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>, mut mediums: ResMut<Assets<ScatteringMedium>>, mut windy: ResMut<Assets<Windy>>) {
    // The lie of the whole island; its cells are made a region at a time as the camera
    // comes near (`regions`), starting here with the ground under the start.
    let land = Land::new(args.seed, terrain::RADIUS);
    let mut voxels = Voxels::default();
    let start = ((args.at.0 / CELL) as i32, (args.at.1 / CELL) as i32);
    let first = Regions::default();
    let _ = start;
    commands.insert_resource(first);
    cube_voxel::mesh::MODELLED.store(!args.blocky, std::sync::atomic::Ordering::Relaxed);
    // The paint: grain for every surface, made in code (`cube_voxel::texture`). Drawn smooth,
    // so it reads as paint and not as pixels; far off the smaller copies take over, so distant
    // ground does not sparkle.
    // Surfaces that have a photographed material (`assets/textures/NAME.jpg` and `NAME_n.jpg`,
    // CC0 from ambientCG) use it; the rest, and all of them with `--plain`, are made in code.
    let textures = std::path::Path::new(&crate::asset_root()).join("textures");
    let plain = args.plain;
    let atlas = cube_voxel::texture::atlas_with(|grain| {
        if plain {
            return None;
        }
        let name = format!("{grain:?}").to_lowercase();
        let open = |file: String| image::open(textures.join(file)).ok().map(|i| i.resize_exact(cube_voxel::texture::TILE as u32, cube_voxel::texture::TILE as u32, image::imageops::FilterType::Triangle).to_rgb8());
        let (colour, facing) = (open(format!("{name}.jpg"))?, open(format!("{name}_n.jpg"))?);
        let linear = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        Some(cube_voxel::texture::Photo { colour: colour.pixels().map(|p| p.0.map(linear)).collect(), facing: facing.pixels().map(|p| p.0.map(|v| v as f32 / 127.5 - 1.0)).collect() })
    });
    let side = atlas.size as u32;
    let mut picture = |bytes: Vec<u8>, format: TextureFormat| {
        let mut image = Image::new_uninit(Extent3d { width: side, height: side, depth_or_array_layers: 1 }, TextureDimension::D2, format, RenderAssetUsages::RENDER_WORLD);
        image.texture_descriptor.mip_level_count = atlas.levels as u32;
        image.data = Some(bytes);
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, lod_max_clamp: (atlas.levels - 1) as f32, anisotropy_clamp: 4, ..default() });
        images.add(image)
    };
    // The paint, and the relief that goes with it: which way each point of a surface faces.
    let (paint, relief) = (picture(atlas.rgba, TextureFormat::Rgba8UnormSrgb), picture(atlas.relief, TextureFormat::Rgba8Unorm));
    let whole = windy.add(ExtendedMaterial { base: StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(paint.clone()), normal_map_texture: Some(relief.clone()), perceptual_roughness: 0.95, reflectance: 0.2, ..default() }, extension: Wind { season: Vec4::new(1.0, 1.0, 1.0, 0.0), wind: Vec4::splat(0.4) } });
    let material = windy.add(ExtendedMaterial { base: StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(paint), normal_map_texture: Some(relief), perceptual_roughness: 0.95, reflectance: 0.2,
        // Foliage has gaps cut in it (the atlas's alpha), and through a gap the inside of the
        // crown's far side is seen: so nothing is culled by which way it faces.
        // (Seen from both sides, but not lit as two-sided: grass is lit as the ground it grows
        // from, facing up, and a blade seen from behind was then lit from under: black against
        // a torch, and dark among the others by day.)
        alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: false, ..default() }, extension: Wind { season: Vec4::new(1.0, 1.0, 1.0, 0.0), wind: Vec4::splat(0.4) } });
    // Water is its own mesh and material: it is blended over what is under it.
    // And it moves: ripples, as a picture of which way the surface faces, slid across it by
    // `ripple`. Smooth enough to throw the sun back as a glitter path.
    let waves = images.add(ripples());
    let water = materials.add(StandardMaterial { base_color: Color::WHITE, // Not seen through: blended water writes no depth, so the mist behind it was
        // measured to the lake's bed, and every terrace of the bed showed as a stripe across
        // the water. The shallows are paler by colour instead.
        alpha_mode: AlphaMode::Opaque, perceptual_roughness: 0.3, reflectance: 0.4, normal_map_texture: Some(waves.clone()), ..default() });
    for &(x, z, across, build) in &args.dug {
        let ground = land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL;
        // A hole is a bowl with its rim at the ground; a stone sits two thirds out of it.
        let (lift, block) = if build { (across / 6.0, Block::Stone) } else { (across * 0.1, Block::Air) };
        voxels.ball(Vec3::new(x, ground + lift, z), across / 2.0, block);
    }
    // The chunks are meshed as the camera comes near them (`stream`), not here.

    // The sea beyond the world's edge: four sheets round it at the water's level, out past the fog.
    let (edge, far, level) = (land.span as f32 * CELL, 1500.0, SEA as f32 * CELL - 0.01);
    let [r, g, b] = cube_voxel::Block::Water.colors().0;
    let sea = materials.add(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: 0.09, reflectance: 0.6, normal_map_texture: Some(waves), uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(far * 2.0 / cube_voxel::mesh::WATER_TILE)), ..default() });
    commands.insert_resource(Waters(vec![water.clone()]));
    commands.insert_resource(Look { solid: material, whole, water: water.clone(), sea: sea.clone() });
    for (x, z, w, d) in [(0.0, -(edge + far) / 2.0, far, (far - edge) / 2.0), (0.0, (edge + far) / 2.0, far, (far - edge) / 2.0), (-(edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge), ((edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge)] {
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(w * 2.0, d * 2.0))), MeshMaterial3d(sea.clone()), Transform::from_xyz(x, level, z)));
    }
    // And plain sea on to the horizon. Past the sheets' edge the engine's own ground showed
    // from anywhere high, a tan slab under the sky with the far land's hazed trees standing
    // against it. Out there it is all the haze's colour, as they are, and needs no waves.
    let out = 40_000.0;
    let beyond = materials.add(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: 0.5, ..default() });
    for (x, z, w, d) in [(0.0, -(far + out) / 2.0, out, (out - far) / 2.0), (0.0, (far + out) / 2.0, out, (out - far) / 2.0), (-(far + out) / 2.0, 0.0, (out - far) / 2.0, far), ((far + out) / 2.0, 0.0, (out - far) / 2.0, far)] {
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(w * 2.0, d * 2.0))), MeshMaterial3d(beyond.clone()), Transform::from_xyz(x, level, z), bevy::light::NotShadowCaster));
    }
    // The island measured for living in (`cube_sim::ecology`): what the small life the game
    // draws is placed by.
    let ecology = cube_sim::ecology::Ecology::new(&land);
    commands.insert_resource(Terrain(voxels, std::sync::Arc::new(land), ecology, HashSet::new()));
    spawn_sun(&mut commands, &mut mediums, &mut images);
}

/// The levels of detail a chunk is drawn at: how many cells a side are taken as one, and the
/// metres from the camera between which that level shows. Each fades into the next across
/// eight metres, so nothing pops. Up close every cell is drawn, with ground cover and the
/// fine shape of broken cells; past 40 m the cells are half a metre, a quarter of the faces;
/// past 104 m a metre, a sixteenth; past 168 m two metres, so that the land can be made out
/// to 208 m and seen far for little.
pub const LEVELS: [(i32, f32, f32); 4] = [(1, 0.0, 40.0), (2, 40.0, 104.0), (4, 104.0, 168.0), (8, 168.0, f32::MAX)];
const FADE: f32 = 8.0;

/// How far outside a level's range a chunk's mesh for it is made (so it is there before it is
/// wanted) and, at twice this, thrown away.
const KEEP: f32 = 12.0;

/// The chunk meshes there are: for each chunk and level, what was spawned for it (nothing, for
/// a chunk with no faces at that level). And how many are still wanted and not yet made.
#[derive(Resource, Default)]
pub struct Drawn {
    /// What is still drawn of a chunk that has changed, until what replaces it is ready: taken
    /// away at once, a hole was dug in a frame of nothing (the ground flickered at every
    /// stroke of the shovel).
    old: std::collections::HashMap<(IVec3, usize), Vec<Entity>>,
    made: std::collections::HashMap<(IVec3, usize), Vec<Entity>, cube_voxel::Quick>,
    /// What is wanted and not made, nearest first, and how many more wait on ground that is on
    /// its way. Found by looking at every chunk there is, which is milliseconds: so the look
    /// is a pass over `keys` a part each frame (`at`), gathered in `next` and swapped in when
    /// the pass ends. `stale`: passes still to finish before what is here can be believed
    /// (something changed), which a picture waits for.
    wanted: Vec<(f32, IVec3, usize)>,
    held: usize,
    keys: Vec<IVec3>,
    at: usize,
    next: (Vec<(f32, IVec3, usize)>, usize),
    stale: u8,
    looked: usize,
    /// Meshes being worked out on other threads.
    pending: std::collections::HashMap<(IVec3, usize), bevy::tasks::Task<Vec<(bool, cube_voxel::mesh::MeshData)>>, cube_voxel::Quick>,
    pub waiting: usize,
}

impl Drawn {
    /// Of the chunk meshes wanted within `near` metres of the eye, how many are made, and how
    /// many are wanted: what the loading screen counts.
    pub fn near(&self, voxels: &Voxels, eye: Vec3, near: f32) -> (usize, usize) {
        let (mut made, mut wanted) = (0, 0);
        for &chunk in voxels.chunks.keys() {
            let away = eye.distance(middle_of(chunk));
            for (level, &(_, from, to)) in LEVELS.iter().enumerate() {
                if away < near && away > from - FADE - KEEP && away < to + KEEP {
                    wanted += 1;
                    made += self.made.contains_key(&(chunk, level)) as usize;
                }
            }
        }
        (made, wanted)
    }
}

fn middle_of(chunk: IVec3) -> Vec3 {
    ((chunk * CHUNK).as_vec3() + CHUNK as f32 / 2.0) * CELL
}

/// The mesh of one chunk at one level, its solid cells and its water, about the chunk's
/// middle: the long part of drawing a chunk, and done on another thread from a copy of the
/// world round it ([`Voxels::around`]).
fn mesh_level(voxels: &Voxels, chunk: IVec3, level: usize) -> Vec<(bool, cube_voxel::mesh::MeshData)> {
    // Every level of a chunk is drawn from the chunk's middle, so the camera's distance to it
    // is the same whichever level is asked: the fade between two levels needs that.
    let (middle, step) = (middle_of(chunk), LEVELS[level].0);
    [false, true]
        .into_iter()
        .filter_map(|liquid| {
            let mut data = if step == 1 { mesh_chunk(voxels, chunk, liquid) } else { mesh_chunk_far(voxels, chunk, liquid, step) };
            data.positions.iter_mut().for_each(|p| *p = (Vec3::from_array(*p) - middle).to_array());
            (!data.indices.is_empty()).then_some((liquid, data))
        })
        .collect()
}

/// Spawns what a chunk's meshes draw.
fn draw_level(commands: &mut Commands, meshes: &mut Assets<Mesh>, look: &Look, terrain: &Terrain, chunk: IVec3, level: usize, made: Vec<(bool, cube_voxel::mesh::MeshData)>) -> Vec<Entity> {
    let middle = middle_of(chunk);
    let (_, from, to) = LEVELS[level];
    let range = VisibilityRange { start_margin: (from - FADE).max(0.0)..from, end_margin: if to == f32::MAX { 1.0e9..1.1e9 } else { to - FADE..to }, use_aabb: false };
    let mut spawned = Vec::new();
    for (liquid, data) in made {
        let faces = Faces(data.indices.len() as u32 / 6);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.tangents)
            .with_inserted_indices(Indices::U32(data.indices));
        let mut drawn = commands.spawn((Mesh3d(meshes.add(mesh)), Transform::from_translation(middle), range.clone(), faces));
        if liquid {
            // Water takes no shadows: at a low sun the shadow map breaks up on a flat sheet
            // and the whole lake was white speckle.
            // (Unless it lies under the land, as a pool in a cave does: there the sun's whole
            // light came through the rock onto it, and it shone in the dark.)
            drawn.insert((MeshMaterial3d(look.water.clone()), bevy::light::NotShadowCaster));
            if terrain.ground(middle.x, middle.z) < middle.y + CHUNK as f32 * CELL + 2.0 {
                drawn.insert(bevy::light::NotShadowReceiver);
            }
        } else {
            drawn.insert(MeshMaterial3d(look.solid.clone()));
        }
        spawned.push(drawn.id());
    }
    spawned
}

/// How many meshes may be in the making at once: enough to keep every thread busy, few
/// enough that what is asked for is still what is nearest when the camera moves.
const AT_ONCE: usize = 96;
/// How many corners of new meshes a frame of play takes in.
const UPLOAD: usize = 60_000;

/// Keeps the meshes there are in step with where the camera is: a chunk is meshed at a level
/// when the camera comes within that level's range (nearest first, on the other threads: the
/// frame only takes what they have finished), and its mesh is dropped when the camera is well outside it. A chunk that changed
/// loses its meshes and is made again. So what is meshed and held is what can be seen, and
/// the world can be bigger than that.
fn stream(mut commands: Commands, time: Res<Time>, args: Res<crate::Args>, loading: Res<crate::loading::Loading>, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, mut meshes: ResMut<Assets<Mesh>>, mut drawn: ResMut<Drawn>, mut dirty: ResMut<Dirty>, regions: Res<Regions>, mut cost: ResMut<Cost>, cameras: Query<&GlobalTransform, With<Camera3d>>) {
    let _slow = crate::Slow::new("stream");
    let (Some(terrain), Some(look), Some(eye)) = (terrain, look, cameras.iter().next().map(|t| t.translation())) else { return };
    let began = std::time::Instant::now();
    let changed = !dirty.0.is_empty();
    let mut again = Vec::new();
    for chunk in dirty.0.drain() {
        for level in 0..LEVELS.len() {
            drawn.pending.remove(&(chunk, level));
            if let Some(old) = drawn.made.remove(&(chunk, level)) {
                cost.remade += 1;
                drawn.old.entry((chunk, level)).or_default().extend(old);
                again.push((chunk, level));
            }
        }
    }
    // (A picture waits for the world and may take what it likes of a frame; a scripted strip
    // is the game being played, and takes what play takes.)
    let hurried = loading.up || (args.scene.is_some() && args.strip == 0) || time.elapsed_secs() < 3.0;
    // (Not when the eye has only moved: what is wanted then changes a little at a time, and
    // a picture of someone running would wait for ever.)
    if changed || drawn.looked != terrain.0.chunks.len() {
        drawn.looked = terrain.0.chunks.len();
        // The pass under way may have gone by what changed: the one after it is the first whole one.
        drawn.stale = 2;
    }
    if drawn.at == 0 {
        drawn.keys.clear();
        drawn.keys.extend(terrain.0.chunks.keys().copied());
    }
    // A chunk beside ground that is not made waits for it: its faces at that edge would be
    // drawn open and the mesh thrown away when the ground arrives (four in ten were, while
    // the player ran). So the outermost row of chunks of what is made is not drawn, 8 m of
    // 200. `soon`: the ground it waits for is on its way, and a picture waits too.
    let (per, edge) = (terrain::REGION / CHUNK, terrain.1.span.div_euclid(terrain::REGION));
    let beside = |chunk: IVec3, of: &dyn Fn((i32, i32)) -> bool| (-1..=1).any(|dz| (-1..=1).any(|dx| of(((chunk.x + dx).div_euclid(per), (chunk.z + dz).div_euclid(per)))));
    let open = |chunk: IVec3| beside(chunk, &|r| r.0 >= -edge && r.0 < edge && r.1 >= -edge && r.1 < edge && !regions.made.contains(&r));
    let soon = |chunk: IVec3| beside(chunk, &|r| regions.unmade.contains(&r));
    let upto = if hurried { drawn.keys.len() } else { (drawn.at + drawn.keys.len() / 16 + 1).min(drawn.keys.len()) };
    for i in drawn.at..upto {
        let chunk = drawn.keys[i];
        let away = eye.distance(middle_of(chunk));
        let mut waits = None;
        for (level, &(_, from, to)) in LEVELS.iter().enumerate() {
            let has = drawn.made.contains_key(&(chunk, level));
            if !has && away > from - FADE - KEEP && away < to + KEEP && *waits.get_or_insert_with(|| open(chunk)) {
                drawn.next.1 += soon(chunk) as usize;
            } else if !has && away > from - FADE - KEEP && away < to + KEEP {
                drawn.next.0.push((away, chunk, level));
            } else if has && (away < from - FADE - KEEP * 2.0 || away > to + KEEP * 2.0) {
                for entity in drawn.made.remove(&(chunk, level)).into_iter().flatten() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
    drawn.at = upto;
    if drawn.at >= drawn.keys.len() {
        let (mut wanted, held) = std::mem::take(&mut drawn.next);
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        // What is no longer wanted is given up.
        let asked: HashSet<(IVec3, usize)> = wanted.iter().map(|w| (w.1, w.2)).collect();
        drawn.pending.retain(|key, _| asked.contains(key));
        // (And what was kept of a changed chunk that is no longer wanted at all.)
        if drawn.stale == 0 {
            let Drawn { old, pending, .. } = &mut *drawn;
            old.retain(|key, entities| {
                let keep = asked.contains(key) || pending.contains_key(key);
                if !keep {
                    entities.drain(..).for_each(|entity| commands.entity(entity).despawn());
                }
                keep
            });
        }
        (drawn.wanted, drawn.held, drawn.at) = (wanted, held, 0);
        drawn.stale = drawn.stale.saturating_sub(1);
    } else {
        let Drawn { wanted, made, .. } = &mut *drawn;
        wanted.retain(|w| !made.contains_key(&(w.1, w.2)));
    }
    drawn.waiting = drawn.wanted.len() + drawn.held + regions.waiting + drawn.stale as usize;
    // What the other threads have finished is drawn (a few milliseconds' worth a frame:
    // making a mesh's buffers is not nothing), and the nearest of what is wanted and not
    // begun is begun.
    let slice = if loading.up { crate::loading::SLICE } else if hurried { 0.05 } else { 0.003 };
    let started = std::time::Instant::now();
    let mut ready: Vec<(IVec3, usize)> = drawn.pending.iter().filter(|(_, task)| task.is_finished()).map(|(key, _)| *key).collect();
    ready.sort_by(|a, b| eye.distance_squared(middle_of(a.0)).total_cmp(&eye.distance_squared(middle_of(b.0))));
    let mut taken = 0;
    for key in ready {
        let Some(made) = drawn.pending.get_mut(&key).and_then(bevy::tasks::futures::check_ready) else { continue };
        drawn.pending.remove(&key);
        taken += made.iter().map(|(_, data)| data.positions.len()).sum::<usize>();
        let spawned = draw_level(&mut commands, &mut meshes, &look, &terrain, key.0, key.1, made);
        drawn.made.insert(key, spawned);
        for entity in drawn.old.remove(&key).into_iter().flatten() {
            commands.entity(entity).despawn();
        }
        cost.meshes += 1;
        // (In play, so many corners a frame at most: the card takes each mesh's buffers in
        // the frame it arrives, and two dozen near chunks at once was a frame of 30 ms.)
        if started.elapsed().as_secs_f32() > slice || (!hurried && taken >= UPLOAD) {
            break;
        }
    }
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    let room = AT_ONCE.saturating_sub(drawn.pending.len());
    let begin: Vec<(IVec3, usize)> = drawn.wanted.iter().map(|w| (w.1, w.2)).filter(|key| !drawn.pending.contains_key(key) && !drawn.made.contains_key(key) && terrain.0.chunks.contains_key(&key.0)).take(room).collect();
    // (What has just changed under someone's hand is begun at once, ahead of the pass that
    // would find it.)
    let begin: Vec<(IVec3, usize)> = again.into_iter().filter(|key| terrain.0.chunks.contains_key(&key.0)).chain(begin).collect();
    let top = if begin.is_empty() { 0 } else { terrain.0.top() };
    for (chunk, level) in begin {
        let around = terrain.0.around(chunk, top);
        drawn.pending.insert((chunk, level), pool.spawn(async move { mesh_level(&around, chunk, level) }));
    }
    cost.mesh += began.elapsed().as_secs_f32();
}

/// What the season does to the look of the land, for the wind shader: the tint of leaves and
/// grass, and how much snow lies. Fresh and a little yellow-green in spring, full green in
/// summer, gold and russet through autumn, dull in winter under snow.
pub fn season_look(when: f32) -> Vec4 {
    // The middle of each season, and round again.
    let keys = [Vec3::new(0.94, 1.08, 0.8), Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.5, 0.98, 0.5), Vec3::new(1.05, 0.88, 0.68), Vec3::new(0.94, 1.08, 0.8)];
    let at = (cube_sim::sky::year(when) * 4.0 - 0.5).rem_euclid(4.0);
    let (from, part) = (at as usize, at.fract());
    let tint = keys[from].lerp(keys[from + 1], part * part * (3.0 - 2.0 * part));
    let snow = ((-cube_sim::sky::warmth(when) - 0.35) / 0.65).clamp(0.0, 1.0) * 0.85;
    tint.extend(snow)
}

fn seasons(args: Res<crate::Args>, live: Option<Res<crate::motion::Live>>, look: Option<Res<Look>>, mut windy: ResMut<Assets<Windy>>, mut breeze: ResMut<Breeze>, mut was: Local<(Vec4, f32)>) {
    let (Some(live), Some(look)) = (live, look) else { return };
    let now = season_look(live.0.when);
    breeze.0 = args.wind.unwrap_or_else(|| cube_sim::sky::wind(live.0.when));
    if now.distance(was.0) < 0.004 && (breeze.0 - was.1).abs() < 0.004 {
        return;
    }
    *was = (now, breeze.0);
    for handle in [&look.solid, &look.whole] {
        if let Some(mut material) = windy.get_mut(handle) {
            (material.extension.season, material.extension.wind) = (now, Vec4::splat(breeze.0));
        }
    }
}

/// Makes the cells of the regions the camera has come near, nearest first, each on another
/// thread and apart from the world, which takes it in when it is done (`Voxels::join`); drops those it has left well behind, unless someone dug or
/// built there. So what is held in cells is what is near, and the island can be a kilometre
/// across (or ten) for the price of the ground underfoot.
#[allow(clippy::too_many_arguments)]
fn regions(args: Res<crate::Args>, loading: Res<crate::loading::Loading>, terrain: Option<ResMut<Terrain>>, live: Option<Res<crate::motion::Live>>, mut grown: ResMut<crate::flora::Grown>, mut forms: ResMut<crate::flora::Forms>, mut regions: ResMut<Regions>, mut dirty: ResMut<Dirty>, mut cost: ResMut<Cost>, cameras: Query<&GlobalTransform, With<Camera3d>>) {
    let _slow = crate::Slow::new("regions");
    let (Some(mut terrain), Some(eye)) = (terrain, cameras.iter().next().map(|t| t.translation())) else { return };
    let began = std::time::Instant::now();
    let size = terrain::REGION as f32 * CELL;
    let away = |r: (i32, i32)| Vec2::new((r.0 as f32 + 0.5) * size - eye.x, (r.1 as f32 + 0.5) * size - eye.z).length();
    let edge = terrain.1.span.div_euclid(terrain::REGION);
    let (here, reach) = (region_of((eye / CELL).floor().as_ivec3()), (MAKE / size).ceil() as i32);
    let mut wanted: Vec<(f32, (i32, i32))> = (-reach..=reach)
        .flat_map(|rz| (-reach..=reach).map(move |rx| (here.0 + rx, here.1 + rz)))
        .filter(|r| r.0 >= -edge && r.0 < edge && r.1 >= -edge && r.1 < edge && !regions.made.contains(r))
        .map(|r| (away(r), r))
        .filter(|w| w.0 < MAKE)
        .collect();
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
    regions.waiting = wanted.len();
    regions.unmade = wanted.iter().map(|w| w.1).collect();
    // Every chunk of a region and the ring round it is drawn again: faces at its edge that
    // were open to nothing now have a neighbour, and the other way about.
    let redraw = |voxels: &Voxels, r: (i32, i32), dirty: &mut Dirty| {
        let per = terrain::REGION / CHUNK;
        let (lo, hi) = ((r.0 * per - 1, r.1 * per - 1), ((r.0 + 1) * per, (r.1 + 1) * per));
        dirty.0.extend(voxels.chunks.keys().filter(|c| c.x >= lo.0 && c.x <= hi.0 && c.z >= lo.1 && c.z <= hi.1));
    };
    // What the other threads have finished is taken in, and what grew there kept for drawing.
    let asked: HashSet<(i32, i32)> = wanted.iter().map(|w| w.1).collect();
    regions.pending.retain(|r, _| asked.contains(r));
    let ready: Vec<(i32, i32)> = regions.pending.iter().filter(|(_, task)| task.is_finished()).map(|(r, _)| *r).collect();
    for r in ready {
        let Some((part, mut form)) = regions.pending.get_mut(&r).and_then(bevy::tasks::futures::check_ready) else { continue };
        regions.pending.remove(&r);
        let Terrain(voxels, land, ..) = &mut *terrain;
        let (lo, hi) = ((r.0 * terrain::REGION, r.1 * terrain::REGION), ((r.0 + 1) * terrain::REGION, (r.1 + 1) * terrain::REGION));
        voxels.join(part, |c| region_of(c * CHUNK) == r);
        // Of the shapes grown, the ones whose middle is in this region are this region's to
        // draw: a tree astride an edge is grown by both sides, and drawn half by each.
        let within = |p: Vec3| (p.x / CELL) as i32 >= lo.0 && ((p.x / CELL) as i32) < hi.0 && (p.z / CELL) as i32 >= lo.1 && ((p.z / CELL) as i32) < hi.1;
        form.limbs.retain(|l| within((l.a + l.b) / 2.0));
        form.clumps.retain(|c| within(c.middle));
        form.stones.retain(|s| within(s.middle));
        // `--specimen`: the ground round the start is cleared of what grew and one thing stood there.
        let start = Vec3::new(args.at.0, 0.0, args.at.1);
        if let Some(name) = &args.specimen {
            let clear = |p: Vec3| p.with_y(0.0).distance(start) > 18.0;
            form.limbs.retain(|l| clear(l.a));
            form.clumps.retain(|c| clear(c.middle));
            form.stones.retain(|s| clear(s.middle));
            if within(start) {
                let foot = IVec3::new((start.x / CELL) as i32, land.height((start.x / CELL) as i32, (start.z / CELL) as i32), (start.z / CELL) as i32);
                let (found, grown) = cube_sim::plants::recorded(|| cube_sim::plants::specimen(voxels, foot, name, &mut cube_sim::rng::Rng::new(args.seed as u64 + 7)));
                if !found {
                    eprintln!("no specimen `{name}`: oak birch pine willow snag young sapling bush log stump rock");
                }
                // (Its own thing among the region's: `plants` counts each growing from one.)
                form.limbs.extend(grown.limbs.into_iter().map(|l| cube_sim::plants::Limb { tree: l.tree + 1_000_000, ..l }));
                form.clumps.extend(grown.clumps.into_iter().map(|c| cube_sim::plants::Clump { tree: c.tree + 1_000_000, ..c }));
                form.stones.extend(grown.stones);
            }
        }
        forms.0.insert(r, std::sync::Arc::new(form));
        // (A changed region made again is the land as the seed has it, until what was done
        // to it is done again, below.)
        if terrain.3.contains(&r) {
            regions.stale.insert(r);
            if std::env::var_os("LOOSE_TRACE").is_some() {
                println!("regions: changed region {r:?} made again from the seed, its edits to be done again");
            }
        }
        regions.made.insert(r);
        regions.waiting -= 1;
        regions.unmade.remove(&r);
        cost.fills += 1;
        redraw(&terrain.0, r, &mut dirty);
    }
    // The nearest of what is wanted and not begun is begun: a few at once, so the threads
    // are left free for the meshes of what is already here.
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    for (_, r) in wanted {
        if regions.pending.len() >= if loading.up || (args.scene.is_some() && args.strip == 0) { 12 } else { 4 } {
            break;
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = regions.pending.entry(r) {
            let land = terrain.1.clone();
            let (lo, hi) = ((r.0 * terrain::REGION, r.1 * terrain::REGION), ((r.0 + 1) * terrain::REGION, (r.1 + 1) * terrain::REGION));
            // (How much grazing is left at its middle: ground eaten down is made with less
            // growing on it, `terrain::graze`. A region is half a pasture across.)
            // (`GRAZED=0.3` makes all of it as if grazed to that, to be looked at.)
            let left = std::env::var("GRAZED").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| live.as_ref().map_or(1.0, |live| live.0.pasture(Vec2::new((lo.0 + hi.0) as f32, (lo.1 + hi.1) as f32) * cube_voxel::CELL / 2.0)));
            slot.insert(pool.spawn(async move {
                let mut part = Voxels::default();
                let ((), form) = cube_sim::plants::recorded(|| terrain::fill(&mut part, &land, lo, hi));
                terrain::graze(&mut part, &land, lo, hi, left);
                (part, form)
            }));
        }
    }
    // A region that has been changed is its seed and what was done to it (`loose::replay`),
    // so it is dropped like any other, with the changed regions it touches: all of them
    // together, when all are far enough, since what fell in one may lie in the next. Made
    // again, they are the land as the seed has it until every one of them and the regions
    // round them are there; then what was done is done again, in the order it was done.
    let groups = {
        let mut left: Vec<(i32, i32)> = terrain.3.iter().copied().collect();
        left.sort_unstable();
        let mut groups: Vec<Vec<(i32, i32)>> = Vec::new();
        while let Some(first) = left.pop() {
            let mut group = vec![first];
            let mut i = 0;
            while i < group.len() {
                let at = group[i];
                let (near, far): (Vec<_>, Vec<_>) = left.iter().partition(|r| (r.0 - at.0).abs() <= 1 && (r.1 - at.1).abs() <= 1);
                group.extend(near);
                left = far;
                i += 1;
            }
            groups.push(group);
        }
        groups
    };
    if let Some(live) = &live {
        for group in &groups {
            if !group.iter().all(|r| regions.stale.contains(r)) {
                continue;
            }
            let round = |r: &(i32, i32)| { let r = *r; (-1..=1).flat_map(move |z| (-1..=1).map(move |x| (r.0 + x, r.1 + z))) };
            if !group.iter().flat_map(round).all(|r| regions.made.contains(&r) || r.0 < -edge || r.0 >= edge || r.1 < -edge || r.1 >= edge) {
                continue;
            }
            let mine: Vec<(u64, cube_sim::loose::Edit)> = live.0.loose.journal.iter().copied().filter(|(_, e)| {
                group.contains(&region_of((e.at() / CELL).floor().as_ivec3()))
            }).collect();
            let near: Vec<(i32, i32)> = group.iter().flat_map(round).collect::<HashSet<_>>().into_iter().filter(|r| forms.0.contains_key(r)).collect();
            // A few milliseconds of it a frame (all of it behind the loading screen): a wood
            // felled again at once was a frame of 50 ms and more on coming back to it.
            let key = group.iter().copied().min().unwrap_or_default();
            let mut doing = regions.replaying.remove(&key).unwrap_or_default();
            let done = {
                let shapes: Vec<&cube_sim::plants::Form> = near.iter().map(|r| &*forms.0[r]).collect();
                doing.advance(&mut terrain.0, &shapes, &mine, if loading.up { std::time::Duration::MAX } else { std::time::Duration::from_millis(3) })
            };
            if !done {
                regions.replaying.insert(key, doing);
                continue;
            }
            let again = doing.loose;
            // What is cells again is no longer among the shapes drawn.
            for r in &near {
                let Some(form) = forms.0.get_mut(r).map(std::sync::Arc::make_mut) else { continue };
                let cut: HashSet<u32> = form.limbs.iter().filter(|l| again.things.iter().filter(|t| t.felled || t.cells || t.notch.is_some()).any(|t| t.limbs.iter().any(|m| m.a == l.a && m.b == l.b))).map(|l| l.tree).collect();
                let before = (form.limbs.len(), form.stones.len());
                form.limbs.retain(|l| !cut.contains(&l.tree));
                form.clumps.retain(|c| !cut.contains(&c.tree));
                form.stones.retain(|s| !again.things.iter().any(|t| t.stone.as_ref().is_some_and(|m| m.middle == s.middle)));
                if before != (form.limbs.len(), form.stones.len()) {
                    grown.again(*r);
                }
            }
            for r in group {
                regions.stale.remove(r);
                redraw(&terrain.0, *r, &mut dirty);
            }
            // (A script's `forget`: held against the cells as they were. Leaf of a tree that
            // stands in the next region is counted apart: a region made again alone does not
            // get back what overhangs it from its neighbours, changed or not, which is the
            // streaming's own seam and none of the journal's.)
            if let Some(edits) = regions.held.take() {
                let (mut differ, mut leaf, mut first) = (0, 0, String::new());
                let mut kinds: std::collections::BTreeMap<String, (usize, IVec3, IVec3)> = Default::default();
                for (place, chunk) in std::mem::take(&mut regions.was) {
                    for i in 0..CHUNK * CHUNK * CHUNK {
                        let l = IVec3::new(i % CHUNK, i / (CHUNK * CHUNK), i / CHUNK % CHUNK);
                        let (a, b) = (chunk.get(l), terrain.0.get(place * CHUNK + l));
                        let plain = |x: Block| if x.sprig() { Block::Air } else { x };
                        if plain(a) != plain(b) || chunk.bits(l).map(|x| x.to_vec()) != terrain.0.bits_at(place * CHUNK + l).map(|x| x.to_vec()) {
                            if a.shaped() && a.foliage() && plain(b) == Block::Air {
                                leaf += 1;
                            } else {
                                differ += 1;
                                let at = place * CHUNK + l;
                                let k = kinds.entry(format!("{a:?}>{b:?}")).or_insert((0, at, at));
                                (k.0, k.1, k.2) = (k.0 + 1, k.1.min(at), k.2.max(at));
                                if first.is_empty() {
                                    first = format!(" (first at {:?}: {a:?} was, {b:?} is)", place * CHUNK + l);
                                }
                            }
                        }
                    }
                }
                if std::env::var_os("LOOSE_TRACE").is_some() {
                    println!("forget kinds: {kinds:?}");
                }
                println!("{} forget: {} changed regions dropped and made again from the seed and {} of {edits} edits · {differ} cells differ{first} · {leaf} cells of a neighbour's overhanging leaf not come back", if differ == 0 { "ok" } else { "FAIL" }, group.len(), mine.len());
            }
        }
    }
    // (Held to how it is once whatever is falling has come to rest: taken as a tree was
    // still coming down, what it was held to had the tree in the air.)
    let falling = live.as_ref().is_some_and(|l| !l.0.loose.pieces.is_empty());
    if regions.forget && !falling && !terrain.3.is_empty() {
        regions.forget = false;
        regions.held = Some(live.as_ref().map_or(0, |l| l.0.loose.journal.len()));
        regions.was = terrain.0.chunks.iter().filter(|(c, _)| terrain.3.contains(&region_of(**c * CHUNK))).map(|(c, k)| (*c, k.clone())).collect();
    }
    let forgetting = regions.held.is_some() && regions.stale.is_empty();
    let far = |r: &(i32, i32)| groups.iter().find(|g| g.contains(r)).is_none_or(|g| g.iter().all(|r| forgetting || away(*r) > DROP));
    // (Nothing is dropped while anything is falling there: a fall is finished first.)
    let gone: Vec<(i32, i32)> = regions.made.iter().copied().filter(|r| if terrain.3.contains(r) { !falling && !regions.stale.contains(r) && far(r) } else { away(*r) > DROP }).collect();
    for r in gone {
        if terrain.3.contains(&r) && std::env::var_os("LOOSE_TRACE").is_some() {
            println!("regions: changed region {r:?} dropped");
        }
        redraw(&terrain.0, r, &mut dirty);
        terrain.0.chunks.retain(|c, _| (c.x.div_euclid(terrain::REGION / CHUNK), c.z.div_euclid(terrain::REGION / CHUNK)) != r);
        regions.made.remove(&r);
        forms.0.remove(&r);
    }
    regions.kept = terrain.3.clone();
    cost.fill += began.elapsed().as_secs_f32();
}
