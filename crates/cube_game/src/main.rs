//! The game so far: an island made from a seed, with its creatures living in it (`--wild`), to
//! walk about as the player (`--play`); and the sandbox panel to spawn any creature and watch
//! its clips.
//!
//! `cargo sandbox --help` lists the ways to start it (`HELP` below).
//!
//! Map: `world` (terrain from cube_sim, drawn) · `creatures` (registry, spawning, clips, wander) ·
//! `motion` (the simulation's creatures, drawn between ticks with the clip that fits) · `player` (controlling
//! one with the keys, the follow and first-person cameras) · `ui` (the egui panel, skeleton
//! overlay) · `camera` (free orbit) · `shot` (contact sheets and scene pictures) · `loading`
//! (the screen the window opens on while the world near the player is made).

mod air;
mod audio;
mod camera;
mod cave;
mod clouds;
mod creatures;
mod far;
mod feet;
mod held;
mod flora;
mod hud;
mod limbs;
mod loading;
mod loose;
mod motion;
mod player;
mod shot;
mod stride;
mod ui;
mod vault;
mod world;

use bevy::prelude::*;

/// `SLOW_TRACE=1`: a system that took more than 8 ms says so as it ends (`let _slow =
/// Slow::new("name")` at its head). With `shot`'s `slow` lines, which say how long the frame
/// was and what was made in it, this says whose the time was: three guesses at a 54 ms frame
/// (the day's turn, a region taken in, a first jump) were wrong before this was asked.
pub struct Slow(&'static str, std::time::Instant);

impl Slow {
    pub fn new(name: &'static str) -> Self {
        Self(name, std::time::Instant::now())
    }
}

impl Drop for Slow {
    fn drop(&mut self) {
        let took = self.1.elapsed().as_secs_f32() * 1000.0;
        if took > 8.0 && std::env::var_os("SLOW_TRACE").is_some() {
            println!("slow system {} {took:.0}ms", self.0);
        }
    }
}

/// What the command line asked for.
#[derive(Resource, Default, Clone)]
pub struct Args {
    pub spawn: Vec<String>,
    pub shot: Option<String>,
    pub scene: Option<String>,
    pub view: String,
    pub cols: usize,
    pub clips: Vec<String>,
    /// Control the player from the start.
    pub play: bool,
    /// The way a scene's play camera faces, radians: the way the set route runs.
    pub yaw: f32,
    /// A scene as a strip of this many pictures, `every` seconds apart, from `after`.
    pub strip: usize,
    /// The strip is saved as a moving picture, a frame each `every` seconds, not side by side.
    pub gif: bool,
    /// A moving picture twice the size: for a place, where the other is for a creature.
    pub big: bool,
    pub every: f32,
    /// What a scripted run does (`player::script`).
    pub script: String,
    /// Draw the shape each creature is bumped as.
    pub bodies: bool,
    /// Print the controlled body four times a second.
    pub trace: bool,
    /// `--loose-feet`: feet as the clip has them, not put on the ground that is there: to set
    /// beside the usual.
    pub loose_feet: bool,
    /// The creature to control instead of the player.
    pub play_as: Option<String>,
    /// Seconds into a scene at which its picture is taken.
    pub after: f32,
    /// The world's seed.
    pub seed: u32,
    /// Every surface's grain made in code, none photographed.
    pub plain: bool,
    /// Start from the save file (`--continue`).
    pub resume: bool,
    /// Water reflects the land (deferred rendering, screen-space reflections); the default.
    pub reflect: bool,
    /// Trees drawn as the cubes they are in the simulation, not as shapes.
    pub blocky: bool,
    /// In a scene: draw what the player sees over the world too (`bar`, or with the `pack`
    /// or `admin` window open).
    pub hud: Option<String>,
    /// No sound.
    pub quiet: bool,
    /// How many times as fast the world's clock runs: a long scripted run in a short wait.
    pub speed: f32,
    /// The player starts with a torch lit.
    pub torch: bool,
    /// A scene's camera put at one point and looking at another, whatever the view (`--eye`).
    pub eye: Option<(Vec3, Vec3)>,
    /// A fire burns in the cave's hall (`cave::Fire`).
    pub fire: bool,
    /// A cave is dug into the hillside ahead of where the player starts (`cave`).
    pub cave: bool,
    /// The player starts with a hunk of wood in his hands (to try F on, with real keys).
    pub carry: bool,
    /// A trial body of boxes, moved by its skeleton and its feet and no clips (`limbs`).
    pub limbs: bool,
    /// A person is moved by clips alone, as before the two-legged walker (`stride`): to set the two side by side.
    pub clips_only: bool,
    /// `--as-played`: a scene whose frames are timed makes what is drawn as the game does,
    /// off the frame, and not at once as a picture needs (`make pace`).
    pub as_played: bool,
    /// The trial body is a person's, on two legs (`cube_anim::biped`).
    pub person: bool,
    /// How hard the wind blows, 0 to 1, in place of the weather's.
    pub wind: Option<f32>,
    /// What grows drawn as low-poly shapes, not as cubes (`flora`): for comparison.
    pub shapes: bool,
    /// One thing that grows, stood alone at the start to be looked at (`plants::specimen`).
    pub specimen: Option<String>,
    /// The hour the world starts at, if not the usual mid afternoon.
    pub hour: Option<f32>,
    /// The day the world starts on, counted from the first day of spring.
    pub day: Option<u32>,
    /// The world starts with its creatures in it, living by their own minds.
    pub wild: bool,
    /// In a scene of the living world: the camera keeps on the nearest creature of this kind
    /// that lives by its own mind (`motion::watch`).
    pub watch: Option<String>,
    /// Where spawned creatures are put and a scene looks, metres (x, z).
    pub at: (f32, f32),
    /// A scene's camera: metres from what it looks at, and radians above level.
    pub dist: Option<f32>,
    pub pitch: Option<f32>,
    pub yaw_given: bool,
    /// Balls dug out of the ground (or, with a block's name, built on it) before the world
    /// is first drawn: metres east, south and across.
    pub dug: Vec<(f32, f32, f32, bool)>,
}

const HELP: &str = "\
cargo sandbox                           the window, with the spawn panel
cargo sandbox --play [--as wolf]        start as the player, or as any creature: WASD runs, Shift
                                        sprints, Ctrl walks (in water: dives), Space jumps, the mouse looks, left click
                                        uses what is in hand (a blow, a shot, a spadeful), holding the
                                        right button aims over the shoulder and guards, B or the
                                        middle button builds with the shovel, the
                                        wheel zooms, V is first person, Tab gives the mouse back
cargo sandbox --play --wild             the same in the living world: every creature where it lives,
                                        grazing, roaming, fleeing and hunting by its own mind
cargo sandbox --spawn wolf,player       start with these in the world (`all` for one of each)
cargo sandbox --list                    the creatures that can be spawned
cargo sandbox --shot ID                 no window: shots/ID.png, every clip at six moments
        [--view quarter|side|front|top] [--cols N] [--clips walk,run]
cargo sandbox --scene FILE --spawn all  no window: a picture of the world with creatures in it
cargo sandbox --scene FILE --play [--as ID] [--after SECONDS] [--view first] [--yaw 2.16]
                                        no window: a set route (walk, run, jump, sprint, jump),
                                        pictured from the camera at SECONDS. It runs toward the
                                        hills; --yaw 2.16 runs it into the pond instead
        … --strip 8 [--every 0.1] [--view side]
                                        the same as a strip of pictures, SECONDS then every 0.1 s
                                        after: one image that shows a movement. Side view is level
                                        and square on, for reading feet
        … --script '1 jump; 2 fwd run; 4 stop'
                                        the route, as times and what changes then: fwd back left
                                        right stop · walk run sprint · jump hop · dive surface ·
                                        attack eat hurt · dig put · first third
        … --hour 6.5 [--day 10]         the hour it is (0 to 24; a minute an hour) and the day, from
                                        the first of spring: 7 days a season (3 mid spring, 10 mid
                                        summer, 17 autumn, 24 winter), and a moon full on days 3, 10…
        … --torch                       the player carries a lit torch (T lights it and puts it out)
        … --carry                       the player starts with a hunk of wood in his hands (F puts it down and takes it up)
        … --hud bar|pack|admin          in a scene with --play: the bar, health and clock drawn over it,
                                        with the pack or the admin panel open
        … --no-reflect                  forward rendering, and water that reflects only the sky
        … --continue                    start from terra-cube.save (F5 writes it, F9 reads it)
        … --quiet                       no sound
        … --speed 6                     the clock six times as fast: five minutes heard in one (`make sound`)
        … --specimen oak                one thing that grows, alone at the start, to be looked at:
                                        oak birch pine willow snag young sapling bush log stump rock
        … --plain                       every surface's grain made in code, none from a photograph
        … --cubes | --smooth            the ground as plain cubes, or eased smooth, not in blocks sized
                                        by its slope; --shapes: trees and rocks as
                                        low-poly shapes, not fine cubes; --blades: grass as pointed
                                        blades, --bits: as columns of bits, not slats; --blocky: as the world's cells
        … --wild --at 47,28 [--dist 12] [--pitch 0.3] [--yaw 1.0] [--view high]
        … --watch deer --at -32,-40      the living world with the camera kept on the nearest deer
                                        living by its own mind (prints `watch` with how far off it is);
                                        with --play the player is put down 6 m from it after 2 s
                                        a scene somewhere else: metres east and south of the start
                                        (`cargo world who` says where creatures live), the camera
                                        that far off and that high; `--view high` is from the air
        … --play --view fell [--yaw R] [--dist 14]   on what is over the player's head, from that side
                                        and distance: a tree he cuts seen coming down (in a wood
                                        most sides have a trunk in the way: try a few)
        … --play --cave                 a cave dug into the nearest hillside to look at light in: a passage,
                                        a hall with a hole to the sky, a gallery and a pool (it prints where);
                                        --fire: with a fire burning in the hall
        … --play --eye X,Y,Z --look X,Y,Z   the camera at one point looking at another, metres, whatever the view
        … --view sky [--yaw 1.57] [--pitch 0.16]
                                        from over the trees looking out and a little up, wide: the
                                        sky over the land's edge (yaw 0 north, 1.57 west, 3.14 south;
                                        pitch in radians above level, 1.4 nearly overhead): `make sky`
