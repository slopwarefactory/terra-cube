//! Control: driving a creature with the keys and a camera that follows it. Any creature can be
//! controlled, which is how the player is played and how an animal's gaits are judged at real
//! speeds. The movement itself is `motion` (and under it `cube_sim::body`); this file turns
//! keys into what is asked of the body, and flies the camera, behind it or from its eyes.

use crate::{
    Args,
    camera::Orbit,
    creatures::{Creature, Registry, Wander},
    motion::{Gait, Live, Mover},
    world::Terrain,
};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>().init_resource::<Menus>().add_systems(Update, (start, keys, script, drive, follow, edit, cursor, trace).chain());
        app.add_systems(Update, (torch, arrows, saves, mark));
    }
}

/// What is asked of the controlled creature, from the keys or from a script.
#[derive(Resource, Default)]
pub struct Controls {
    /// x to the camera's right, y away from the camera.
    pub dir: Vec2,
    pub gait: Gait,
    pub jump: bool,
    /// Swim down.
    pub dive: bool,
    /// Dig a hole where it looks, or build a lump there: once, then cleared.
    pub dig: bool,
    /// What a dig cuts and how big a bite it takes (the tool in hand's: `Item::cuts`);
    /// nothing, and it is a plain spadeful of anything (a script's `dig`).
    pub tool: Option<(cube_sim::items::Stuff, f32)>,
    /// A blow with a tool that is on its way: the seconds until it lands and cuts, and what
    /// it cuts and how big a bite.
    pub blow: Option<(f32, cube_sim::items::Stuff, f32)>,
    /// What a `put` puts down: what is in hand, as the block it is in the world.
    pub lump: Option<cube_voxel::Block>,
    pub put: bool,
    /// A hunk of wood or stone is to be taken up (the one looked at, or the nearest within
    /// reach), or the one carried put down: F, or a script's `lift`. And the one carried is
    /// to be put down where he looks (using what is in hand, with a hunk in the hands).
    pub grab: bool,
    /// (A script's reach: it cannot walk up to what it felled, so it takes a hunk up from as
    /// far off as it chops, and puts it down by itself. Not one within its hands' reach:
    /// that is one it has put down, and it took the same one up four times running.)
    pub far: bool,
    pub lay: Option<cube_voxel::Block>,
    /// (A script's, for trying what a cut does: a bite out of the nearest tree's trunk, a
    /// metre up, each one further across it than the last.)
    pub chop: bool,
    /// (And a blow of a pick at the nearest boulder: a script's `pick`.)
    pub pick: bool,
    /// (And a block of stone let go five metres up, three ahead: to see what comes down hard do.)
    pub drop: bool,
    /// (And every changed region dropped and made again from the seed and the journal: `world::regions`.)
    pub forget: bool,
    /// Something to do on the spot, once: a blow, a bite of grass, a flinch.
    pub act: Option<cube_sim::life::Act>,
    /// What is in hand is to be used, once: a blow, a bite, a spadeful.
    pub use_held: bool,
    /// A slot of the bar to take in hand, or a step along it.
    pub hold: Option<usize>,
    pub cycle: i32,
    /// The guard is up (the right button held, with anything but the shovel in hand).
    pub guard: bool,
    /// Aiming (the right button held): the camera comes in over the shoulder, a mark shows
    /// where it looks, and the body turns to face that way, so a blow, a shot or a spadeful
    /// goes where the mark is.
    pub aim: bool,
    /// A kind of creature to be put down just ahead (a script's `spawn=ID`).
    pub spawn: Option<String>,
    /// An arrow loosed: from where, and which way.
    pub shot: Option<(Vec3, Vec3)>,
    /// The player's state is to be written to the save file, or read from it.
    pub save: bool,
    pub load: bool,
}

/// The windows that take the mouse from the camera while they are open: the pack, the admin
/// panel.
#[derive(Resource, Default)]
pub struct Menus {
    pub pack: bool,
    pub admin: bool,
}

impl Menus {
    pub fn open(&self) -> bool {
        self.pack || self.admin
    }
}

/// On the creature being controlled.
#[derive(Component, Default)]
pub struct Controlled {
    /// Seen from its own eyes, the model hidden.
    pub first_person: bool,
}

