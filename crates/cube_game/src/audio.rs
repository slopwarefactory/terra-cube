//! What is heard. Every sound is a file in `assets/audio` (CC0; its README says where each is
//! from), found by name: `wolf` is `wolf_1.ogg`, `wolf_2.ogg`, and one of them is picked.
//!
//! Nothing here decides what happens, only what it sounds like. The air of a place is what
//! the ecology says it is (`ecology::ambience`): quiet beds of wind, the night's insects and
//! water, each several recordings faded one into another at random so none is heard to come
//! round, and over them the small sounds one at a time from somewhere about: a bird's phrase,
//! a far crow, leaves in a gust, a grasshopper, a frog, an owl.
//! A creature is heard when the simulation says it does something: a blow, a wound, a death,
//! and now and then, at the hours it is about, its call. The player is heard walking on what
//! is underfoot. Every sound played is also written to the trace (`sound NAME`, and each
//! second `bed NAME` for what plays on), with how loud it reached the ear, so a scripted run
//! can be judged for what was heard without anyone listening (`tools/audio/mix.py`).

use crate::{
    motion::{Live, Mover},
    player::{Controlled, Menus, Torch},
    world::Terrain,
};
use bevy::{audio::{DefaultSpatialScale, SpatialScale, Volume}, prelude::*};
use cube_sim::{
    ecology,
    life::{Act, Temper},
};
use std::collections::HashMap;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DefaultSpatialScale(SpatialScale::new(1.0 / SCALE)))
            .init_resource::<Bank>()
            .add_systems(Startup, load)
            .add_systems(Update, (ears, air, creatures, footsteps, hands, small).chain());
    }
}

/// How loud everything is, of 1: `--quiet` makes it 0.
const MASTER: f32 = 0.8;
/// How far off a creature is still heard, metres.
const HEARD: f32 = 45.0;
/// Metres to one unit of the engine's hearing: a sound is at its full loudness within this,
/// and falls with the square of the distance beyond.
const SCALE: f32 = 5.0;
/// The small sounds of the air are not left to that fall, which has a bird at fifty metres
/// unheard: each is at its full loudness within this many metres and falls with the distance,
/// not its square, beyond (half as loud at twice as far).
const NEAR: f32 = 12.0;

/// Every sound, by name, and what has been asked to play this frame.
#[derive(Resource, Default)]
pub struct Bank {
    sets: HashMap<String, Vec<Handle<AudioSource>>>,
    /// Which of each set was played last: never the same file twice running.
    last: HashMap<String, usize>,
    turn: u32,
    loud: f32,
    said: bool,
    /// Where the ears are, and the time, for the trace.
    ear: Vec3,
    clock: f32,
    /// How loud each kind of sound is, of 1, set by ear in the admin panel: the air of the
    /// place (wind, insects, water, and the small sounds over them), creatures' voices, and
    /// footfalls. Nobody who wrote this could listen to it; the developer could, and found
    /// all three too much, and then the air still too much.
    pub mix: Mix,
}

#[derive(Clone, Copy)]
pub struct Mix {
    pub air: f32,
    pub voices: f32,
    pub steps: f32,
}

impl Default for Mix {
    fn default() -> Self {
        // The air at 0.3: with the beds' own weights (`air`) a noon meadow's wind reaches
        // the ear near -56 dB, some 18 dB under the walker's own footfall, and a bird's call
        // from 25 m near -47 dB. `tools/audio/mix.py` measures it.
        Mix { air: 0.3, voices: 0.6, steps: 0.4 }
    }
}

/// What a sound is mixed as.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Air,
    Voice,
    Step,
    Hand,
}

impl Bank {
    fn roll(&mut self) -> f32 {
        self.turn ^= self.turn << 13;
        self.turn ^= self.turn >> 17;
        self.turn ^= self.turn << 5;
        (self.turn & 0xFFFF) as f32 / 65535.0
    }

    /// One of a set, and never the one it gave last time if there is another.
    fn pick(&mut self, name: &str) -> Option<(usize, Handle<AudioSource>)> {
        let count = self.sets.get(name).map(Vec::len).filter(|n| *n > 0)?;
        let mut pick = (self.roll() * count as f32) as usize % count;
        if let Some(last) = self.last.get(name).copied().filter(|_| count > 1) {
            pick = (self.roll() * (count - 1) as f32) as usize % (count - 1);
            if pick >= last {
                pick += 1;
            }
        }
        self.last.insert(name.to_owned(), pick);
        Some((pick, self.sets[name][pick].clone()))
    }