--dig X,Z,ACROSS and --build X,Z,ACROSS cut a round hole in the ground there, or raise a round
stone on it, to the 5 cm bit, before the first frame: for looking at what broken cells cost.
--seed N makes another island. cargo world (survey, run, map, who) judges one with no window.
        … --gif 12 [--view three|side] [--as wolf]
                                        the same as a moving picture, FILE.gif: with no --script it
                                        is the show reel (walk, run, sprint, jump, stop, attack,
                                        hurt, run and jump again), so every change of clip is seen
--wind 0.8 sets how hard the wind blows (0 to 1) in place of the weather's; --big makes a --gif
twice the size, for a place and not a creature (`make wind` makes shots/wind.gif so).
--limbs puts a four-legged body of boxes in a scene, moved by its skeleton and feet and no
clips, going round the place looked at: standing, walking, running, lying down (`make limbs`).
--clips-only moves a person by its clips alone, the two-legged walker off (`stride`).
--limbs-person puts a person of boxes there instead: standing, walking, running, stopping.
--bodies draws the shape each creature is bumped as (also a tick box in the panel).
--trace prints the controlled body (position, speed, clip, camera) four times a second.
In the window: Tab controls the picked creature, and lets go of it. Otherwise right-drag
orbits, wheel zooms, WASD and QE move, F looks at the picked creature.
Setup trouble: cargo forge doctor";