/// `--play`: control the player (`--as ID`: that creature) from the start. Tab takes and gives
/// up control of the picked creature after that.
fn start(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    keys: Res<ButtonInput<KeyCode>>,
    selected: Option<Res<crate::ui::Selected>>,
    creatures: Query<(Entity, &Creature, &Transform), Without<crate::creatures::Picture>>,
    controlled: Query<Entity, With<Controlled>>,
    mut cameras: Query<&mut Orbit>,
    mut begun: Local<bool>,
    // The creature last let go of, and what was picked in the panel at the time.
    mut last: Local<Option<(Entity, Option<Entity>)>>,
    loading: Res<crate::loading::Loading>,
) {
    let toggle = keys.just_pressed(KeyCode::Tab) && !loading.up;
    let picked_now = selected.as_ref().and_then(|s| s.0);
    let begin = args.play && !*begun && !creatures.is_empty();
    if toggle && let Some(entity) = controlled.iter().next() {
        commands.entity(entity).remove::<Controlled>().insert(Visibility::Inherited);
        *last = Some((entity, picked_now));
        return;
    }
    if !(toggle || begin) {
        return;
    }
    *begun = true;
    let picked = selected.and_then(|s| s.0).and_then(|e| creatures.get(e).ok());
    let wanted = args.play_as.as_deref().unwrap_or("player");
    // (Of its kind, the one nearest where the scene is: with the island's creatures about,
    // `--as deer` took whichever deer came first, somewhere on the island, and a scene of it
    // showed nothing.)
    let here = Vec3::new(args.at.0, 0.0, args.at.1);
    let named = creatures.iter().filter(|(_, c, _)| registry.0[c.def].id == wanted).min_by(|a, b| a.2.translation.with_y(0.0).distance(here).total_cmp(&b.2.translation.with_y(0.0).distance(here)));
    // Tab goes back to the creature it let go of, unless another has been picked since.
    let again = last.and_then(|(entity, picked_then)| (picked_then == picked_now).then(|| creatures.get(entity).ok()).flatten());
    let Some((entity, creature, at)) = (if begin { named.or(picked) } else { again.or(picked).or(named) }).or_else(|| creatures.iter().next()) else { return };
    let def = &registry.0[creature.def];
    commands.entity(entity).remove::<Wander>().insert(Controlled { first_person: args.view == "first" });
    for mut orbit in &mut cameras {
        orbit.distance = (def.meta.height.max(def.meta.length) * 3.2).clamp(2.5, 9.0);
        orbit.pitch = 0.28;
        // Behind it, looking the way it faces. A scene's camera is aimed by `--yaw` instead.
        if args.scene.is_none() {
            orbit.yaw = at.rotation.to_euler(EulerRot::YXZ).0 + std::f32::consts::PI;
        }
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, buttons: Res<ButtonInput<MouseButton>>, args: Res<Args>, mut loading: ResMut<crate::loading::Loading>, mut menus: ResMut<Menus>, mut controls: ResMut<Controls>, mut controlled: Query<&mut Controlled>) {
    if args.scene.is_some() {
        return;
    }
    // Behind the loading screen no key does anything; each one pressed is counted, so a test
    // can tell a key that was turned away from one that never arrived.
    if loading.up {
        loading.barred += keys.get_just_pressed().len() as u32;
        *controls = default();
        return;
    }
    // The pack opens on I and the admin panel on F1; Escape shuts both. While either is
    // open the keys and the mouse are the window's.
    if keys.just_pressed(KeyCode::KeyI) {
        menus.pack = !menus.pack;
    }
    if keys.just_pressed(KeyCode::F5) {
        controls.save = true;
    }
    if keys.just_pressed(KeyCode::F9) {
        controls.load = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        menus.admin = !menus.admin;
    }
    if keys.just_pressed(KeyCode::Escape) {
        (menus.pack, menus.admin) = (false, false);
    }
    if menus.open() {
        (controls.dir, controls.jump) = (Vec2::ZERO, false);
        return;
    }
    let held = |k: KeyCode| keys.pressed(k) as i32 as f32;
    controls.dir = Vec2::new(held(KeyCode::KeyD) - held(KeyCode::KeyA), held(KeyCode::KeyW) - held(KeyCode::KeyS));
    controls.gait = if keys.pressed(KeyCode::ControlLeft) {
        Gait::Walk
    } else if keys.pressed(KeyCode::ShiftLeft) {
        Gait::Sprint
    } else {
        Gait::Run
    };
    controls.jump = keys.pressed(KeyCode::Space);
    controls.dive = keys.pressed(KeyCode::ControlLeft);
    // The left button uses what is in hand; 1 to 8 take a slot of the bar, Q and E step
    // along it, T takes the torch.
    controls.use_held |= buttons.just_pressed(MouseButton::Left) && !controlled.is_empty();
    for (slot, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8].into_iter().enumerate() {
        if keys.just_pressed(key) {
            controls.hold = Some(slot);
        }
    }
    controls.cycle += keys.just_pressed(KeyCode::KeyE) as i32 - keys.just_pressed(KeyCode::KeyQ) as i32;
    if keys.just_pressed(KeyCode::KeyT) {
        controls.hold = Some(usize::MAX);
    }
    // (Building was the right button's, a click; held, the right button now aims, and a
    // lump is laid with the middle button or B.)
    controls.put |= (buttons.just_pressed(MouseButton::Middle) || keys.just_pressed(KeyCode::KeyB)) && !controlled.is_empty();
    controls.grab |= keys.just_pressed(KeyCode::KeyF) && !controlled.is_empty();
    controls.guard = buttons.pressed(MouseButton::Right);
    controls.aim = buttons.pressed(MouseButton::Right) && !controlled.is_empty();
    if keys.just_pressed(KeyCode::KeyV) {
        for mut c in &mut controlled {
            c.first_person = !c.first_person;
        }
    }
}

/// The route a scripted run takes when `--script` gives none: toward where the camera faces.
/// What `--gif` runs when no script is given: every gait and every change between them, a
/// turn about, a jump, a stop, a blow and a flinch, then off again. Out and back, so it stays
/// in the clearing at the start where nothing stands between it and the camera.
pub const SHOW: &str = "0.5 fwd walk; 2.5 run; 4 back sprint; 5 jump; 5.6 stop; 6.4 attack; 8 hurt; 8.8 left run; 9.6 jump; 10.4 right walk; 11.5 stop";

pub const ROUTE: &str = "0.5 fwd walk; 2 run; 3 jump; 5 sprint; 6 jump";

/// With no window there are no keys: `--script` stands in for them. It is steps split by `;`,
/// each a time in seconds and what changes then: `fwd back left right stop` (direction),
/// `walk run sprint` (gait), `jump` (held, a full jump), `hop` (tapped), `dive surface` (in water), `dig put` (a hole or a lump where it looks), `first third` (view).
fn script(time: Res<Time>, args: Res<Args>, stage: Option<Res<crate::shot::Stage>>, mut controls: ResMut<Controls>, mut controlled: Query<&mut Controlled>, mut clock: Local<(f32, f32)>) {
    if args.scene.is_none() || !args.play || controlled.is_empty() {
        return;
    }
    // A script's times are on the picture's clock: seconds since the scene was ready.
    let (before, now) = (clock.0, stage.map_or(clock.0 + time.delta_secs(), |s| s.ready_for));
    clock.0 = now;
    controls.jump = now < clock.1;
    for step in args.script.split(';') {
        let mut words = step.split_whitespace();
        let Some(at) = words.next().and_then(|w| w.parse::<f32>().ok()) else { continue };
        if !(before < at && at <= now) {
            continue;
        }
        for word in words {
            match word {
                "fwd" => controls.dir = Vec2::Y,
                "back" => controls.dir = Vec2::NEG_Y,
                "left" => controls.dir = Vec2::NEG_X,
                "right" => controls.dir = Vec2::X,
                "stop" => controls.dir = Vec2::ZERO,
                "walk" => controls.gait = Gait::Walk,
                "run" => controls.gait = Gait::Run,
                "sprint" => controls.gait = Gait::Sprint,
                "jump" => clock.1 = now + 0.4,
                "dive" => controls.dive = true,
                // (And a script's reach: it aims at the ground before it, `edit`.)
                "attack" | "use" => (controls.use_held, controls.far) = (true, true),
                "torch" => controls.hold = Some(usize::MAX),
                "save" => controls.save = true,
                "load" => controls.load = true,
                "guard" => controls.guard = true,
                "unguard" => controls.guard = false,
                "hold1" | "hold2" | "hold3" | "hold4" | "hold5" | "hold6" | "hold7" | "hold8" => controls.hold = word[4..].parse::<usize>().ok().map(|n| n - 1),
                "eat" => controls.act = Some(cube_sim::life::Act::Eat),
                "hurt" => controls.act = Some(cube_sim::life::Act::Hurt),
                "dig" => controls.dig = true,
                "chop" => controls.chop = true,
                "lift" => (controls.grab, controls.far) = (true, true),
                "pick" => controls.pick = true,
                "drop" => controls.drop = true,
                "forget" => controls.forget = true,
                "put" => controls.put = true,
                "surface" => controls.dive = false,
                "hop" => clock.1 = now + 0.04,
                "aim" | "noaim" => controls.aim = word == "aim",
                "first" | "third" => controlled.iter_mut().for_each(|mut c| c.first_person = word == "first"),
                // `spawn=goblin`: one of that kind is put down two metres ahead, for a fight.
                other if other.starts_with("spawn=") => controls.spawn = Some(other[6..].to_owned()),
                other => warn!("--script: no such step `{other}`"),
            }
        }
    }
}

/// `--trace`, and every scripted run: the controlled body in a line, four times a second.
fn trace(time: Res<Time>, args: Res<Args>, live: Res<Live>, controlled: Query<(&Mover, &Creature, &Controlled)>, registry: Res<Registry>, cameras: Query<&Orbit>, mut clock: Local<(f32, f32)>) {
    if !(args.trace || args.scene.is_some() && args.play) {
        return;
    }
    let Ok((m, creature, c)) = controlled.single() else { return };
    let Some(agent) = live.agent(m) else { return };
    clock.0 += time.delta_secs();
    if clock.0 >= clock.1 {
        clock.1 += if args.trace { 0.25 } else { 1.0 };
        let def = &registry.0[creature.def];
        let clip = def.clips.get(creature.clip).map_or("-", |c| c.0.as_str());
        let b = &agent.body;
        let yaw = cameras.iter().next().map_or(0.0, |o| o.yaw);
        println!(
            "play t={:.1} pos={:.1},{:.1},{:.1} speed={:.2} grounded={} swimming={} clip={clip}x{:.2} yaw={yaw:.2} first={} health={:.0} held={} carries={} landed={} kills={}",
            clock.0, b.pos.x, b.pos.y, b.pos.z, b.ground_speed(), b.grounded, b.swimming, creature.speed, c.first_person, agent.health, agent.pack.as_ref().and_then(|p| p.held()).map_or("nothing", |i| i.name()), agent.carries.map_or("nothing".to_owned(), |b| format!("{b:?}")), agent.log.landed.map_or("nothing".to_owned(), |p| format!("{p:?}")), agent.log.kills
        );
    }
}

/// Hands the controls to the controlled creature's body, turned to the way the camera faces.
fn drive(mut controls: ResMut<Controls>, time: Res<Time>, args: Res<Args>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, cameras: Query<&Orbit>, mut controlled: Query<(&Controlled, &Mover, &mut Visibility)>, mut carried: Local<bool>) {
    // A scripted route runs the way `--yaw` says, whatever the camera is doing.
    let yaw = if args.scene.is_some() { args.yaw } else { cameras.iter().next().map_or(0.0, |o| o.yaw) };
    // The camera looks along -Z turned by its yaw; right is +X turned the same.
    let (sin, cos) = yaw.sin_cos();
    for (c, mover, mut visibility) in &mut controlled {
        // (`--carry`: a hunk of wood in his hands from the start.)
        if args.carry && !std::mem::replace(&mut *carried, true) {
            live.0.lifted(mover.id, cube_voxel::Block::Billet);
        }
        // What is in hand, and using it: the simulation says what that does.
        if let Some(pack) = live.agent_mut(mover).and_then(|a| a.pack.as_mut()) {
            match controls.hold.take() {
                Some(usize::MAX) => {
                    if let Some(slot) = pack.slots[..cube_sim::items::BAR].iter().position(|s| s.is_some_and(|s| s.0.lights())) {
                        pack.hold(slot);
                    }
                }
                Some(slot) => pack.hold(slot),
                None => {}
            }
            pack.cycle(std::mem::take(&mut controls.cycle));
        }
        if let (Some(id), Some(terrain)) = (controls.spawn.take(), &terrain)
            && let (Some(kind), Some(me)) = (live.0.kind(&id), live.agent(mover).map(|a| (a.body.pos, a.facing)))
        {
            let at = me.0 + Vec3::new(me.1.sin(), 0.0, me.1.cos()) * 2.0;
            live.0.add(&terrain.0, kind, Vec3::new(at.x, terrain.ground(at.x, at.z), at.z), me.1 + std::f32::consts::PI, cube_sim::life::Driver::Mind);
        }
        if let Some((left, stuff, across)) = controls.blow {
            let left = left - time.delta_secs();
            controls.blow = (left > 0.0).then_some((left, stuff, across));
            if left <= 0.0 {
                (controls.dig, controls.tool) = (true, Some((stuff, across)));
            }
        }
        if std::mem::take(&mut controls.use_held) {
            let used = live.0.use_held(mover.id);
            // What a tool cuts is cut as its blow lands, not as the button goes down.
            if let cube_sim::items::Used::Cut(stuff, across) = used {
                controls.blow = Some((cube_sim::life::SWING, stuff, across));
            }
            // Earth in hand is put down where he looks; and so is a hunk in the hands.
            if let cube_sim::items::Used::Place(block) = used {
                (controls.put, controls.lump) = (true, Some(block));
            }
            if let cube_sim::items::Used::Lay(block) = used {
                controls.lay = Some(block);
            }
            // A shot: an arrow is seen to fly the way the archer faces.
            if let Some(a) = live.agent(mover).filter(|a| used == cube_sim::items::Used::Swing && a.pack.as_ref().and_then(|p| p.held()) == Some(cube_sim::items::Item::Bow)) {
                controls.shot = Some((a.body.pos + Vec3::Y * 1.4, Vec3::new(a.facing.sin(), 0.0, a.facing.cos())));
            }
        }
        // (With a hunk in his hands he walks: it is heavy, and hauling is meant to be work.)
        let gait = if live.agent(mover).is_some_and(|a| a.carries.is_some()) { Gait::Walk } else { controls.gait };
        // (And no faster than what it weighs lets him: `matter::burden`.)
        let burden = live.agent(mover).and_then(|a| a.carries).map_or(1.0, |b| cube_sim::matter::burden(cube_sim::matter::hunk(b)));
        let Some(speed) = live.kind(mover).map(|k| k.speed(gait) * burden) else { continue };
        let Some(m) = live.agent_mut(mover) else { continue };
        // (F with a hunk in the hands puts it down.)
        if controls.grab && m.carries.is_some() {
            (controls.grab, controls.lay) = (false, m.carries);
        }
        // The right button guards, with anything but a tool that cuts in hand.
        let shovel = m.pack.as_ref().and_then(|p| p.held()).is_some_and(|i| i.cuts().is_some());
        m.guard = controls.guard && !shovel;
        // (The middle button puts down what is in hand too, if it is a thing that is put down.)
        if controls.put && controls.lump.is_none() {
            controls.lump = m.pack.as_ref().and_then(|p| p.held()).and_then(|i| i.places());
            controls.put = controls.lump.is_some();
        }
        m.intent.dir = Vec2::new(cos, -sin) * controls.dir.x + Vec2::new(-sin, -cos) * controls.dir.y;
        m.intent.speed = speed;
        // Wanted for a moment after the press, so one just before landing still jumps.
        if controls.jump {
            m.want_jump = 0.12;
        }
        m.hold_jump = controls.jump;
        match controls.act.take() {
            Some(cube_sim::life::Act::Hurt) => m.hurt = 0.5,
            Some(act) => m.doing = Some((act, if act == cube_sim::life::Act::Eat { 3.0 } else { 1.3 })),
            None => {}
        }
        m.intent.dive = controls.dive;
        m.gait = controls.gait;
        m.turn_to = (c.first_person || controls.aim).then_some(yaw + std::f32::consts::PI);
        let want = if c.first_person { Visibility::Hidden } else { Visibility::Inherited };
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// The camera on the controlled creature: the mouse turns it. Behind the creature the wheel
/// moves it in and out and ground in the way pulls it closer; in first person it is the eyes.
fn follow(
    args: Res<Args>,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    menus: Res<Menus>,
    loading: Res<crate::loading::Loading>,
    controlled: Query<(&Controlled, &Creature, &Transform), Without<Orbit>>,
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
    time: Res<Time>,
    mut steady: Local<Option<(f32, f32)>>,
    controls: Res<Controls>,
    mut aim: Local<f32>,
) {
    let (Ok((c, creature, at)), Some(terrain)) = (controlled.single(), terrain) else { return };
    let def = &registry.0[creature.def];
    // The camera rides a height of its own that comes after the body's: the ground is blocks,
    // each a step of a hand or more, and a camera fixed to the body took every one of them as
    // a jolt (the developer: "the camera shakes a lot when moving up and down terrain"). It
    // is never more than half a metre behind, and a long drop or a waking elsewhere is taken
    // at once.
    let dt = time.delta_secs();
    let (mut height, mut out) = steady.unwrap_or((at.translation.y, f32::MAX));
    let behind = at.translation.y - height;
    height = if behind.abs() > 3.0 || std::env::var_os("CAMERA_RAW").is_some() { at.translation.y } else { at.translation.y - (behind * (-(if c.first_person { 12.0 } else { 7.0 }) * dt).exp()).clamp(-0.5, 0.5) };
    let at = &Transform { translation: at.translation.with_y(height), ..*at };
    for (mut orbit, mut transform) in &mut cameras {
        if args.scene.is_none() && !menus.open() && !loading.up {
            orbit.yaw -= motion.delta.x * 0.003;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.003).clamp(-1.3, 1.45);
            if !c.first_person {
                orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(1.2, 30.0);
            }
        }
        if let (Some((eye, look)), true) = (args.eye, args.scene.is_some()) {
            // Put where it was asked and looking where it was asked: a place a body's own
            // camera cannot stand, as the far end of a cave.
            *transform = Transform::from_translation(eye).looking_at(look, Vec3::Y);
            continue;
        }
        if args.scene.is_some() && (args.view == "hand" || args.view == "lefthand") {
            let side = if args.view == "hand" { 1.0 } else { -1.0 };
            // Close on what the right hand holds (`lefthand`: the left), from that side and a little ahead: whether
            // a handle is in a closed hand cannot be read from further off.
            let focus = at.translation + Vec3::Y * def.meta.height * 0.56 + at.rotation * Vec3::new(0.2 * side, 0.0, 0.2) * def.meta.height / 1.75;
            transform.translation = focus + Quat::from_rotation_y(args.yaw + 2.2 * side) * Vec3::Z * def.meta.height * 0.8 + Vec3::Y * def.meta.height * 0.05;
            transform.look_at(focus, Vec3::Y);
            continue;
        }
        if args.scene.is_some() && args.view == "fell" {
            // On what is over his head, from `--dist` metres off (14) on the side `--yaw`
            // says: to see a tree he cuts come down, all of it. (In a wood most sides have a
            // trunk in the way: try a few.)
            let focus = at.translation + Vec3::Y * 5.0;
            transform.translation = focus + Quat::from_rotation_y(args.yaw) * Vec3::Z * args.dist.unwrap_or(14.0) + Vec3::Y * 2.0;
            transform.look_at(focus, Vec3::Y);
            continue;
        }
        if args.scene.is_some() && (args.view == "side" || args.view == "three") {
            // For filmstrips: level, square on to the route, close enough to read the feet. `three`
            // is from ahead and to one side, for a moving picture: the face and the gait both show.
            let size = def.meta.height.max(def.meta.length);
            let focus = at.translation + Vec3::Y * def.meta.height * 0.5;
            let round = if args.view == "three" { std::f32::consts::PI - 0.95 } else { std::f32::consts::FRAC_PI_2 };
            transform.translation = focus + Quat::from_rotation_y(args.yaw + round) * Vec3::Z * size * if args.gif { 2.3 } else { 3.0 } + Vec3::Y * size * 0.25;
            transform.look_at(focus, Vec3::Y);
            continue;
        }
        let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0) * Vec3::Z;
        if c.first_person {
            let eyes = at.translation + Vec3::Y * def.meta.height * 0.92;
            transform.translation = eyes;
            transform.look_to(-back, Vec3::Y);
            continue;
        }
        // Aiming, it comes in over the right shoulder: close behind, and looking past him
        // from his right, so what he aims at is seen and not his back.
        *aim += ((controls.aim as u8 as f32) - *aim) * (1.0 - (-12.0 * dt).exp());
        let (far, aside) = (orbit.distance + (orbit.distance.min(2.1) - orbit.distance) * *aim, Quat::from_rotation_y(orbit.yaw) * Vec3::X * 0.55 * *aim);
        orbit.focus = at.translation + Vec3::Y * def.meta.height * (0.85 + 0.1 * *aim) + aside;
        // Ground in the way pulls it in quickly and lets it out slowly: on a slope the ground
        // behind comes and goes with every step, and the camera went in and out with it.
        let clear = terrain.0.raycast(orbit.focus, back, far + 0.3).map_or(far, |hit| (hit - 0.3).max(0.4)).min(far);
        out = if out == f32::MAX { clear } else { (out + (clear - out) * (1.0 - (-(if clear < out { 22.0 } else { 2.5 }) * dt).exp())).min(far.max(clear)) };
        transform.translation = orbit.focus + back * out;
        transform.look_to(-back, Vec3::Y);
    }
    *steady = Some((height, out));
}

/// The mark in the middle of the picture, shown while aiming or looking through his eyes.
#[derive(Component)]
struct Mark;

fn mark(mut commands: Commands, controls: Res<Controls>, menus: Res<Menus>, controlled: Query<&Controlled>, mut marks: Query<&mut Visibility, With<Mark>>) {
    let show = !menus.open() && controlled.iter().next().is_some_and(|c| c.first_person || controls.aim);
    if marks.is_empty() {
        commands.spawn((Mark, Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(50.0), width: px(4), height: px(4), margin: UiRect::all(px(-2)), ..default() }, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.8)), Visibility::Hidden));
    }
    for mut seen in &mut marks {
        *seen = if show { Visibility::Visible } else { Visibility::Hidden };
    }
}