    /// One of a set, at a place in the world (heard from where it is) or in the head (None).
    pub fn play(&mut self, commands: &mut Commands, name: &str, at: Option<Vec3>, volume: f32) {
        self.pitched(commands, name, at, volume, 1.0);
    }

    /// Where a big sound is played from: on the way to where it was made, four tenths as far.
    /// A tree's top strikes the ground ten metres from whoever felled it, and by the
    /// engine's fall with distance its crash was a third as loud as the axe had been: "it
    /// doesnt have any ground contact noises", the developer, 2026-11-14.
    pub fn carried(&self, at: Vec3) -> Vec3 {
        self.ear + (at - self.ear) * 0.4
    }

    /// The same, played higher or lower: `pitch` times as fast.
    pub fn pitched(&mut self, commands: &mut Commands, name: &str, at: Option<Vec3>, volume: f32, pitch: f32) {
        let kind = if name.starts_with("step_") || name == "splash" || name == "swim" {
            Kind::Step
        } else if ["swing", "hit", "hurt", "torch_ignite", "chop", "split", "dig", "pick", "crash", "thud", "rubble", "crack"].contains(&name) || name.starts_with("ui_") {
            Kind::Hand
        } else {
            Kind::Voice
        };
        self.emit(commands, name, at, volume, pitch, kind);
    }

    /// One of the small sounds of the air, from a place: heard from that side, and fainter
    /// with distance by [`NEAR`]'s rule.
    fn far(&mut self, commands: &mut Commands, name: &str, from: Vec3, volume: f32, pitch: f32) {
        let away = from.distance(self.ear);
        let at = self.ear + (from - self.ear).normalize_or(Vec3::Y) * (SCALE * 0.8);
        self.emit(commands, name, Some(at), volume * NEAR / away.max(NEAR), pitch, Kind::Air);
    }

    fn emit(&mut self, commands: &mut Commands, name: &str, at: Option<Vec3>, volume: f32, pitch: f32, kind: Kind) {
        let Some((pick, handle)) = self.pick(name) else { return };
        let (mix, word) = match kind {
            Kind::Air => (self.mix.air, "air"),
            Kind::Voice => (self.mix.voices, "voice"),
            Kind::Step => (self.mix.steps, "step"),
            Kind::Hand => (1.0, "hand"),
        };
        let gain = volume * mix * self.loud;
        if self.said {
            // What reaches the ear: the engine's fall with distance is in `gain`.
            let fall = at.map_or(1.0, |p| (SCALE / p.distance(self.ear).max(0.01)).powi(2).min(1.0));
            println!("sound {name}_{} vol={volume:.2} gain={:.4} t={:.1} kind={word}{}", pick + 1, gain * fall, self.clock, at.map_or(String::new(), |p| format!(" at {:.0},{:.0},{:.0}", p.x, p.y, p.z)));
        }
        // A little off its pitch each time, so the same file is not the same sound; further
        // off where the set is one file and it is all there is to tell two plays apart.
        let off = if self.sets[name].len() == 1 { 0.12 } else { 0.07 };
        let settings = PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain)).with_speed((1.0 - off + self.roll() * 2.0 * off) * pitch);
        match at {
            Some(at) => commands.spawn((AudioPlayer::new(handle), settings.with_spatial(true), Transform::from_translation(at))),
            None => commands.spawn((AudioPlayer::new(handle), settings)),
        };
    }
}

fn load(args: Res<crate::Args>, assets: Res<AssetServer>, mut bank: ResMut<Bank>) {
    bank.turn = 0x51ED_270B;
    bank.loud = if args.quiet { 0.0 } else { MASTER };
    bank.said = args.trace || args.scene.is_some() && args.play;
    let dir = std::path::Path::new(&crate::asset_root()).join("audio");
    let mut names: Vec<String> = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter_map(|f| f.file_name().to_str().and_then(|n| n.strip_suffix(".ogg")).map(str::to_owned)).collect();
    names.sort();
    for name in names {
        // `wolf_2` is of the set `wolf`; `ui_open` is its own.
        let set = name.rsplit_once('_').filter(|(_, n)| n.parse::<u32>().is_ok()).map_or(name.as_str(), |(set, _)| set).to_owned();
        bank.sets.entry(set).or_default().push(assets.load(format!("audio/{name}.ogg")));
    }
}