fn fail(message: &str) -> ! {
    eprintln!("sandbox: {message}");
    std::process::exit(2);
}

/// Creatures on disk that have a model, as the registry will find them.
fn creature_ids() -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir("assets/creatures")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("creature.toml").exists() && e.path().join("model.glb").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

fn parse_args() -> Args {
    let mut args = Args { view: "quarter".into(), cols: 6, yaw: -2.2, every: 0.1, script: player::ROUTE.into(), seed: cube_sim::terrain::SEED, at: (0.25, 0.25), reflect: true, ..default() };
    let mut it = std::env::args().skip(1);
    let mut script_given = false;
    let list = |v: Option<String>| v.map(|s| s.split(',').map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" | "help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            "--list" => {
                println!("{}", creature_ids().join(" "));
                std::process::exit(0);
            }
            "--spawn" => args.spawn = list(it.next()),
            "--shot" => args.shot = it.next(),
            "--scene" => args.scene = it.next(),
            "--view" => args.view = it.next().unwrap_or_default(),
            "--cols" => args.cols = it.next().and_then(|v| v.parse().ok()).unwrap_or(6),
            "--clips" => args.clips = list(it.next()),
            "--play" => args.play = true,
            "--trace" => args.trace = true,
            "--loose-feet" => args.loose_feet = true,
            "--bodies" => args.bodies = true,
            "--script" => {
                args.script = it.next().unwrap_or_default();
                script_given = true;
            }
            "--strip" => args.strip = it.next().and_then(|v| v.parse().ok()).unwrap_or(8),
            "--gif" => {
                // Twelve frames a second for that many seconds: enough to read a gait, and a
                // file a few megabytes long.
                let seconds: f32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(12.0);
                (args.gif, args.every, args.strip) = (true, 1.0 / 12.0, (seconds * 12.0) as usize);
                if !script_given {
                    args.script = player::SHOW.into();
                }
            }
            "--every" => args.every = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.1),
            "--yaw" => {
                args.yaw = it.next().and_then(|v| v.parse().ok()).unwrap_or(-2.2);
                args.yaw_given = true;
            }
            "--wild" => args.wild = true,
            "--watch" => (args.wild, args.watch) = (true, it.next()),
            "--plain" => args.plain = true,
            "--continue" => args.resume = true,
            "--no-reflect" => args.reflect = false,
            "--blocky" => args.blocky = true,
            "--hud" => args.hud = Some(it.next().unwrap_or_else(|| "bar".into())),
            "--quiet" => args.quiet = true,
            "--speed" => args.speed = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--torch" => args.torch = true,
            "--cave" => args.cave = true,
            "--fire" => args.fire = true,
            "--eye" => {
                // `--eye X,Y,Z --look X,Y,Z`: where the camera is and what it looks at, metres.
                let point = |v: Option<String>| v.map(|v| v.split(',').filter_map(|n| n.parse::<f32>().ok()).collect::<Vec<_>>()).filter(|p| p.len() == 3).map(|p| Vec3::new(p[0], p[1], p[2]));
                let eye = point(it.next());
                let look = if it.next().as_deref() == Some("--look") { point(it.next()) } else { None };
                args.eye = eye.zip(look);
            }
            "--carry" => args.carry = true,
            "--limbs" => args.limbs = true,
            "--clips-only" => args.clips_only = true,
            "--as-played" => args.as_played = true,
            "--limbs-person" => (args.limbs, args.person) = (true, true),
            "--big" => args.big = true,
            "--wind" => args.wind = it.next().and_then(|v| v.parse().ok()),
            "--shapes" => args.shapes = true,
            "--bits" => cube_voxel::mesh::BITS.store(true, std::sync::atomic::Ordering::Relaxed),
            "--blades" => cube_voxel::mesh::BLADES.store(true, std::sync::atomic::Ordering::Relaxed),
            "--specimen" => args.specimen = it.next(),
            "--cubes" => cube_voxel::mesh::GROUND.store(cube_voxel::mesh::Ground::Cubes as u8, std::sync::atomic::Ordering::Relaxed),
            "--smooth" => cube_voxel::mesh::GROUND.store(cube_voxel::mesh::Ground::Smooth as u8, std::sync::atomic::Ordering::Relaxed),
            "--stepped" => cube_voxel::mesh::GROUND.store(cube_voxel::mesh::Ground::Stepped as u8, std::sync::atomic::Ordering::Relaxed),
            "--dig" | "--build" => {
                let v: Vec<f32> = it.next().unwrap_or_default().split(',').filter_map(|v| v.parse().ok()).collect();
                if v.len() != 3 {
                    fail("--dig and --build want metres east, south and across: --dig 3,4,1.2");
                }
                args.dug.push((v[0], v[1], v[2], a == "--build"));
            }
            "--hour" => args.hour = it.next().and_then(|v| v.parse().ok()),
            "--day" => args.day = it.next().and_then(|v| v.parse().ok()),
            "--seed" => args.seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(cube_sim::terrain::SEED),
            "--dist" => args.dist = it.next().and_then(|v| v.parse().ok()),
            "--pitch" => args.pitch = it.next().and_then(|v| v.parse().ok()),
            "--at" => {
                let at: Vec<f32> = it.next().unwrap_or_default().split(',').filter_map(|v| v.parse().ok()).collect();
                if at.len() != 2 {
                    fail("--at wants metres east and south of the start: --at 30,-12");
                }
                args.at = (at[0], at[1]);
            }
            "--as" => {
                args.play = true;
                args.play_as = it.next();
            }
            "--after" => args.after = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            other => fail(&format!("unknown argument {other}\n{HELP}")),
        }
    }
    // Names are settled here, so a typo stops before the engine starts instead of in its log.
    let known = creature_ids();
    if known.is_empty() {
        fail("no creatures under assets/creatures; run from the repository root (cargo sandbox does)");
    }
    if args.spawn.iter().any(|s| s == "all") {
        args.spawn = known.clone();
    }
    let me = args.play_as.clone().unwrap_or("player".into());
    if args.play && !args.spawn.contains(&me) {
        args.spawn.insert(0, me);
    }
    if let Some(id) = args.spawn.iter().chain(&args.shot).chain(&args.play_as).find(|id| !known.contains(id)) {
        fail(&format!("no creature `{id}`; there are: {}", known.join(" ")));
    }
    let headless = args.shot.is_some() || args.scene.is_some();
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        fail(if headless { "no display: start `Xvfb :1 -screen 0 1920x1080x24` and set DISPLAY=:1 (docs/development.md)" } else { "no display: the window needs a real screen. For a picture instead: cargo sandbox --scene shots/scene.png --spawn all" });
    }
    args
}