/// How far the controlled creature reaches to dig or build, and how big a hole and a lump are,
/// metres. Both are cut to the bit: a hole is a rounded bowl, not a missing cell.
const REACH: f32 = 8.0;
const HOLE: f32 = 0.4;
/// (A lump is one cell's worth, as one carried is: at 0.18 m it was a cell and a half, and
/// earth dug and put back came to more than was dug.)
const LUMP: f32 = 0.155;
/// How far he reaches with his hands, to take a hunk up or put one down, metres.
const HANDS: f32 = 2.5;

/// Digs or builds where the controlled creature looks: along the camera's line, from its eyes.
#[allow(clippy::too_many_arguments)]
fn edit(mut regions: ResMut<crate::world::Regions>, mut controls: ResMut<Controls>, mut live: ResMut<Live>, mut forms: ResMut<crate::flora::Forms>, mut grown: ResMut<crate::flora::Grown>, terrain: Option<ResMut<Terrain>>, mut dirty: ResMut<crate::world::Dirty>, controlled: Query<(&Mover, &Transform), With<Controlled>>, cameras: Query<&Transform, With<Orbit>>, mut chopping: Local<Option<(Vec3, u32)>>, mut felled: Local<bool>) {
    let (mut dig, put, chop, mut tool, lump) = (std::mem::take(&mut controls.dig), std::mem::take(&mut controls.put), std::mem::take(&mut controls.chop), controls.tool.take(), controls.lump.take());
    let (grab, lay, pick) = (std::mem::take(&mut controls.grab), controls.lay.take(), std::mem::take(&mut controls.pick));
    let (Some(mut terrain), Ok((mover, at)), Some(camera)) = (terrain, controlled.single(), cameras.iter().next()) else { return };
    if std::mem::take(&mut controls.forget) {
        regions.forget = true;
    }
    // Aiming, a blow or an arrow goes along the line he looks: what of a body it lands on
    // is the simulation's to say (`Kind::struck`).
    if let Some(m) = live.agent_mut(mover) {
        m.aim = controls.aim.then(|| (camera.translation, *camera.forward()));
    }
    if std::mem::take(&mut controls.drop) {
        let middle = at.translation + at.rotation * Vec3::Z * 3.0 + Vec3::Y * 5.0;
        // (Put, as anything is: it is in the journal, and with nothing under it it falls.)
        let changed = live.0.loose.put(&mut terrain.0, middle, 0.75, cube_voxel::Block::Stone);
        terrain.touched(&mut dirty, changed);
    }
    // A hunk taken up or put down, by hand: no tool. Taken up, the one he looks at, or
    // failing that the nearest within his reach; put down, where he looks if that is within
    // his reach and not on himself, or else a step ahead of him.
    if grab || lay.is_some() {
        use cube_voxel::CELL;
        let along = *camera.forward();
        let (feet, ahead) = (at.translation, at.translation + at.rotation * Vec3::Z * 1.1 + Vec3::Y * 0.3);
        let seen = terrain.0.raycast(camera.translation, along, HANDS + camera.translation.distance(feet)).map(|far| camera.translation + along * far).filter(|hit| hit.distance(feet + Vec3::Y) < HANDS);
        if let Some(block) = lay {
            let to = seen.filter(|_| !controls.far).map(|hit| hit - along * CELL * 0.5).filter(|to| to.with_y(0.0).distance(feet.with_y(0.0)) > 0.8).unwrap_or(ahead);
            // Never on himself. A hunk lies on the square it is over, which may reach back
            // half a metre from where it was aimed: put down, it came down into him ("it
            // kind of can fall into you", the developer, 2026-11-13). Its square is moved
            // on, the way he faces, until all of it is clear of where he stands.
            let side = cube_sim::loose::HUNK as f32 * CELL;
            let clear = |to: Vec3| {
                let lo = Vec3::new((to.x / side).floor() * side, 0.0, (to.z / side).floor() * side);
                let near = Vec3::new(feet.x.clamp(lo.x, lo.x + side), 0.0, feet.z.clamp(lo.z, lo.z + side));
                near.distance(feet.with_y(0.0)) > 0.45
            };
            let on = (at.rotation * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z);
            let to = (0..8).map(|k| to + on * k as f32 * CELL).find(|to| clear(*to)).unwrap_or(to + on * side * 2.0);
            if let Some((corner, changed)) = live.0.loose.lay(&mut terrain.0, to, block) {
                if std::env::var_os("LOOSE_TRACE").is_some() {
                    // (And what is under it there, for whoever asks whether it was put on another.)
                    let under = (1..=8).map(|down| terrain.0.get(corner - IVec3::Y * down)).find(|b| b.solid());
                    println!("loose: laid {block:?} at cell {} {} {} on {under:?}", corner.x, corner.y, corner.z);
                }
                terrain.touched(&mut dirty, changed);
                live.0.laid(mover.id);
            }
        } else if live.0.free(mover.id) {
            let looked = seen.map(|hit| hit + along * 0.05).filter(|hit| cube_sim::loose::hunk_at(&terrain.0, (*hit / CELL).floor().as_ivec3()).is_some());
            let near = || {
                let (c, r) = (((feet + Vec3::Y) / CELL).floor().as_ivec3(), (if controls.far { 14.0 } else { HANDS } / CELL) as i32);
                (-r.min(16)..=r.min(16)).flat_map(|y| (-r..=r).flat_map(move |z| (-r..=r).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| terrain.0.get(*cell).hunk()).map(|cell| (cell.as_vec3() + 0.5) * CELL).filter(|p| !controls.far || p.distance(feet) > HANDS).min_by(|a, b| a.distance(feet + Vec3::Y).total_cmp(&b.distance(feet + Vec3::Y)))
            };
            if let Some(from) = looked.or_else(near)
                && let Some((block, changed)) = live.0.loose.lift(&mut terrain.0, from)
            {
                if std::env::var_os("LOOSE_TRACE").is_some() {
                    println!("loose: lifted {block:?} at {from:.2?}");
                }
                terrain.touched(&mut dirty, changed);
                live.0.lifted(mover.id, block);
            } else if let Some(changed) = (if controls.far { live.0.loose.shovable(feet, 14.0) } else { seen.map(|hit| hit + along * 0.05) }).and_then(|at| live.0.loose.shove(&mut terrain.0, at, if controls.far { (at - feet).with_y(0.0).normalize_or(along) } else { along })) {
                // No hunk to take up, and what he looks at lies loose and is light: he
                // heaves it away from him.
                terrain.touched(&mut dirty, changed);
            } else {
                // Nothing to take up or heave: then the meat of what lies dead at his feet.
                let meat = live.0.butcher(mover.id);
                if meat > 0 && std::env::var_os("LOOSE_TRACE").is_some() {
                    println!("loose: took {meat} meat from what lay dead");
                }
            }
        }
    }
    if !(dig || put || chop || pick) {
        return;
    }
    // A script's chop: a blow at the nearest standing trunk until it is down, then blows
    // along it as it lies (`Loose::chop` says what each does: limbs it, then breaks it).
    // (Not while what it cut is still coming down: it went on to the trees beyond, a blow each.)
    let mut aimed = None;
    if chop && std::env::var_os("LOOSE_TRACE").is_some() && live.0.loose.pieces.iter().any(|p| p.cells.len() >= 200) {
        println!("loose: a script's blow waits for what is in the air: {:?}", live.0.loose.pieces.iter().filter(|p| p.cells.len() >= 200).map(|p| (p.id, p.cells.len(), p.now(p.pivot).round())).collect::<Vec<_>>());
    }
    if chop && live.0.loose.pieces.iter().all(|p| p.cells.len() < 200) {
        // (A tree, not a stump: something over three metres of trunk.)
        let near = |l: &&cube_sim::plants::Limb| l.roots > 0 && l.b.y - l.a.y > 3.0 && l.a.with_y(0.0).distance(at.translation.with_y(0.0)) < 14.0;
        // (What was being chopped has come down: the count begins again on what is next.)
        if chopping.is_some() && live.0.loose.things.iter().any(|t| t.lies.is_some()) && !*felled {
            (*chopping, *felled) = (None, true);
        }
        // (Once one is down the blows are for it as it lies, not for the next one standing:
        // each tree round it took a blow.)
        let foot = if *felled { None } else { chopping.map(|c| c.0) }.or_else(|| if *felled { None } else { forms.0.values().flat_map(|f| f.limbs.iter()).filter(near).min_by(|a, b| a.a.distance(at.translation).total_cmp(&b.a.distance(at.translation))).map(|l| l.a + Vec3::Y * (cube_sim::plants::Limb::UNDER + 1.0)) });
        // (Nothing standing: then across what lies felled nearest, two fifths of the way
        // along it, to see a trunk cut into lengths.)
        // (At cut wood that is there, as a person aims, a different piece of it each blow:
        // aimed along where the tree had lain, the blows went on at the same two logs when
        // the rest had dropped or been broken away, and a great willow came to three hunks.)
        // (And wood that is in a cell as bits: a thin log at rest at its own angle is in
        // no whole cell, and a birch's logs were never split by a script.)
        let lying = || {
            let stump = chopping.map(|c| c.0);
            let (c, r) = ((at.translation / cube_voxel::CELL).floor().as_ivec3(), (16.0 / cube_voxel::CELL) as i32);
            let mut wood: Vec<IVec3> = (-16..=24).flat_map(|y| (-r..=r).flat_map(move |z| (-r..=r).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| { let cut = |b: cube_voxel::Block| b.wood() && !b.shaped(); match terrain.0.get(*cell) { cube_voxel::Block::Broken => terrain.0.bits_at(*cell).is_some_and(|bits| bits.iter().any(|b| cut(*b))), b => cut(b) } }).filter(|cell| stump.is_none_or(|s| ((cell.as_vec3() + 0.5) * cube_voxel::CELL).with_y(0.0).distance(s.with_y(0.0)) > 1.5)).collect();
            wood.sort_by_key(|c| (c.x, c.z, c.y));
            let pick = chopping.map_or(0, |c| c.1) as usize * 37;
            (!wood.is_empty()).then(|| (wood[pick % wood.len()].as_vec3() + 0.5) * cube_voxel::CELL)
        };
        if foot.is_none() && let Some(middle) = lying() {
            let bites = chopping.map_or(0, |c| c.1);
            *chopping = Some((chopping.map_or(middle, |c| c.0), bites + 1));
            aimed = Some((middle, Vec3::NEG_Y));
            (dig, tool) = (true, Some((cube_sim::items::Stuff::Wood, 0.7)));
        }
        let Some(foot) = foot.filter(|_| aimed.is_none()).or(aimed.map(|a| a.0)) else { return };
        if aimed.is_none() {
        let bites = chopping.map_or(0, |c| c.1);
        *chopping = Some((foot, bites + 1));
        let away = (foot - at.translation).with_y(0.0).normalize_or(Vec3::X);
        aimed = Some((foot - away * 0.3, away));
        (dig, tool) = (true, Some((cube_sim::items::Stuff::Wood, 0.8)));
        }
    }
    // A script's pick: a blow at the top of the nearest boulder.
    if pick && let Some(stone) = forms.0.values().flat_map(|f| f.stones.iter()).map(|s| (s.middle, s.radii)).chain(live.0.loose.things.iter().filter(|t| t.stone.as_ref().is_some_and(|s| t.blows < cube_sim::loose::CHIPS(s.radii.max_element()))).filter_map(|t| t.stone.as_ref().map(|s| (s.middle, s.radii)))).min_by(|a, b| a.0.distance(at.translation).total_cmp(&b.0.distance(at.translation))).filter(|s| s.0.distance(at.translation) < 14.0) {
        aimed = Some((stone.0 + Vec3::Y * (stone.1.y - 0.1 + HOLE * 0.3), Vec3::NEG_Y));
        (dig, tool) = (true, Some((cube_sim::items::Stuff::Stone, 0.6)));
    } else if pick && let Some(lump) = live.0.loose.lump(at.translation, 14.0) {
        // (No boulder near that is not yet broken: then at the nearest lump of rubble.)
        aimed = Some((lump + Vec3::Y * HOLE * 0.3, Vec3::NEG_Y));
        (dig, tool) = (true, Some((cube_sim::items::Stuff::Stone, 0.6)));
    }
    let Some(height) = live.kind(mover).map(|k| k.height) else { return };
    // Where the middle of the picture is: along the camera's own line, as far as he reaches
    // from his eyes. (From his eyes along the camera's way, it was beside the mark whenever
    // the camera stood to one side of him.)
    let (eyes, mut along) = (at.translation + Vec3::Y * height * 0.92, *camera.forward());
    let hit = match aimed {
        Some((at, away)) => {
            along = away;
            at - away * HOLE * 0.3
        }
        None => {
            let seen = terrain.0.raycast(camera.translation, along, REACH + camera.translation.distance(eyes)).map(|far| camera.translation + along * far).filter(|hit| hit.distance(eyes) <= REACH);
            match seen {
                Some(hit) => hit,
                // (A script has no eye to aim with, and where the ground falls away before
                // him the middle of the picture is out of his reach: then at the ground a
                // stride on. Until this a script's spade or pick dug only on a hillside it
                // happened to face, and the land's stone had never been dug in a trial.)
                None if controls.far => {
                    let over = eyes + along.with_y(0.0).normalize_or(Vec3::X) * 0.8;
                    let Some(far) = terrain.0.raycast(over, Vec3::NEG_Y, 4.0) else { return };
                    along = Vec3::NEG_Y;
                    over + Vec3::NEG_Y * far
                }
                None => return,
            }
        }
    };
    if aimed.is_none() && hit.distance(eyes) > REACH {
        return;
    }
    let far = hit.distance(eyes);
    if dig {
        // A cut (`cube_sim::loose`): a tree or a boulder it bites into is cells from then on
        // and no longer among the shapes drawn; what it leaves standing on nothing falls.
        let (stuff, bite) = (tool.map(|t| t.0), tool.map_or(HOLE, |t| t.1 / 2.0));
        let middle = hit + along * bite * 0.3;
        let here = crate::world::region_of((middle / cube_voxel::CELL).floor().as_ivec3());
        let near: Vec<(i32, i32)> = (-1..=1).flat_map(|z| (-1..=1).map(move |x| (here.0 + x, here.1 + z))).filter(|r| forms.0.contains_key(r)).collect();
        let cut = {
            let shapes: Vec<&cube_sim::plants::Form> = near.iter().map(|r| &*forms.0[r]).collect();
            // (An axe does not dig wood: its blow is the tree's to answer, `Loose::chop`.)
            // (Nor does a pick dig a boulder: it chips it until it breaks, `Loose::chip`. The
            // land's own stone it digs.)
            match stuff {
                Some(cube_sim::items::Stuff::Wood) => live.0.loose.chop(&mut terrain.0, middle, along, &shapes),
                Some(cube_sim::items::Stuff::Stone) => match live.0.loose.chip(&mut terrain.0, middle, along, &shapes) {
                    Some(cut) => cut,
                    None => live.0.loose.cut(&mut terrain.0, middle, bite, along, &shapes, stuff),
                },
                _ => live.0.loose.cut(&mut terrain.0, middle, bite, along, &shapes, stuff),
            }
        };
        for (n, region) in near.iter().enumerate() {
            let (trees, mut stones): (Vec<u32>, Vec<usize>) = (cut.trees.iter().filter(|t| t.0 == n).map(|t| t.1).collect(), cut.stones.iter().filter(|s| s.0 == n).map(|s| s.1).collect());
            if trees.is_empty() && stones.is_empty() {
                continue;
            }
            let Some(form) = forms.0.get_mut(region).map(std::sync::Arc::make_mut) else { continue };
            form.limbs.retain(|l| !trees.contains(&l.tree));
            form.clumps.retain(|c| !trees.contains(&c.tree));
            stones.sort_unstable_by(|a, b| b.cmp(a));
            for stone in stones {
                form.stones.remove(stone);
            }
            grown.again(*region);
        }
        // (A tree an axe is at is drawn alone from its first blow, with its notch: `loose::felled`.)
        for region in &near {
            let Some(form) = forms.0.get_mut(region) else { continue };
            let struck: std::collections::HashSet<u32> = form.limbs.iter().filter(|l| live.0.loose.things.iter().filter(|t| t.notch.is_some()).any(|t| t.limbs.iter().any(|m| m.a == l.a && m.b == l.b))).map(|l| l.tree).collect();
            if !struck.is_empty() {
                let form = std::sync::Arc::make_mut(form);
                form.limbs.retain(|l| !struck.contains(&l.tree));
                form.clumps.retain(|c| !struck.contains(&c.tree));
                grown.again(*region);
            }
        }
        // (What the ball itself reached is drawn again; what was cut away is his.)
        let reach = Vec3::splat(bite + cube_voxel::CELL);
        let (lo, hi) = (((middle - reach) / cube_voxel::CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)), ((middle + reach) / cube_voxel::CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        let reached = (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z))));
        terrain.touched(&mut dirty, cut.changed.into_iter().chain(reached));
        live.0.took(mover.id, cut.taken);
    } else if let Some(block) = lump.filter(|_| far > 1.0) {
        // Not on top of itself: a lump it stood inside would hold it fast. Into air only:
        // what is put down lies on what is there, not through it. And one fewer is carried.
        // (Stone and timber go in the cell on this side of what he looks at: half a cell back.)
        let middle = hit - along * if block == cube_voxel::Block::Dirt { LUMP * 0.3 } else { cube_voxel::CELL * 0.5 };
        // (Earth comes to rest where it falls, not where it was let go: what changed is the simulation's to say.)
        let changed = live.0.loose.put(&mut terrain.0, middle, LUMP, block);
        if std::env::var_os("LOOSE_TRACE").is_some() {
            println!("loose: put {block:?} at {middle:.2?}");
        }
        terrain.touched(&mut dirty, changed);
        live.0.placed(mover.id);
    }
}

