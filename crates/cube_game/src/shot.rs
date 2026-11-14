//! Contact sheets: `--shot ID` lays the creature out as a grid, a row a clip and a column a
//! moment in it, each held still, and writes one picture of it to `shots/ID.png`. One frame
//! shows every clip, so checking a creature by eye costs one image.
//!
//! `--view quarter|side|front` turns the creatures (the camera is level, so feet read against
//! the slab under each row); `--cols N` moments a clip; `--clips a,b` only those rows.

use crate::{
    Args,
    creatures::{self, Creature, Meta, Registry, Rig},
    world,
};
use bevy::{
    camera::{RenderTarget, ScalingMode},
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, save_to_disk},
    },
};

pub struct ShotPlugin;

impl Plugin for ShotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.62, 0.66, 0.72))).init_resource::<Stage>().add_systems(Startup, setup).add_systems(Update, (layout, capture, ride).chain());
    }
}

#[derive(Resource, Default)]
pub struct Stage {
    /// Seconds since the scene was ready to be pictured (every model in, every mesh made):
    /// the clock a picture's `--after` counts on, and a script's times too (`player::script`).
    /// Counted from the start of the program, a script was over before the world had been
    /// made, and every strip and moving picture was of someone standing still.
    pub ready_for: f32,
    /// For a strip: how long the world took to be ready (real seconds) and what that cost,
    /// then every frame's time while the strip ran.
    pub load: Option<(f32, world::Cost)>,
    pub ran: Vec<f32>,
    /// The camera's height each of those frames.
    pub rode: Vec<f32>,
    /// How far the foot that stood lowest was above the ground drawn under it, each frame.
    pub stood: Vec<f32>,
    /// How far under the drawn ground its deepest knee or ankle was, each frame.
    pub under: Vec<f32>,
    /// The trunk of what is played, each frame it stood on the ground: when, how high the
    /// bone it hangs from is, how far it is tipped nose up (radians), and how high the model
    /// as a whole was put.
    pub trunk: Vec<(f32, f32, f32, f32)>,
    laid_out: bool,
    expected: usize,
    settled: u32,
    asked: bool,
    path: String,
    target: Handle<Image>,
    legend: String,
    taken: usize,
    frames: Vec<image::RgbImage>,
}

fn rows(id: &str, args: &Args) -> Vec<String> {
    let meta: Meta = std::fs::read_to_string(format!("assets/creatures/{id}/meta.toml")).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let mut names: Vec<String> = meta.clips.keys().cloned().collect();
    names.sort_by_key(|n| creatures::clip_rank(n));
    if !args.clips.is_empty() {
        names.retain(|n| args.clips.contains(n));
    }
    names
}

/// The picture that fits the sheet: square cells, at most about 1000 pixels tall.
fn sheet_size(id: &str, args: &Args) -> (u32, u32) {
    let rows = rows(id, args).len().max(1) as u32;
    let cell = (1000 / rows).min(240);
    (args.cols as u32 * cell, rows * cell)
}

fn setup(mut commands: Commands) {
    // No shadows: on a sheet one row's would fall across the next.
    world::spawn_lights(&mut commands, false);
}