/// The camera is the ears.
fn ears(mut commands: Commands, time: Res<Time>, mut bank: ResMut<Bank>, cameras: Query<Entity, (With<Camera3d>, Without<SpatialListener>)>, heads: Query<&GlobalTransform, With<Camera3d>>) {
    for camera in &cameras {
        commands.entity(camera).insert(SpatialListener::new(0.2));
    }
    bank.clock = time.elapsed_secs();
    if let Some(head) = heads.iter().next() {
        bank.ear = head.translation();
    }
}

/// The beds: sounds that go on, each a set of recordings (`amb_wind_1`, `amb_wind_2`, ...).
const BEDS: [&str; 5] = ["amb_wind", "amb_night", "amb_water", "torch_loop", "amb_rain"];
/// Seconds one recording of a bed takes to give way to the next.
const FADE: f32 = 7.0;

/// One recording of a bed, playing: which bed, how far it has come in (0 to 1), and whether
/// it is on its way out.
#[derive(Component)]
struct Bed {
    of: usize,
    file: usize,
    fade: f32,
    out: bool,
}

/// How loud each bed is now, when each next changes its recording, and when the trace last
/// said so.
#[derive(Default)]
struct Beds {
    now: [f32; 5],
    change: [f32; 5],
    said: f32,
}

/// The air of the place the camera is in, as the ecology gives it. Each bed is quiet, and is
/// never one file going round: every half minute or so another recording of the set comes in
/// under the one playing, at a speed a little its own, and that one goes out. A bed that is
/// not heard here is not played at all.
fn air(mut commands: Commands, time: Res<Time>, mut bank: ResMut<Bank>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, cameras: Query<&GlobalTransform, With<Camera3d>>, torches: Query<(), With<Torch>>, breeze: Res<crate::world::Breeze>, mut beds: Query<(Entity, &mut Bed, Option<&mut AudioSink>)>, mut state: Local<Beds>) {
    let (Some(live), Some(terrain), Some(eye)) = (live, terrain, cameras.iter().next().map(|c| c.translation())) else { return };
    let a = ecology::ambience(&terrain.2.place(&terrain.1, eye.x, eye.z), live.0.when);
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    // Of a file levelled to -27 dB: with the air's 0.3 and the master's 0.8, the wind of an
    // open meadow lies between -62 dB in a lull and -53 in a gust, a summer night's bed of
    // insects at -56 (the crickets heard one by one are over it), a river at its bank at -53. A footfall is near -38.
    // (Rain is over the wind and under a footfall: a downpour some 6 dB over a gust. Snow
    // falls without a sound.)
    let (rain, snow) = crate::world::falling(live.0.when);
    let want = [a.wind * 0.3 * (0.2 + 1.1 * breeze.0).min(1.0), a.night * 0.14 * (1.0 - 0.8 * rain), a.water * 0.22, if torches.is_empty() { 0.0 } else { 0.25 }, if snow { 0.0 } else { rain * 0.55 }];
    let ease = 1.0 - (-1.5 * dt).exp();
    let say = bank.said && t >= state.said;
    if say {
        state.said = t + 1.0;
    }
    for (i, name) in BEDS.into_iter().enumerate() {
        state.now[i] += (want[i] - state.now[i]) * ease;
        let heard = want[i] > 0.0 || state.now[i] > 0.002;
        let playing = beds.iter().any(|(_, bed, _)| bed.of == i && !bed.out);
        let next = heard && (!playing || t >= state.change[i]);
        let mix = if name == "torch_loop" { 1.0 } else { bank.mix.air };
        for (entity, mut bed, sink) in &mut beds {
            if bed.of != i {
                continue;
            }
            bed.out |= next || !heard;
            bed.fade = (bed.fade + if bed.out { -dt } else { dt } / FADE).clamp(0.0, 1.0);
            if bed.out && bed.fade <= 0.0 {
                commands.entity(entity).despawn();
                continue;
            }
            // Two recordings crossing keep one loudness between them.
            let gain = state.now[i] * (bed.fade * std::f32::consts::FRAC_PI_2).sin() * mix * bank.loud;
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(gain));
            }
            if say && gain > 0.0 {
                println!("bed {name}_{} gain={gain:.4} t={t:.1}", bed.file + 1);
            }
        }
        if next && let Some((file, handle)) = bank.pick(name) {
            state.change[i] = t + 24.0 + bank.roll() * 30.0;
            // The first of a bed is there at once; the rest come in under it.
            let fade = if playing { 0.0 } else { 0.6 };
            commands.spawn((Bed { of: i, file, fade, out: false }, AudioPlayer::new(handle), PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)).with_speed(0.94 + bank.roll() * 0.12)));
        }
    }
}