/// The mouse belongs to the camera while a creature is controlled, and to the panel otherwise.
fn cursor(menus: Res<Menus>, controlled: Query<(), With<Controlled>>, mut windows: Query<&mut CursorOptions>) {
    let locked = !controlled.is_empty() && !menus.open();
    for mut options in &mut windows {
        let mode = if locked { CursorGrabMode::Locked } else { CursorGrabMode::None };
        if options.grab_mode != mode {
            options.grab_mode = mode;
            options.visible = !locked;
        }
    }
}

/// The torch the controlled creature carries: a warm light at its shoulder that throws
/// shadows and wavers as a flame does. T lights it and puts it out; `--torch` starts with it
/// lit. Night is dark on purpose (docs/art-guide.md) and this is how it is played.
#[derive(Component)]
pub struct Torch;

fn torch(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<crate::Args>,
    live: Res<Live>,
    loading: Res<crate::loading::Loading>,
    bearers: Query<(&Transform, &Mover), With<Controlled>>,
    flames: Query<&GlobalTransform, With<crate::held::Flame>>,
    mut torches: Query<(Entity, &mut Transform, &mut PointLight), (With<Torch>, Without<Controlled>)>,
    mut embers: Query<(Entity, &Ember, &mut Transform, &mut PointLight), (Without<Torch>, Without<Controlled>)>,
    mut cores: Query<(Entity, &mut Transform, &mut SpotLight), (With<Core>, Without<Torch>, Without<Ember>, Without<Controlled>)>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    roofed: Res<crate::world::Roofed>,
) {
    // Lit while the torch is the thing in hand (T takes it from the bar); `--torch` lights
    // it whatever is held, for pictures.
    let held = |m: &Mover| live.agent(m).is_some_and(|a| a.dead.is_none() && a.act != cube_sim::life::Act::Swim && a.carries.is_none() && a.pack.as_ref().and_then(|p| p.held()).is_some_and(|i| i.lights()));
    // (Not behind the loading screen: it would be heard to catch.)
    let Some((bearer, _)) = bearers.iter().next().filter(|(_, m)| !loading.up && (args.torch || held(m))) else {
        for entity in torches.iter().map(|t| t.0).chain(embers.iter().map(|e| e.0)).chain(cores.iter().map(|c| c.0)) {
            commands.entity(entity).despawn();
        }
        return;
    };
    // At the head of the torch in his hand; by the shoulder where no torch is drawn (`--torch`).
    let at = flames.iter().next().map_or(bearer.translation + bearer.rotation * Vec3::new(0.3, 1.55, -0.2), |f| f.translation() + Vec3::Y * 0.08);
    let t = time.elapsed_secs();
    let waver = 1.0 + 0.1 * (t * 11.0).sin() + 0.06 * (t * 23.0 + 1.3).sin() + 0.04 * (t * 5.3).sin();
    // A flame is as strong at dusk as at midnight, and the eye is shut ten stops further: by
    // its true strength it lit nothing until night proper, not the hand that held it. It is
    // given three quarters of what the eye has shut, so that at sunset it warms who holds it
    // and the ground at his feet, and by day (where it is given no more than at sunset) it
    // is a flame and no light.
    let waver = waver * 2f32.powf(SEEN * crate::world::shut(live.0.when, roofed.0).min(11.0));
    let stir = Vec3::new((t * 7.0).sin(), (t * 9.0).cos(), (t * 6.0).sin()) * 0.015;
    // The light that lights the way stands over the flame, not in it (see `LIFT`).
    let over = at + Vec3::Y * LIFT + stir;
    let amber = Color::srgb(1.0, 0.5, 0.19);
    let glow = |shadows: bool, lumens: f32, range: f32| PointLight { color: amber, intensity: lumens, range, radius: 0.05, shadow_maps_enabled: shadows, ..default() };
    if let Ok((_, mut place, mut light)) = torches.single_mut() {
        place.translation = over;
        light.intensity = TORCH * waver;
    } else {
        commands.spawn((Torch, glow(true, TORCH, 16.0), Transform::from_translation(over)));
    }
    // The hot middle of the pool is thrown down ahead of him, where the torch is held out
    // and he is going, and misses him: a paler light in a cone from the same place.
    let feet = Vec3::new(at.x, bearer.translation.y, at.z);
    // (Ahead is the way the torch is held out from him: no need to know which way a body faces.)
    let ahead = (feet - bearer.translation).with_y(0.0).normalize_or(bearer.rotation * Vec3::Z);
    let thrown = Transform::from_translation(over).looking_at(feet + ahead * 1.7, Vec3::Y);
    if let Ok((_, mut place, mut light)) = cores.single_mut() {
        *place = thrown;
        light.intensity = CORE * waver;
    } else {
        commands.spawn((Core, SpotLight { color: Color::srgb(1.0, 0.58, 0.26), intensity: CORE, range: 9.0, radius: 0.05, inner_angle: 0.2, outer_angle: 0.62, shadow_maps_enabled: false, ..default() }, thrown));
    }
    // What comes back off the ground toward the eye: from between him and the camera.
    let eye = cameras.iter().next().map_or(bearer.translation, |c| c.translation());
    let back = (eye - bearer.translation).with_y(0.0);
    let bounce = bearer.translation + Vec3::Y * 1.2 + back.clamp_length_max(2.2) * 0.8;
    if embers.is_empty() {
        commands.spawn((Ember::Fill, glow(false, FILL, 16.0), Transform::from_translation(over)));
        commands.spawn((Ember::Flame, glow(false, EMBER, 2.5), Transform::from_translation(at)));
        commands.spawn((Ember::Bounce, glow(false, BOUNCE, 8.0), Transform::from_translation(bounce)));
    }
    for (_, ember, mut place, mut light) in &mut embers {
        (place.translation, light.intensity) = match ember {
            Ember::Fill => (over, FILL * waver),
            Ember::Flame => (at + stir, EMBER * waver),
            Ember::Bounce => (bounce, BOUNCE * waver),
        };
    }
}