fn layout(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    mut stage: ResMut<Stage>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if stage.laid_out {
        return;
    }
    stage.laid_out = true;
    let id = args.shot.clone().unwrap();
    let Some(def) = registry.index(&id).map(|i| &registry.0[i]) else {
        eprintln!("no creature `{id}` with a model.glb under assets/creatures");
        std::process::exit(2);
    };
    let names = rows(&id, &args);
    let (cols, n) = (args.cols, names.len());
    let cell = def.meta.height.max(def.meta.length) * 1.3;
    // (`top`: from above, its head up the picture: what lies flat on the ground, a serpent or
    // a crab, is a line from any side.)
    let (turn, lift) = match args.view.as_str() {
        "front" => (Quat::IDENTITY, 0.0),
        "side" => (Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 0.0),
        "top" => (Quat::from_rotation_z(std::f32::consts::PI) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), cell * 0.42),
        _ => (Quat::from_rotation_y(0.65), 0.0),
    };
    let slab = meshes.add(Cuboid::new(cols as f32 * cell, cell * 0.02, cell * 0.7));
    let slab_material = materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.27, 0.3), perceptual_roughness: 1.0, ..default() });
    let middle = Vec3::new(0.0, -((n - 1) as f32) * cell / 2.0 + cell * 0.42, 0.0);
    let (width, height) = sheet_size(&id, &args);
    stage.target = images.add(Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None));
    let camera = commands
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(stage.target.clone().into()),
            Projection::Orthographic(OrthographicProjection { scaling_mode: ScalingMode::AutoMin { min_width: cols as f32 * cell, min_height: n as f32 * cell }, ..OrthographicProjection::default_3d() }),
            Transform::from_translation(middle + Vec3::Z * 50.0).looking_at(middle, Vec3::Y),
        ))
        .id();
    for (row, name) in names.iter().enumerate() {
        let y = -(row as f32) * cell;
        commands.spawn((Mesh3d(slab.clone()), MeshMaterial3d(slab_material.clone()), Transform::from_xyz(0.0, y - cell * 0.01, 0.0)));
        let info = &def.meta.clips[name];
        for col in 0..cols {
            let part = if info.looping { col as f32 / cols as f32 } else { col as f32 / (cols - 1).max(1) as f32 };
            let x = (col as f32 - (cols - 1) as f32 / 2.0) * cell;
            let mut creature = Creature::new(registry.index(&id).unwrap());
            creature.hold = Some(info.seconds * part * 0.999);
            creature.clip_name = Some(name.clone());
            commands.spawn((creature, Transform::from_xyz(x, y + lift, 0.0).with_rotation(turn), Visibility::default()));
        }
        commands.spawn((
            Text::new(format!("{name} {:.1}s {}", info.seconds, info.source)),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::BLACK),
            Node { position_type: PositionType::Absolute, left: px(6), top: percent(100.0 * row as f32 / n as f32 + 0.5), ..default() },
            UiTargetCamera(camera),
        ));
    }
    stage.expected = n * cols;
    stage.path = if args.view == "quarter" { format!("shots/{id}.png") } else { format!("shots/{id}-{}.png", args.view) };
    stage.legend = format!("rows={} cols={cols} view={}", names.join(","), args.view);
}

/// What the picture cost to draw, said with every shot so a change that slows the game is
/// seen the day it is made: the middle and the worst frame of the last sixty, on what drew
/// them (a software renderer here is many times slower than a graphics card: compare a number
/// only with the same machine's), and the terrain faces in view, which is the same anywhere.
#[derive(Default)]
struct Pace(Vec<f32>);

impl Pace {
    fn say(&self, faces: &Query<(&world::Faces, &ViewVisibility)>, adapter: &Option<Res<bevy::render::renderer::RenderAdapterInfo>>) -> String {
        let mut frames = self.0.clone();
        frames.sort_by(f32::total_cmp);
        let in_view: u32 = faces.iter().filter(|(_, seen)| seen.get()).map(|(f, _)| f.0).sum();
        let on = adapter.as_ref().map_or("?".into(), |a| a.name.clone());
        format!("pace frame={:.0}ms worst={:.0}ms faces-in-view={:.2}M on={on}", frames.get(frames.len() / 2).unwrap_or(&0.0) * 1000.0, frames.last().unwrap_or(&0.0) * 1000.0, in_view as f32 / 1e6)
    }
}