/// What a creature was last heard doing, and when it may next call.
#[derive(Default)]
struct Heard {
    act: Option<Act>,
    calls: f32,
}

/// Creatures near the camera are heard when what they are doing changes, and call now and
/// then at the hours they are about.
fn creatures(mut commands: Commands, time: Res<Time>, mut bank: ResMut<Bank>, live: Option<Res<Live>>, cameras: Query<&GlobalTransform, With<Camera3d>>, movers: Query<&Mover>, mut heard: Local<HashMap<u32, Heard>>, mut spoke: Local<(String, f32, HashMap<String, f32>)>) {
    let (Some(live), Some(eye)) = (live, cameras.iter().next().map(|c| c.translation())) else { return };
    let t = time.elapsed_secs();
    for mover in &movers {
        let Some(agent) = live.agent(mover) else { continue };
        let kind = &live.0.kinds[agent.kind];
        let at = agent.body.pos + Vec3::Y * kind.height * 0.6;
        if at.distance(eye) > HEARD {
            heard.remove(&mover.id);
            continue;
        }
        let person = kind.temper == Temper::Person;
        let voice = kind.voice.as_ref().map(|v| v.name().to_owned());
        // Several kinds share one recording (a fox and a lynx the wolf's, a moose the deer's,
        // an ogre the troll's): each is played at the pitch of its own size, against the size
        // of the animal recorded, since a larger throat is a lower voice. So no two kinds
        // sound the same, and none sounds like an animal three times its weight.
        let pitch = voice.as_deref().map_or(1.0, |v| {
            let recorded = match v {
                "wolf" => 0.85,
                "boar" | "bear" => 1.3,
                "deer" => 1.9,
                "troll" => 3.0,
                "rabbit" => 0.3,
                "frog" => 0.15,
                // (A common toad, a hand long: the giant one is given the lowest it may have.)
                "toad" => 0.1,
                "goat" => 1.1,
                "horse" => 1.6,
                "goblin" => 1.2,
                "golem" => 3.6,
                "crow" => 0.35,
                _ => kind.height,
            };
            (recorded / kind.height.max(0.05)).powf(0.35).clamp(0.7, 1.5)
        });
        // A big thing is louder.
        let loud = (0.5 + kind.height * 0.35).min(1.6);
        let first = !heard.contains_key(&mover.id);
        let mine = heard.entry(mover.id).or_default();
        if first {
            mine.calls = t + 6.0 + bank.roll() * 80.0;
        }
        if mine.act != Some(agent.act) {
            match agent.act {
                Act::Attack if person => bank.play(&mut commands, "swing", Some(at), 0.7),
                Act::Attack => {
                    if let Some(voice) = &voice {
                        bank.pitched(&mut commands, voice, Some(at), loud, pitch);
                    }
                }
                Act::Hurt => {
                    bank.play(&mut commands, "hit", Some(at), 0.9);
                    if person {
                        bank.play(&mut commands, "hurt", Some(at), 0.8);
                    }
                }
                Act::Die => bank.play(&mut commands, if person { "hurt" } else if kind.height > 1.0 { "monster_die" } else { voice.as_deref().unwrap_or("hit") }, Some(at), loud),
                _ => {}
            }
            mine.act = Some(agent.act);
        }
        // Its call: seldom, and only at the hours it is about.
        if t >= mine.calls && agent.dead.is_none() {
            mine.calls = t + 35.0 + bank.roll() * 80.0;
            let about = kind.niche().about(live.0.when);
            if let Some(voice) = voice.filter(|_| !person && about > 0.4 && matches!(agent.act, Act::Idle | Act::Walk | Act::Eat)) {
                // A herd is many animals with one voice, and each calling in its own time
                // was one recording over and over (a horse's, seventeen times in five
                // minutes). So: no call within six seconds of another's; a kind is silent
                // for a minute after one of it has called; and the kind that called last
                // waits until another kind has, or three minutes.
                let own = spoke.2.get(&voice).copied().unwrap_or(-1000.0);
                if t >= spoke.1 + 6.0 && t >= own + 60.0 && (spoke.0 != voice || t >= spoke.1 + 180.0) {
                    bank.pitched(&mut commands, &voice, Some(at), loud * 0.7, pitch);
                    spoke.2.insert(voice.clone(), t);
                    (spoke.0, spoke.1) = (voice, t);
                }
            }
        }
    }
}