/// How bright the torch is, lumens. Set for the camera as it is at night; by day it is lost
/// in the sun, as a real one is.
/// How much of the stops the eye has shut since night a flame's light is given back.
const SEEN: f32 = 0.75;
const TORCH: f32 = 2600.0;
/// How far over the flame that light stands, metres. In the flame itself, a hand from the
/// bearer's face and a man's height from the ground, it gave his face thirty times the light
/// it gave the ground at his feet (by the square of how far): he was a white figure in a dim
/// pool, and every blade of grass threw a shadow many times its own length. From overhead
/// the ground and the bearer are lit alike, as a camera that could see both would have it.
const LIFT: f32 = 1.3;
/// The hot middle of the pool, lumens: in a cone, so the pool goes from pale and bright
/// where the torch is held out to deep amber at its edge. One light alone was one hue from
/// his feet to the dark. (It is nearly the fire's own colour and pale by being bright: made
/// of a yellower light it was khaki on grass.)
const CORE: f32 = 5200.0;
/// The part of it that throws no shadow, lumens: what a fire's light does by coming off
/// everything it falls on. Without it a shadow from the torch was a black hole in the
/// ground. (It is also seen faintly through a trunk or a wall.)
const FILL: f32 = 300.0;
/// The same from the side the eye is on, lumens: the near side of a trunk between the
/// flame and the eye faces no part of the fire, and was black. (Tried as a warm fill over
/// the whole night: the far woods went olive and there was no blue left for the fire to
/// be warm against.)
const BOUNCE: f32 = 260.0;
/// The flame's own light on what is right beside it (his hand, the side of his face, the
/// torch's head), lumens: a little, with no shadows, in the flame.
const EMBER: f32 = 20.0;