#[allow(clippy::too_many_arguments)]
fn capture(mut commands: Commands, mut stage: ResMut<Stage>, args: Res<Args>, time: Res<Time>, real: Res<Time<Real>>, rigs: Query<&Rig>, creatures: Query<(), With<Creature>>, drawn: Option<Res<world::Drawn>>, faces: Query<(&world::Faces, &ViewVisibility)>, adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>, mut exit: MessageWriter<AppExit>, cost: Option<Res<world::Cost>>, mut waited: Local<f32>, mut pace: Local<Pace>, mut was: Local<world::Cost>) {
    // (Once a strip has begun every frame is counted, the ones the world was being made in
    // most of all: those are the slow ones.)
    if stage.load.is_some() {
        stage.ran.push(real.delta_secs());
        // `SLOW_TRACE=1`: every frame over a fiftieth of a second, and what was made in it.
        if let Some(cost) = &cost {
            if real.delta_secs() > 0.02 && std::env::var_os("SLOW_TRACE").is_some() {
                println!("slow {:.0}ms · {}", real.delta_secs() * 1000.0, cost.since(&was));
            }
            *was = **cost;
        }
    }
    // Every creature there is going to be has its model (the ones asked for, and any the world
    // came with), and every chunk in view has its mesh.
    // Once it has all been there the clock runs whatever is being made (someone walking has
    // always a little ground on its way, a frame or two from the other threads); only the
    // moment of a single picture waits again for everything.
    let whole = stage.laid_out && rigs.iter().count() >= stage.expected.max(creatures.iter().count()) && !drawn.is_some_and(|d| d.waiting > 0);
    if !stage.laid_out || (stage.settled == 0 && !whole) {
        return;
    }
    stage.settled += 1;
    *waited += time.delta_secs();
    stage.ready_for = *waited;
    if !stage.asked {
        pace.0.push(real.delta_secs());
        if pace.0.len() > 60 {
            pace.0.remove(0);
        }
    }
    if args.strip > 0 {
        if stage.load.is_none() {
            stage.load = Some((real.elapsed_secs(), cost.as_deref().copied().unwrap_or_default()));
        }
        // A strip: a picture every `every` seconds; `gather` saves them as one when all are in.
        if stage.settled >= 40 && *waited >= args.after + stage.taken as f32 * args.every && stage.taken < args.strip {
            stage.taken += 1;
            commands.spawn(Screenshot::image(stage.target.clone())).observe(gather);
        }
        if stage.settled > 20000 {
            eprintln!("the strip never finished");
            exit.write(AppExit::error());
        }
        return;
    }
    if stage.settled >= 40 && *waited >= args.after && !stage.asked && (whole || *waited >= args.after + 2.0) {
        stage.settled = 40;
        stage.asked = true;
        if let Some(dir) = std::path::Path::new(&stage.path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::remove_file(&stage.path);
        commands.spawn(Screenshot::image(stage.target.clone())).observe(save_to_disk(stage.path.clone()));
    }
    if stage.asked && std::path::Path::new(&stage.path).exists() && stage.settled > 50 {
        println!("shot {} {} · {}", stage.path, stage.legend, pace.say(&faces, &adapter));
        exit.write(AppExit::Success);
    }
    if stage.asked && stage.settled > 600 {
        eprintln!("the screenshot never arrived");
        exit.write(AppExit::error());
    }
}

/// While a strip runs: how the camera rode and how the feet stood, each frame, for `gather` to say.
fn ride(time: Res<Time>, mut stage: ResMut<Stage>, eyes: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<world::Terrain>>, walkers: Query<(&crate::feet::Legs, &Transform, &crate::motion::Mover), With<crate::player::Controlled>>, live: Option<Res<crate::motion::Live>>, bones: Query<&GlobalTransform>) {
    if stage.load.is_none() {
        return;
    }
    if let Some(camera) = eyes.iter().next() {
        stage.rode.push(camera.translation().y);
    }
    // (Only while it stands on the ground, and not at the lip of a drop, where the ground
    // under a foot is another level altogether.)
    if let (Some(terrain), Some((legs, at, _))) = (&terrain, walkers.iter().next().filter(|(.., m)| live.as_ref().and_then(|l| l.agent(m)).is_some_and(|a| a.body.grounded && !a.body.swimming))) {
        let gaps = legs.legs.iter().filter_map(|[.., tip]| bones.get(*tip).ok()).filter_map(|g| crate::feet::ground_at(terrain, g.translation().x, g.translation().z, at.translation.y).map(|ground| g.translation().y - ground));
        if let Some(lowest) = gaps.reduce(f32::min).filter(|g| g.abs() < 0.45) {
            stage.stood.push(lowest);
        }
        // And how far under the ground that is drawn any knee or ankle of it is: a leg in the ground.
        let under = legs.legs.iter().flat_map(|[_, knee, tip]| [knee, tip]).filter_map(|b| bones.get(*b).ok()).filter_map(|g| crate::feet::ground_at(terrain, g.translation().x, g.translation().z, at.translation.y).map(|ground| ground - g.translation().y)).fold(f32::MIN, f32::max);
        if under > f32::MIN {
            stage.under.push(under);
        }
        if let Some(root) = legs.root.and_then(|r| bones.get(r).ok()) {
            let ahead = at.rotation * Vec3::Z;
            stage.trunk.push((time.elapsed_secs(), root.translation().y, (root.rotation() * (at.rotation.inverse() * ahead)).y.clamp(-1.0, 1.0).asin(), at.translation.y));
        }
    }
}

/// One picture of a strip arriving. When the last is in they are set side by side and saved.
fn gather(shot: On<bevy::render::view::screenshot::ScreenshotCaptured>, args: Res<Args>, cost: Option<Res<world::Cost>>, mut stage: ResMut<Stage>, mut exit: MessageWriter<AppExit>) {
    let Ok(picture) = shot.image.clone().try_into_dynamic() else { return };
    stage.frames.push(picture.to_rgb8());
    if stage.frames.len() < args.strip {
        return;
    }
    // What the run cost: the load, then the frames while it ran (the middle one, the one in a
    // hundred, the worst, and how many took over a thirtieth of a second) and what was made
    // meanwhile. `make pace` holds these to bars.
    if let (Some((took, at_load)), Some(cost)) = (stage.load, cost) {
        let mut ran = stage.ran.clone();
        // (When the slowest frame was, seconds into the run: against what was happening then.)
        let slowest = ran.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0.0, |(n, _)| ran[..n].iter().sum::<f32>());
        ran.sort_by(f32::total_cmp);
        let at = |part: f32| ran.get(((ran.len() as f32 * part) as usize).min(ran.len().saturating_sub(1))).copied().unwrap_or(0.0) * 1000.0;
        println!("load {took:.1}s · {}", at_load.since(&world::Cost::default()));
        // How the camera rode: the most it rose or fell in one frame, and how unevenly (the
        // middle change, frame to frame, of how fast its height was changing). A camera that
        // takes each block of the ground as a step is rough; one that glides is not.
        let moved: Vec<f32> = stage.rode.windows(2).map(|w| w[1] - w[0]).collect();
        let mut jolts: Vec<f32> = moved.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        jolts.sort_by(f32::total_cmp);
        println!("camera worst-step={:.1}cm rough={:.2}cm p99={:.1}cm", moved.iter().fold(0.0f32, |m, v| m.max(v.abs())) * 100.0, jolts.get(jolts.len() / 2).copied().unwrap_or(0.0) * 100.0, jolts.get(jolts.len() * 99 / 100).copied().unwrap_or(0.0) * 100.0);
        // How its feet stood: the foot that is lowest is the one on the ground, and it
        // should be the same little way above the ground drawn under it whatever the
        // slope. The middle of that, and how far it strays either way.
        let mut stood = stage.stood.clone();
        stood.sort_by(f32::total_cmp);
        if let (Some(low), Some(mid), Some(high)) = (stood.get(stood.len() / 20), stood.get(stood.len() / 2), stood.get(stood.len() * 19 / 20)) {
            println!("feet stance-gap mid={:.1}cm from={:.1}cm to={:.1}cm spread={:.1}cm", mid * 100.0, low * 100.0, high * 100.0, (high - low) * 100.0);
        }
        // How its trunk rode: how fast the model as a whole was put up or down at the most
        // (before its clip and its feet), and the bone its body hangs from: how fast that
        // rose or fell, how much that changed from one frame to the next (a jolt), and how
        // fast it tipped. By the second, since a frame a picture is taken in is a long one.
        // A trunk that takes each block of a hillside as a step or a nod is what "not smooth
        // on hills" looks like; on the level a walk is under 1 m/s and a jolt under 0.5.
        if stage.trunk.len() > 3 {
            let rate = |of: &dyn Fn(&(f32, f32, f32, f32)) -> f32| -> Vec<f32> { stage.trunk.windows(2).filter(|w| w[1].0 > w[0].0).map(|w| (of(&w[1]) - of(&w[0])) / (w[1].0 - w[0].0)).collect() };
            let (model, trunk, nods) = (rate(&|t| t.3), rate(&|t| t.1), rate(&|t| t.2.to_degrees()));
            let jolts: Vec<f32> = trunk.windows(2).map(|w| w[1] - w[0]).collect();
            let part = |of: &[f32], p: usize| {
                let mut of: Vec<f32> = of.iter().map(|v| v.abs()).collect();
                of.sort_by(f32::total_cmp);
                of.get((of.len() * p / 100).min(of.len().saturating_sub(1))).copied().unwrap_or(0.0)
            };
            println!("trunk model={:.1}m/s rise-p99={:.1}m/s rise={:.1}m/s jolt-p99={:.1}m/s jolt={:.1}m/s nod-p99={:.0}deg/s nod={:.0}deg/s", part(&model, 100), part(&trunk, 99), part(&trunk, 100), part(&jolts, 99), part(&jolts, 100), part(&nods, 99), part(&nods, 100));
        }
        if !stage.under.is_empty() {
            let (worst, frames) = (stage.under.iter().copied().fold(f32::MIN, f32::max), stage.under.iter().filter(|u| **u > 0.0).count());
            // (A knee or an ankle is the middle of a limb some 6 cm thick: nearer the ground than that, the limb is in it.)
            let near = stage.under.iter().filter(|u| **u > -0.06).count();
            println!("legs under-ground worst={:.1}cm frames={:.0}% touching={:.0}% clear={:.1}cm", worst.max(0.0) * 100.0, frames as f32 * 100.0 / stage.under.len() as f32, near as f32 * 100.0 / stage.under.len() as f32, -worst * 100.0);
        }
        println!("frames n={} p50={:.1}ms p99={:.1}ms worst={:.0}ms at {slowest:.1}s over33={} in {:.0}s · {}", ran.len(), at(0.5), at(0.99), at(1.0), ran.iter().filter(|f| **f > 1.0 / 30.0).count(), ran.iter().sum::<f32>(), cost.since(&at_load));
    }
    if let Some(dir) = std::path::Path::new(&stage.path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if args.gif {
        // A moving picture: the frames in turn, each shown for as long as they were apart.
        let saved = std::fs::File::create(&stage.path).map_err(|e| e.to_string()).and_then(|file| {
            let mut gif = image::codecs::gif::GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
            gif.set_repeat(image::codecs::gif::Repeat::Infinite).map_err(|e| e.to_string())?;
            let delay = image::Delay::from_saturating_duration(std::time::Duration::from_secs_f32(args.every));
            gif.encode_frames(stage.frames.iter().map(|f| image::Frame::from_parts(image::DynamicImage::ImageRgb8(f.clone()).to_rgba8(), 0, 0, delay))).map_err(|e| e.to_string())
        });
        match saved {
            Ok(()) => println!("gif {} {} frames={} seconds={:.1}", stage.path, stage.legend, args.strip, args.strip as f32 * args.every),
            Err(e) => eprintln!("saving {}: {e}", stage.path),
        }
        exit.write(AppExit::Success);
        return;
    }
    let (w, h) = stage.frames[0].dimensions();
    let mut strip = image::RgbImage::new(w * args.strip as u32, h);
    for (i, frame) in stage.frames.iter().enumerate() {
        image::imageops::replace(&mut strip, frame, (i as u32 * w) as i64, 0);
    }
    if let Some(dir) = std::path::Path::new(&stage.path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match strip.save(&stage.path) {
        Ok(()) => println!("shot {} {} strip={} every={}s from={}s", stage.path, stage.legend, args.strip, args.every, args.after),
        Err(e) => eprintln!("saving {}: {e}", stage.path),
    }
    exit.write(AppExit::Success);
}

/// `--scene FILE`: the test world with the `--spawn` creatures (`all` for one of each) standing
/// in it, every other one walking, drawn offscreen to one picture. How an agent sees the world.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stage>().add_systems(Update, (scene, capture, ride).chain());
    }
}