/// Makes the folder that holds `assets/` the working directory, and returns `assets/` in
/// full for Bevy. Started by cargo that is already so; a built program started on its own
/// (a Windows build launched from WSL, a double click) finds it above where it sits.
fn asset_root() -> String {
    let here = std::env::current_dir().unwrap_or_default();
    let exe = std::env::current_exe().unwrap_or_default();
    let found = here.ancestors().chain(exe.ancestors()).find(|dir| dir.join("assets/creatures/style.toml").exists());
    let Some(root) = found else { fail("no assets/ folder here or above the program; run it from the repository") };
    if std::env::set_current_dir(root).is_err() {
        fail(&format!("cannot enter {}", root.display()));
    }
    root.join("assets").to_string_lossy().into_owned()
}

fn main() {
    let assets = asset_root();
    let args = parse_args();
    let scene = args.scene.is_some();
    let shot = args.shot.is_some() || scene;
    let mut app = App::new();
    // A shot draws to an image, with no window: it works on a machine with no screen (winit
    // still wants a display to exist, so a server runs Xvfb; docs/development.md).
    // (A picture must have everything in it, so what is first drawn is made ready on the
    // frame it is first drawn in: 41 ms of a 54 ms frame, once, each time a new kind of thing
    // came into view (the engine's own trace, 2026-11-13). The game as played makes it ready
    // off the frame; `--as-played` has a timed scene do the same.)
    app.add_plugins(DefaultPlugins.set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: shot && !args.as_played, ..default() }).set(if shot {
        WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() }
    } else {
        WindowPlugin { primary_window: Some(Window { title: "terra-cube sandbox".into(), resolution: (1600, 900).into(), ..default() }), ..default() }
    }).set(bevy::asset::AssetPlugin { file_path: assets, ..default() })
    // The world is made on the threads the engine keeps for long work, and by default it
    // keeps a quarter of the machine's and no more than four: most of them, here.
    .set(TaskPoolPlugin { task_pool_options: TaskPoolOptions { async_compute: bevy::app::TaskPoolThreadAssignmentPolicy { min_threads: 2, max_threads: usize::MAX, percent: 0.6, on_thread_spawn: None, on_thread_destroy: None }, ..default() } }))
    .insert_resource(args)
    .init_resource::<loading::Loading>()
    .add_plugins((creatures::CreaturesPlugin, held::HeldPlugin, feet::FeetPlugin, limbs::LimbsPlugin, stride::StridePlugin, loose::LoosePlugin));
    if shot {
        // With no window the loop would idle at sixty a second, and every `pace` would read
        // 17 ms whatever the frame cost: run flat out, so the number is the work.
        // (Not `Continuous`: with no window that never ticks at all. An idle wait of nothing.)
        let at_once = bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_micros(200));
        app.insert_resource(bevy::winit::WinitSettings { focused_mode: at_once, unfocused_mode: at_once });
    }
    if scene {
        app.add_plugins((world::WorldPlugin, far::FarPlugin, flora::FloraPlugin, air::AirPlugin, clouds::CloudsPlugin, vault::VaultPlugin, cave::CavePlugin, audio::SoundPlugin, motion::MotionPlugin, player::PlayPlugin, shot::ScenePlugin));
        if app.world().resource::<Args>().hud.is_some() {
            app.add_plugins((bevy_egui::EguiPlugin::default(), hud::HudPlugin));
        }
    } else if shot {
        app.add_plugins(shot::ShotPlugin);
    } else {
        app.add_plugins((world::WorldPlugin, far::FarPlugin, flora::FloraPlugin, air::AirPlugin, clouds::CloudsPlugin, vault::VaultPlugin, cave::CavePlugin, audio::SoundPlugin, camera::CameraPlugin, motion::MotionPlugin, player::PlayPlugin, ui::UiPlugin, hud::HudPlugin));
        // The window opens on a loading screen when someone is to play in it. Never a scene:
        // a picture does its own waiting (`shot`).
        if app.world().resource::<Args>().play {
            app.add_plugins(loading::LoadingPlugin);
        }
    }
    let speed = app.world().resource::<Args>().speed;
    if speed > 0.0 && speed != 1.0 {
        app.world_mut().resource_mut::<Time<Virtual>>().set_relative_speed(speed);
    }
    app.run();
}