/// The torch's lights that throw no shadow (`FILL`, `EMBER`, `BOUNCE`).
#[derive(Component)]
enum Ember {
    Fill,
    Flame,
    Bounce,
}
/// And its cone (`CORE`).
#[derive(Component)]
struct Core;

/// An arrow in flight: which way, and how long it has flown. It is only to be seen: what
/// the shot hit was decided by the simulation when the bow was loosed.
#[derive(Component)]
struct Arrow(Vec3, f32);

fn arrows(mut commands: Commands, time: Res<Time>, mut controls: ResMut<Controls>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut flying: Query<(Entity, &mut Arrow, &mut Transform)>, mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>) {
    if let Some((from, way)) = controls.shot.take() {
        let (mesh, material) = look.get_or_insert_with(|| (meshes.add(Cuboid::new(0.025, 0.025, 0.8)), materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.42, 0.25), perceptual_roughness: 0.8, ..default() }))).clone();
        // Loosed a third of a second on, when the simulation lands the shot.
        commands.spawn((Arrow(way, -0.3), Mesh3d(mesh), MeshMaterial3d(material), Transform::from_translation(from + way * 0.6).looking_to(way, Vec3::Y), Visibility::Hidden));
    }
    for (entity, mut arrow, mut at) in &mut flying {
        arrow.1 += time.delta_secs();
        if arrow.1 > 0.0 {
            commands.entity(entity).insert(Visibility::Inherited);
            // Sixty metres a second, dropping a little.
            at.translation += (arrow.0 * 60.0 + Vec3::NEG_Y * arrow.1 * 6.0) * time.delta_secs();
        }
        if arrow.1 > 0.55 {
            commands.entity(entity).despawn();
        }
    }
}

/// Where the player's state is kept between runs: beside where the game is run from.
fn save_file() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join("terra-cube.save")
}

/// F5 writes the player's state (where, when, health, pack) and F9 reads it back; `--continue`
/// reads it at the start. The land is the seed's and is not written: what was dug is lost.
fn saves(args: Res<Args>, mut controls: ResMut<Controls>, mut live: ResMut<Live>, controlled: Query<&Mover, With<Controlled>>, mut began: Local<bool>) {
    let Ok(mover) = controlled.single() else { return };
    let first = !std::mem::replace(&mut *began, true) && args.resume;
    if std::mem::take(&mut controls.save) {
        match std::fs::write(save_file(), live.0.save(mover.id)) {
            Ok(()) => println!("saved {}", save_file().display()),
            Err(e) => warn!("could not save: {e}"),
        }
    }
    if std::mem::take(&mut controls.load) || first {
        match std::fs::read_to_string(save_file()) {
            Ok(text) => println!("loaded {} ({})", save_file().display(), if live.0.load(mover.id, &text) { "ok" } else { "not understood" }),
            Err(e) => warn!("no save to load: {e}"),
        }
    }
}