/// The player's feet: a step for each stride on the ground, by what is underfoot; a splash
/// going into water and a stroke for each while swimming.
fn footsteps(mut commands: Commands, mut bank: ResMut<Bank>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, controlled: Query<&Mover, With<Controlled>>, mut gone: Local<(f32, Vec3, bool)>) {
    let (Some(live), Some(terrain), Ok(mover)) = (live, terrain, controlled.single()) else { return };
    let Some(agent) = live.agent(mover) else { return };
    let (at, body) = (agent.body.pos, &agent.body);
    let moved = Vec2::new(at.x - gone.1.x, at.z - gone.1.z).length();
    gone.1 = at;
    if moved > 2.0 || agent.dead.is_some() {
        return;
    }
    if body.swimming != gone.2 {
        gone.2 = body.swimming;
        if body.swimming {
            bank.play(&mut commands, "splash", Some(at), 0.8);
        }
    }
    gone.0 += moved;
    let speed = body.ground_speed();
    if body.swimming {
        if gone.0 > 1.6 {
            gone.0 = 0.0;
            bank.play(&mut commands, "swim", Some(at), 0.5);
        }
    } else if body.grounded && gone.0 > (0.75 + speed * 0.14).min(1.6) {
        gone.0 = 0.0;
        let under = terrain.0.get((at / cube_voxel::CELL).floor().as_ivec3() - IVec3::Y);
        use cube_voxel::Block;
        let stone = matches!(under, Block::Rock | Block::Stone | Block::Slate | Block::Chalk | Block::Gravel | Block::Broken);
        let wet = terrain.2.place(&terrain.1, at.x, at.z).depth > 0.0;
        bank.play(&mut commands, if wet { "splash" } else if stone { "step_stone" } else { "step_grass" }, Some(at), if wet { 0.35 } else { 0.35 + speed * 0.05 });
    }
}

/// The hands: a click when another thing is taken in hand, the pack opening and shutting,
/// the torch catching.
fn hands(mut commands: Commands, mut bank: ResMut<Bank>, menus: Res<Menus>, live: Option<Res<Live>>, controlled: Query<&Mover, With<Controlled>>, torches: Query<(), With<Torch>>, mut was: Local<(Option<usize>, bool, bool)>) {
    let held = live.and_then(|l| controlled.single().ok().and_then(|m| l.agent(m).and_then(|a| a.pack.as_ref().map(|p| p.hand))));
    if held != was.0 {
        if was.0.is_some() && held.is_some() {
            bank.play(&mut commands, "ui_select", None, 0.5);
        }
        was.0 = held;
    }
    if menus.open() != was.1 {
        was.1 = menus.open();
        bank.play(&mut commands, if was.1 { "ui_open" } else { "ui_close" }, None, 0.5);
    }
    let lit = !torches.is_empty();
    if lit != was.2 {
        was.2 = lit;
        if lit {
            bank.play(&mut commands, "torch_ignite", None, 0.6);
        }
    }
}

/// Which small bird sings where: the name of its set, and how wooded it likes it (0 open
/// ground, 1 deep wood). A set with no files is not sung.
const SINGERS: [(&str, f32); 10] = [
    ("song_lark", 0.0),
    ("song_sparrow", 0.1),
    ("song_robin", 0.4),
    ("song_merle", 0.4),
    ("song_wren", 0.6),
    ("song_tit", 0.6),
    ("song_chickadee", 0.7),
    ("song_warbler", 0.8),
    ("song_nuthatch", 0.9),
    ("song_thrush", 1.0),
];

/// A bird part way through its phrase: a few calls from one perch.
struct Phrase {
    set: &'static str,
    from: Vec3,
    left: u32,
    next: f32,
    pitch: f32,
    loud: f32,
}

/// One cricket of the night: where it sits, how fast it chirps and how high, and the bout it
/// is in (it chirps for a while, then rests).
#[derive(Clone, Copy, Default)]
struct Cricket {
    at: Vec3,
    every: f32,
    pitch: f32,
    next: f32,
    rests: f32,
}