fn scene(mut commands: Commands, args: Res<Args>, registry: Res<Registry>, terrain: Option<Res<world::Terrain>>, mut stage: ResMut<Stage>, mut images: ResMut<Assets<Image>>) {
    let Some(terrain) = terrain else { return };
    if stage.laid_out {
        return;
    }
    stage.laid_out = true;
    let ids = &args.spawn;
    for (n, id) in ids.iter().enumerate() {
        let Some(def) = registry.index(id) else {
            eprintln!("no creature `{id}`");
            continue;
        };
        let mut creature = Creature::new(def);
        if n % 2 == 1 && !args.play {
            creature.clip_name = Some("walk".into());
        }
        commands.spawn((creature, creatures::spot(n, &terrain, args.at), Visibility::default()));
        stage.expected += 1;
    }
    let (w, h) = if args.gif && args.big {
        // (A place, not a creature: big enough to see leaves move.)
        (720, 405)
    } else if args.gif {
        (360, 240)
    } else if args.strip > 0 {
        (1920 / args.strip.clamp(4, 12) as u32, 300)
    } else {
        (1600, 900)
    };
    stage.target = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    let mut focus = Vec3::new(args.at.0, terrain.ground(args.at.0, args.at.1) + 0.8, args.at.1);
    // One creature alone is a portrait: the camera looks at it, not at the start.
    if let (false, [id]) = (args.play, &ids[..])
        && let Some(def) = registry.index(id)
    {
        focus = creatures::spot(0, &terrain, args.at).translation + Vec3::Y * registry.0[def].meta.height * 0.6;
    }
    // Without `--play` the camera stands off the start: `--dist` metres away, `--pitch` up,
    // round to `--yaw`. `--view high` is the lie of the land from the air.
    let high = args.view == "high";
    let dist = args.dist.unwrap_or(if high { 150.0 } else { 8.7 * (1.0 + ids.len() as f32 * 0.07) });
    let pitch = args.pitch.unwrap_or(if high { 0.6 } else { 0.3 });
    let yaw = if args.play || args.yaw_given { args.yaw } else { 0.57 };
    // A specimen is looked at part way up it, from however far off.
    if args.specimen.is_some() {
        focus.y += dist * 0.25 - 0.8;
    }
    let from = focus + Quat::from_euler(EulerRot::YXZ, yaw, -pitch, 0.0) * Vec3::Z * dist;
    // Never from under a hillside.
    let from = from.with_y(from.y.max(terrain.ground(from.x, from.z) + 1.6));
    // `--view moon`: from over the trees, looking at where the moon is at that day and hour,
    // close in: its phase, to be seen.
    let when = cube_sim::sky::at(args.day.unwrap_or(3), args.hour.unwrap_or(15.0));
    let (from, focus, lens) = if args.view == "moon" { (focus + Vec3::Y * 40.0, focus + Vec3::Y * 40.0 + cube_sim::sky::moon(when) * 100.0, 0.12) } else { (from, focus, std::f32::consts::FRAC_PI_4) };
    // `--view sky`: from over the trees too, looking out toward `--yaw` (0 north, 1.57 west)
    // and a little up (`--pitch`, radians above level), wide: the land's edge low in the
    // picture and the sky over it, to judge the sky and the light by (`make sky`).
    let (from, focus, lens) = if args.view == "sky" {
        let out = Quat::from_euler(EulerRot::YXZ, args.yaw, args.pitch.unwrap_or(0.16), 0.0) * Vec3::NEG_Z;
        (focus + Vec3::Y * 40.0, focus + Vec3::Y * 40.0 + out * 100.0, 1.05)
    } else {
        (from, focus, lens)
    };
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: lens, far: 4000.0, ..default() }),
        RenderTarget::Image(stage.target.clone().into()),
        // Only `--play` uses the orbit: behind the player, facing the way the route runs.
        crate::camera::Orbit { focus, yaw: args.yaw, pitch: 0.28, distance: args.dist.unwrap_or(5.5) },
        Transform::from_translation(from).looking_at(focus, Vec3::Y),
        world::look(),
    ));
    stage.path = args.scene.clone().unwrap();
    stage.legend = format!("creatures={}", ids.join(","));
}