/// When each of the small sounds is next due.
#[derive(Default)]
struct Small {
    crickets: [Cricket; 5],
    begun: bool,
    phrase: Option<Phrase>,
    song: f32,
    far: f32,
    hawk: f32,
    owl: f32,
    insect: f32,
    buzz: f32,
    frog: f32,
    rustle: f32,
    wind: f32,
}

/// Somewhere about the ears: any bearing, `near` to `far` metres off (more often near), and
/// `low` to `high` metres over them.
fn about(bank: &mut Bank, near: f32, far: f32, low: f32, high: f32) -> Vec3 {
    let (turn, out) = (bank.roll() * std::f32::consts::TAU, bank.roll());
    bank.ear + Vec3::new(turn.cos(), 0.0, turn.sin()) * (near + (far - near) * out * out) + Vec3::Y * (low + (high - low) * bank.roll())
}

/// The small sounds of a place, each seldom and from somewhere: what makes still air a place.
/// A songbird gives a phrase of a few calls from one perch (the perch of a bird that is out,
/// when one is near, so the one seen is the one heard), of a kind that suits how wooded it
/// is; a crow, a woodpecker or a dove sounds from far off; the hawks overhead call; leaves
/// stir when the wind gusts; grasshoppers and bees on a summer's day in the open; frogs by
/// water from dusk; an owl in the wood at night, and the crickets of a warm one. How often is the ecology's (`ambience`):
/// nothing here knows the place or the hour.
fn small(mut commands: Commands, time: Res<Time>, mut bank: ResMut<Bank>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, singers: Query<(&crate::air::Sings, &Transform)>, breeze: Res<crate::world::Breeze>, mut due: Local<Small>) {
    let (Some(live), Some(terrain)) = (live, terrain) else { return };
    let (t, eye) = (time.elapsed_secs(), bank.ear);
    let place = terrain.2.place(&terrain.1, eye.x, eye.z);
    let a = ecology::ambience(&place, live.0.when);
    if !due.begun {
        // Not all at once when the world starts.
        *due = Small { begun: true, crickets: default(), phrase: None, song: t + 2.0 + bank.roll() * 8.0, far: t + 10.0 + bank.roll() * 50.0, hawk: t + 15.0 + bank.roll() * 60.0, owl: t + 8.0 + bank.roll() * 40.0, insect: t + 3.0 + bank.roll() * 10.0, buzz: t + 5.0 + bank.roll() * 20.0, frog: t + 3.0 + bank.roll() * 10.0, rustle: t + 2.0, wind: 0.0 };
    }
    // One of the creatures of the air that is out (one put away for the night or the winter
    // is not in the world to sing), of a kind, within so many metres.
    let out = |bank: &mut Bank, kind: &str, within: f32| {
        let near: Vec<Vec3> = singers.iter().filter(|(sings, at)| sings.0 == kind && at.translation.y > -100.0 && at.translation.distance(eye) < within).map(|(_, at)| at.translation).collect();
        (!near.is_empty()).then(|| near[(bank.roll() * near.len() as f32) as usize % near.len()])
    };

    // Songbirds.
    if a.birds > 0.03 && due.phrase.is_none() && t >= due.song {
        let wooded = (place.cover / 0.6).min(1.0);
        let weights = SINGERS.map(|(set, likes)| if bank.sets.contains_key(set) { (1.0 - (wooded - likes).abs() * 1.6).max(0.06) } else { 0.0 });
        let mut pick = bank.roll() * weights.iter().sum::<f32>();
        let set = SINGERS.iter().zip(weights).find(|(_, w)| { pick -= w; pick <= 0.0 && *w > 0.0 }).map(|(s, _)| s.0);
        // Never from nearer than eighteen metres, however near the bird: a call from a bird
        // at arm's length was a call in the ear.
        let seen = out(&mut bank, "bird", 70.0).filter(|_| bank.turn & 1 == 0).map(|at| eye + (at - eye).normalize_or(Vec3::Y) * at.distance(eye).max(18.0));
        let from = seen.unwrap_or_else(|| about(&mut bank, 18.0, 60.0, 1.0, 9.0));
        if let Some(set) = set {
            due.phrase = Some(Phrase { set, from, left: 1 + (bank.roll() * 3.6) as u32, next: t, pitch: 0.96 + bank.roll() * 0.08, loud: 0.6 + bank.roll() * 0.4 });
        }
        due.song = t + (5.0 + bank.roll() * 12.0) / a.birds.max(0.2);
    }
    if let Some(mut phrase) = due.phrase.take() {
        if t >= phrase.next {
            bank.far(&mut commands, phrase.set, phrase.from, 0.3 * phrase.loud, phrase.pitch);
            phrase.left -= 1;
            phrase.next = t + 2.2 + bank.roll() * 2.6;
        }
        due.phrase = (phrase.left > 0 && a.birds > 0.03).then_some(phrase);
    }
    // A crow, a woodpecker, a dove: single, far, by day.
    if t >= due.far {
        due.far = t + 40.0 + bank.roll() * 90.0;
        let sets: Vec<&str> = ["far_crow", "far_drum", "far_dove"].into_iter().filter(|set| bank.sets.contains_key(*set)).collect();
        if a.birds > 0.1 && !sets.is_empty() {
            let which = sets[(bank.roll() * sets.len() as f32) as usize % sets.len()];
            let from = about(&mut bank, 45.0, 95.0, 2.0, 15.0);
            bank.far(&mut commands, which, from, 0.6, 1.0);
        }
    }
    if t >= due.hawk {
        due.hawk = t + 50.0 + bank.roll() * 110.0;
        if let Some(at) = out(&mut bank, "hawk", 200.0) {
            // (At 0.55 its loudest cry, alone in a quiet meadow, was within a tenth of a
            // decibel of the bar on a sound over a footfall, and over it one run in a few.)
            bank.far(&mut commands, "hawk", at, 0.45, 1.0);
        }
    }
    if t >= due.owl {
        due.owl = t + 45.0 + bank.roll() * 100.0;
        if a.owls > 0.3 {
            let from = about(&mut bank, 35.0, 90.0, 3.0, 12.0);
            bank.far(&mut commands, "owl", from, 0.6, 1.0);
        }
    }
    // The insects of a summer's day, low in the grass; and a dragonfly that is near.
    if t >= due.insect {
        due.insect = t + (9.0 + bank.roll() * 22.0) / a.insects.max(0.15);
        if a.insects > 0.05 {
            let from = about(&mut bank, 3.0, 16.0, -1.6, 0.3);
            bank.far(&mut commands, "insect", from, 0.3, 1.0);
        }
    }
    if t >= due.buzz {
        due.buzz = t + 12.0 + bank.roll() * 25.0;
        if let Some(at) = out(&mut bank, "buzz", 18.0) {
            bank.far(&mut commands, "buzz", at, 0.2, 1.0);
        }
    }
    if t >= due.frog {
        due.frog = t + (4.0 + bank.roll() * 12.0) / a.frogs.max(0.15);
        if a.frogs > 0.05 {
            let from = about(&mut bank, 12.0, 40.0, -1.6, -0.5);
            let pitch = 0.9 + bank.roll() * 0.2;
            bank.far(&mut commands, "frog", from, 0.2, pitch);
        }
    }
    // The night's crickets, one by one: each sits somewhere near and keeps its own time, so
    // the night is not one recording going round. As many as the night is warm.
    let many = (a.night * due.crickets.len() as f32).ceil() as usize;
    for cricket in due.crickets.iter_mut().take(many) {
        if cricket.every == 0.0 || cricket.at.distance(eye) > 45.0 {
            *cricket = Cricket { at: about(&mut bank, 5.0, 32.0, -1.7, -0.9), every: 0.75 + bank.roll() * 0.7, pitch: 0.9 + bank.roll() * 0.2, next: t + bank.roll() * 6.0, rests: t + 8.0 + bank.roll() * 20.0 };
        }
        if t >= cricket.next {
            if t >= cricket.rests {
                cricket.next = t + 4.0 + bank.roll() * 16.0;
                cricket.rests = cricket.next + 6.0 + bank.roll() * 22.0;
            } else {
                cricket.next = t + cricket.every;
                bank.far(&mut commands, "cricket", cricket.at, 0.07, cricket.pitch);
            }
        }
    }
    // Leaves, as a gust comes on.
    let wind = breeze.0;
    if a.leaves > 0.1 && wind > 0.5 && wind > due.wind && t >= due.rustle {
        due.rustle = t + 4.0 + bank.roll() * 8.0;
        let from = about(&mut bank, 5.0, 25.0, 2.0, 10.0);
        bank.far(&mut commands, "rustle", from, 0.55 * a.leaves * wind, 1.0);
    }
    due.wind = wind;
}
