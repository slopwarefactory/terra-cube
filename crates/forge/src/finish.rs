//! The local, free, repeatable half of a build: Tripo's rigged export in, the game's model out.
//! Stands the model at the origin facing +Z at its height in metres, names and makes its clips,
//! and measures what the game needs to know (`meta.toml`). Run again after any change to
//! clips.rs or style.toml: it never talks to Tripo.

use crate::clips::{self, Clip};
use crate::glb::Glb;
use crate::rig::{Pose, Facing, Roles, Skeleton};
use crate::spec::{self, ClipMeta, Creature, Meta, Plan};
use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use serde_json::{Value, json};
use std::path::Path;

pub struct Finished {
    pub glb: Glb,
    pub skeleton: Skeleton,
    pub roles: Roles,
    pub clips: Vec<Clip>,
    pub meta: Meta,
    /// Metres to a unit of the file the skeleton and clips are in.
    pub scale: f32,
}

/// Mesh bounds from the POSITION accessors' own min and max.
fn bounds(glb: &Glb) -> Result<(Vec3, Vec3)> {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for mesh in glb.array("meshes") {
        for p in mesh["primitives"].as_array().into_iter().flatten() {
            let a = &glb.array("accessors")[p["attributes"]["POSITION"].as_u64().context("primitive without POSITION")? as usize];
            let v = |key: &str| -> Vec3 { Vec3::from_slice(&a[key].as_array().map_or(vec![0.0; 3], |m| m.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())) };
            lo = lo.min(v("min"));
            hi = hi.max(v("max"));
        }
    }
    if lo.x > hi.x {
        bail!("the model has no mesh");
    }
    Ok((lo, hi))
}

/// How fast the ground passes under the feet of an in-place gait, in file units a second:
/// the middle backward speed of feet while they are down. The middle, not the mean, because
/// that is the speed at which the fewest planted frames slide (`check` measures the same).
fn ground_speed(sk: &Skeleton, roles: &Roles, clip: &Clip) -> Option<f32> {
    let floor = roles.legs.iter().map(|l| sk.position(l.foot()).y).fold(f32::MAX, f32::min);
    let worlds: Vec<_> = clip.frames.iter().map(|p| sk.pose_world(p)).collect();
    // A foot is down within a hundredth of the body's height of the floor; widen that for a
    // clip whose feet never quite get there.
    for near in [0.01, 0.03, 0.08] {
        let mut speeds = Vec::new();
        for w in worlds.windows(2) {
            for leg in &roles.legs {
                let foot = leg.foot();
                if w.iter().all(|m| m[foot].w_axis.y - floor <= near * roles.height) {
                    speeds.push(-(w[1][foot].w_axis.truncate() - w[0][foot].w_axis.truncate()).dot(roles.forward) * clips::FPS);
                }
            }
        }
        if speeds.len() >= 6 {
            speeds.sort_by(f32::total_cmp);
            let middle = speeds[speeds.len() / 2];
            return (middle > 0.0).then_some(middle);
        }
    }
    None
}

/// How fast a recorded gait goes, two ways, over each pair of legs (a biped's one, an animal's
/// fore and hind). `stance`: of a pair, the foot that is lower is the one carrying the body,
/// and the body goes forward as fast as that foot goes back; the middle of those speeds.
/// `steps`: how far apart the pair's feet get, fore and aft, added up over the clip: each
/// swing from one foot ahead to the other ahead is a step's travel. The first is right for a
/// run too (a step's travel leaves out the flight); the second is the check on it.
fn recorded_speed(sk: &Skeleton, roles: &Roles, clip: &Clip) -> Option<(f32, f32)> {
    let pair = |front: bool| -> Option<[usize; 2]> {
        let of: Vec<usize> = roles.legs.iter().filter(|l| l.front == front).map(|l| l.foot()).collect();
        (of.len() == 2).then(|| [of[0], of[1]])
    };
    let pairs: Vec<[usize; 2]> = if roles.legs.len() == 2 { vec![[roles.legs[0].foot(), roles.legs[1].foot()]] } else { [pair(true), pair(false)].into_iter().flatten().collect() };
    if pairs.is_empty() {
        return None;
    }
    let worlds: Vec<_> = clip.frames.iter().map(|p| sk.pose_world(p)).collect();
    let (mut stance, mut swung) = (Vec::new(), 0.0);
    for w in worlds.windows(2) {
        let at = |m: &Vec<glam::Mat4>, j: usize| m[j].w_axis.truncate();
        for &[a, b] in &pairs {
            let (low, high) = if at(&w[0], a).y < at(&w[0], b).y { (a, b) } else { (b, a) };
            if at(&w[0], high).y - at(&w[0], low).y > 0.01 * roles.height {
                stance.push(-(at(&w[1], low) - at(&w[0], low)).dot(roles.forward) * clips::FPS);
            }
            let apart = |m: &Vec<glam::Mat4>| (at(m, a) - at(m, b)).dot(roles.forward);
            swung += (apart(&w[1]) - apart(&w[0])).abs();
        }
    }
    stance.retain(|v| *v > 0.0);
    stance.sort_by(f32::total_cmp);
    let steps = swung / 2.0 / clip.seconds() / pairs.len() as f32;
    Some((*stance.get(stance.len() / 2)?, steps))
}

pub fn finish(source: &Path, creature: &Creature, plan: &Plan, _max_slide: f32) -> Result<Finished> {
    // A model built here of boxes has no file to read; any other is Tripo's export.
    let mut crawler = None;
    let mut glb = match &creature.built {
        // (What crawls is made whole by `crawl`: its boxes, its bones and its motion.)
        Some(built) if matches!(built.form.as_str(), "crawler" | "fowl" | "serpent") => {
            let (glb, made) = match built.form.as_str() {
                "fowl" => crate::crawl::Crawler::fowl(built)?,
                "serpent" => crate::crawl::Crawler::serpent(built)?,
                _ => crate::crawl::Crawler::model(built)?,
            };
            crawler = Some(made);
            glb
        }
        Some(built) => crate::built::model(built)?,
        None => Glb::read(source)?,
    };
    // What flies or swims is given its bones here (`shaped`), whoever made its mesh.
    if plan.rig == "forge" {
        crate::shaped::rig(&mut glb, creature.built.is_some())?;
    }
    // And what goes on four legs may be given our own skeleton, the walker's (`own`).
    let owned = if plan.rig == "own" { Some(crate::own::Own::rig(&mut glb, creature.height).map(|mut own| { own.low = creature.low; own })?) } else { None };
    let skeleton = Skeleton::read(&glb)?;
    // (A model built here faces +z: it was built so.)
    // And a person may be moved by the two-legged walker in place of anybody's clips (`strider`).
    let strode = if !creature.walked.is_empty() { Some(crate::strider::Strider::new(&skeleton, creature.height)?) } else { None };
    let mut roles = match &owned {
        _ if crawler.is_some() => crawler.as_ref().unwrap().roles(&skeleton),
        Some(own) => own.roles(&skeleton),
        None => Roles::read(&skeleton, if creature.built.is_some() { Facing::Told(Vec3::Z) } else { Facing::parse(&creature.facing)? }),
    };
    roles.nose = crate::check::nose(&glb, &skeleton, &roles);
    roles.sinks = match creature.rests.as_str() {
        "belly" => 0.62,
        "crouch" => 0.34,
        "curl" => 0.55,
        "side" | "stands" | "" => 0.0,
        other => bail!("rests = \"{other}\": side, belly, crouch, curl or stands"),
    };
    roles.curls = creature.rests == "curl";
    // A four-legged creature with any other number of legs has a skeleton that is short of
    // one (Tripo's rig sometimes is) or one the reader gets wrong. Say so; do not make clips.
    if creature.plan == "quadruped" && roles.legs.len() != 4 {
        bail!("{} legs found on a four-legged creature: the rig is short of a leg, or misread. `cargo forge inspect` shows it; a rig short of a leg wants rigging again", roles.legs.len());
    }

    let (lo, hi) = bounds(&glb)?;
    let scale = match creature.span {
        // (Built here, it is already its true size.)
        _ if creature.built.is_some() => 1.0,
        Some(span) => span / (hi.x - lo.x).max(hi.z - lo.z).max(1e-5),
        None => creature.height / (hi.y - lo.y).max(1e-5),
    };

    // Clips: Tripo's under our names, then the made ones the plan asks for.
    let theirs = clips::read(&glb, &skeleton)?;
    let mut out: Vec<Clip> = Vec::new();
    let mut library: Option<crate::library::Library> = None;
    // Each gait's own speed over the ground, played as it is (file units a second).
    let mut paces: std::collections::BTreeMap<String, f32> = Default::default();
    for name in plan.clips.iter().filter(|c| !creature.without.contains(c)) {
        let preset = plan.presets.get(name).filter(|_| !creature.made.contains(name));
        // (A described motion's clip is named by its description, cut short: the start is enough.)
        let found = preset.and_then(|p| theirs.iter().position(|c| c.name.rsplit(':').next() == Some(p.as_str()) || (plan.motions.contains_key(p) && c.name.starts_with(p.as_str()))));
        // A clip of the library, carried onto this skeleton, comes before Tripo's own.
        // (Not onto a body whose skin tears, one that has had to have its made clips eased:
        // a fox, a toad and a bullfrog tore at two and three times the bar.)
        let carried = match plan.library.get(name).filter(|_| !creature.made.contains(name) && (plan.rig == "humanoid" || creature.ease >= 1.0)
            // (Nor a wolf's gait onto what hops.)
            && !(creature.hops && matches!(name.as_str(), "walk" | "run" | "sprint"))
            && !(name == "lie" && creature.rests != "side")) {
            Some(theirs) => {
                // (A person's clips go on a person's skeleton bone for bone; an animal's on
                // anything with four legs, by its roles.)
                let human = plan.rig == "humanoid";
                if library.is_none() {
                    library = Some(if human { crate::library::Library::read()? } else { crate::library::Library::open(crate::library::beast())? });
                }
                let lib = library.as_ref().unwrap();
                Some(Clip { name: name.clone(), looping: true, frames: if human { lib.carry(theirs, &skeleton)? } else { lib.carry_beast(theirs, &skeleton, &roles)? }, source: "library", speed: None })
            }
            None => None,
        };
        // Asleep is where the fall ends, held, breathing: five seconds of its last moment,
        // the body rising and sinking a little.
        let carried = carried.map(|mut clip| {
            if name == "lie" && plan.rig != "humanoid" {
                let last = clip.frames.last().cloned().unwrap_or_else(|| skeleton.rest());
                clip.frames = (0..=150).map(|i| {
                    let mut pose = last.clone();
                    pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * 0.008 * roles.height * (0.5 - 0.5 * (i as f32 / 150.0 * std::f32::consts::TAU).cos()));
                    pose
                }).collect();
            }
            clip
        });
        let recorded = carried.as_ref().or(found.map(|i| &theirs[i]));
        // (One that rests on its feet, a pony or a moose, rests as it stands: it is not laid
        // down as a dog lies because the plan has a clip of that name.)
        // (What hops, hops, whosever skeleton it is on: its gaits are the hop's recipe laid on
        // our bones by their roles, not the walker's steps. A frog put on our skeleton walked.)
        let stands = owned.is_some() && name == "lie" && matches!(creature.rests.as_str(), "stands" | "");
        let mut clip = if let Some(crawler) = &crawler { crawler.clip(name, &skeleton)? } else if let Some(own) = owned.as_ref().filter(|_| stands) {
            Clip { name: name.clone(), source: "made", ..own.clip_judged("idle", &skeleton, &|_| None)? }
        } else if let Some(own) = owned.as_ref().filter(|_| crate::own::WALKED.contains(&name.as_str()) && !(creature.hops && matches!(name.as_str(), "walk" | "run" | "sprint"))) {
            // (Judged as it will be drawn: raised out of the ground first, as every lying clip is.)
            own.clip_judged(name, &skeleton, &|pose| {
                let mut pose = pose.clone();
                pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * crate::check::sunk(&glb, &skeleton, std::slice::from_ref(&pose))[0]);
                // (And not torn: a pose that passes every measure of lying by pulling its skin
                // apart is not one.)
                let torn = crate::check::tears(&glb, &skeleton, std::slice::from_ref(&pose));
                crate::walker::judged(&glb, &skeleton, &roles, &pose).map(|(short, good)| { let bar = plan.max_stretch.unwrap_or(1.2); (short + (torn - bar).max(0.0) * 4.0, good && torn <= bar) })
            })?
        } else if let Some(strode) = strode.as_ref().filter(|_| creature.walked.contains(name)) { strode.clip(name, &skeleton)? } else { match recorded {
            Some(taken) => {
                let source = taken.source;
                let looping = !matches!(name.as_str(), "attack" | "hurt" | "die" | "jump" | "land");
                let mut frames = match plan.trim.get(name) {
                    Some(&[from, to, speed]) => clips::trimmed(taken, from, to, speed),
                    None => taken.frames.clone(),
                };
                if matches!(name.as_str(), "walk" | "run") && creature.gait_gain != 1.0 {
                    clips::gained(&skeleton, &mut frames, creature.gait_gain);
                }
                if looping {
                    clips::close_loop(&mut frames);
                    clips::centred(&skeleton, &roles, &mut frames);
                }
                // A leap is recorded leaving the ground, and the game is what lifts the body:
                // each moment of it is stood on its own lowest foot.
                if matches!(name.as_str(), "jump" | "fall" | "land") {
                    clips::footed(&skeleton, &roles, &mut frames);
                }
                // The library's swimmer lies at the height of its own feet; the game holds a
                // swimmer by the middle. It is raised until its hips, on the whole, are where
                // they are standing.
                if name == "swim" && source == "library" {
                    let high = |pose: &crate::rig::Pose| skeleton.pose_world(pose)[skeleton.root].w_axis.y;
                    let lift = high(&skeleton.rest()) - frames.iter().map(high).sum::<f32>() / frames.len().max(1) as f32;
                    for pose in &mut frames {
                        pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * lift);
                    }
                }
                // A carried animal's gait is set down: its body's height came from another
                // animal's build, and the lowest any foot comes in the whole stride is the
                // ground. (By the stride for a gallop, which leaves the ground.)
                // (A person's too: the hips went by the two figures' heights, and a body with
                // shorter legs for its height walked 7 cm over the ground.)
                if source == "library" && matches!(name.as_str(), "idle" | "walk" | "run" | "sprint") && !roles.legs.is_empty() {
                    let low = |pose: &crate::rig::Pose| {
                        let world = skeleton.pose_world(pose);
                        roles.legs.iter().map(|l| world[l.foot()].w_axis.y).fold(f32::MAX, f32::min)
                    };
                    // (A walk has a foot down at every moment, and is set down moment by moment.)
                    let stood = low(&skeleton.rest());
                    let stride = frames.iter().map(low).fold(f32::MAX, f32::min) - stood;
                    for pose in &mut frames {
                        let over = if matches!(name.as_str(), "walk" | "idle") { low(pose) - stood } else { stride };
                        pose.root_t -= skeleton.base.inverse().transform_vector3(Vec3::Y * over);
                    }
                }
                // And no part of it goes under the ground: a troll laid down where the library's
                // slight figure lay was a hand's depth into the floor, and a golem's toes in a lunge.
                // (Not an animal's gait, which was set down by its feet: a paw turned toe down
                // goes a little into the floor, and raising the whole animal for it left the
                // other three feet in the air.)
                let gait = matches!(name.as_str(), "idle" | "walk" | "run" | "sprint");
                if source == "library" && name != "swim" && !gait {
                    // On our own skeleton the whole neck is one bone and the head another, where
                    // the animal the clip was drawn on has four: each is given half of what was
                    // carried onto it. (All of it, a pony's neck folded flat under it as it died.)
                    if owned.is_some() {
                        let rest = skeleton.rest();
                        for pose in &mut frames {
                            // (And the tail, two bones for the animal's eight, a third.)
                            for (&j, much) in roles.head.iter().map(|j| (j, 0.5)).chain(roles.tail.iter().map(|j| (j, 0.35))) {
                                pose.rotations[j] = rest.rotations[j].slerp(pose.rotations[j], much);
                            }
                        }
                    }
                    let under = crate::check::off_floor(&glb, &skeleton, &frames);
                    for (pose, under) in frames.iter_mut().zip(under) {
                        // (Raised out of the ground always; and what dies is let down onto it.)
                        pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * if name == "die" { under } else { under.max(0.0) });
                    }
                }
                Clip { name: name.clone(), looping, frames, source, speed: None }
            }
            None => {
                let stand = out.iter().find(|c| c.name == "idle" && c.source != "made").map(|c| &c.frames[0]);
                clips::make(name, &skeleton, &roles, &out, stand, plan.upright, scale, (1.0 + creature.gait_gain) / 2.0, creature.ease, creature.hops).with_context(|| format!("no preset and no recipe for the clip `{name}` (clips.rs `make`)"))?
            }
        } };
        // A carried animal's gait goes at the speed that animal's own made gait went (its legs'
        // length says how fast it walks and runs: the game's speeds, and who catches whom, are
        // as they were). A walk is played faster or slower until its feet hold at that speed:
        // a badger steps quicker than a wolf. A gallop is not: its feet say less than it
        // covers, being in the air for part of it.
        if clip.source == "library" && plan.rig != "humanoid" && matches!(name.as_str(), "walk" | "sprint")
            && let Some(own) = clips::make(name, &skeleton, &roles, &out, None, plan.upright, scale, (1.0 + creature.gait_gain) / 2.0, creature.ease, creature.hops).and_then(|c| c.speed)
        {
            if name == "walk" && let Some((stance, steps)) = recorded_speed(&skeleton, &roles, &clip) {
                let feet = if (stance / steps - 1.0).abs() < 0.25 { stance } else { steps };
                clip.frames = clips::trimmed(&clip, 0.0, clip.seconds(), (own / feet).clamp(0.6, 1.8));
            }
            clip.speed = Some(own);
        }
        // A person goes at the speeds Tripo's own walk and run went: the game's speeds stay as
        // they were. The clips are not played to fit them. (They were, until 2026-11-12: the
        // library's jog is a 5.4 m/s run and its sprint 8.3, and played at seven tenths of
        // their rate to go 3.6 and 5.4 they were a run in slow motion on sliding feet. The
        // game mixes the gaits to fit the speed instead, each by its own true speed, `paces`.)
        if clip.source == "library" && plan.rig == "humanoid" && matches!(name.as_str(), "walk" | "run")
            && let Some((own_stance, own_steps)) = found.and_then(|i| {
                // (Tripo's clip as it was used: its stride reined in where the creature's is, its loop closed, stood over its own place.)
                let mut frames = theirs[i].frames.clone();
                if creature.gait_gain != 1.0 {
                    clips::gained(&skeleton, &mut frames, creature.gait_gain);
                }
                clips::close_loop(&mut frames);
                clips::centred(&skeleton, &roles, &mut frames);
                recorded_speed(&skeleton, &roles, &Clip { name: name.clone(), looping: true, frames, source: "tripo", speed: None })
            })
            && let Some((stance, _)) = recorded_speed(&skeleton, &roles, &clip)
        {
            let own = if (own_stance / own_steps - 1.0).abs() < 0.25 { own_stance } else { own_steps };
            let _ = stance;
            clip.speed = Some(own);
        }
        // A body that needed its gait reined in needs the same of a leap, by half as much.
        if matches!(name.as_str(), "jump" | "fall") && clip.source == "made" && creature.gait_gain != 1.0 {
            clips::gained(&skeleton, &mut clip.frames, (1.0 + creature.gait_gain) / 2.0);
        }
        if matches!(name.as_str(), "walk" | "run" | "sprint") && !roles.legs.is_empty() && clip.speed.is_none() && clip.source == "made" {
            clips::grounded(&skeleton, &roles, &mut clip.frames);
        }
        // A recorded gait is kept as it was recorded, and the game moves the body at the speed
        // its feet say. Until 2026-11-11 its feet were then locked to a straight line at that
        // speed, its stride lengthened, and the whole gait replaced by a made one where the
        // feet still slid by our measure. Set side by side (three sheets of the player), the
        // recorded walk was a person walking and what replaced it was a shuffle: an ankle
        // does not travel at one speed while its foot is down (the heel lands, the foot
        // rolls, the toe pushes off), so the measure failed a good clip.
        if matches!(name.as_str(), "walk" | "run" | "sprint") && clip.source != "made" && clip.speed.is_none()
            && let Some((stance, steps)) = recorded_speed(&skeleton, &roles, &clip)
        {
            if std::env::var_os("FORGE_TRACE").is_some() {
                eprintln!("{name} recorded: {:.2} m/s by the foot that is down, {:.2} by its steps", stance * scale, steps * scale);
            }
            // Where the two disagree by more than a quarter the body is not one the first
            // can read (the troll's run: its lower foot is not always the one down).
            clip.speed = Some(if (stance / steps - 1.0).abs() < 0.25 { stance } else { steps });
            // The library's sprint is a sprinter's, near twice the run (its steps say less: a
            // sprinter is in the air between them). The game's sprint is half as fast again
            // as the run, so the clip is played that much slower and its feet still hold.
            if let (true, Some(run)) = (name == "sprint" && clip.source == "library", out.iter().find(|c| c.name == "run").and_then(|c| c.speed)) {
                clip.speed = Some(stance * (run * 1.5 / stance).clamp(0.6, 1.0));
            }
        }
        // What goes on four legs has its gaits walked: the body moved by its feet at the
        // speed the gait is for, and one stride of it kept (`walker.rs`). Whatever the clip
        // was till now (made here, or carried from the wolf somebody drew) said how fast.
        // (Not what hops; and not where it does not come out: the clip stays as it was.)
        if plan.rig != "humanoid" && creature.plan == "quadruped" && !creature.hops && matches!(name.as_str(), "walk" | "run" | "sprint")
            && let Some(speed) = clip.speed
            && let Some(frames) = crate::walker::stride(&skeleton, &roles, out.iter().find(|c| c.name == "idle").map_or(&skeleton.rest(), |c| &c.frames[0]), speed, scale)
            // (Kept only if it does not tear the skin: a toad's and a tortoise's legs go out
            // sideways and do not fold under them, and walked so their skin was pulled to
            // twice its length.)
            && crate::check::tears(&glb, &skeleton, &frames) <= plan.max_stretch.unwrap_or(1.0) * 0.95
        {
            (clip.frames, clip.source) = (frames, "walker");
        }
        // And it lies down as a dog lies (`walker::lie`), where it is one that lies on its
        // belly and the pose does not tear it.
        if plan.rig != "humanoid" && creature.plan == "quadruped" && name == "lie" && creature.rests == "belly"
            && let Some(frames) = crate::walker::lie(&glb, &skeleton, &roles, out.iter().find(|c| c.name == "idle").map_or(&skeleton.rest(), |c| &c.frames[0]))
            && crate::check::tears(&glb, &skeleton, &frames) <= plan.max_stretch.unwrap_or(1.0) * 0.95
        {
            // Kept only if it is lying by every measure of `poses.toml`; what it falls short
            // by is said either way (`FORGE_TRACE=1`), for whoever is to make it lie.
            let (text, good) = crate::walker::measured(&glb, &skeleton, &roles, &frames[0]).unwrap_or_default();
            if std::env::var_os("FORGE_TRACE").is_some() {
                eprintln!("lie {}: {text}", if good { "kept" } else { "not kept" });
            }
            // (`FORGE_KEEP=1` keeps it though it fails, to be looked at: never for what is committed.)
            if good || std::env::var_os("FORGE_KEEP").is_some() {
                (clip.frames, clip.source) = (frames, "walker");
            }
        }
        // Its own speed over the ground: the library's, where the library says (its figure's, by this one's size).
        if matches!(name.as_str(), "walk" | "run" | "sprint") {
            // (By its size, and not in step with it: a figure twice as tall takes strides twice
            // as long and takes them slower, by the root of its size, as anything heavy does.
            // Its clips are played that much slower. At the same rate as a person's, a troll
            // three metres tall ran at 8.6 m/s with a person's quick steps.)
            let said = library.as_ref().filter(|_| clip.source == "library" && plan.rig == "humanoid").and_then(|lib| {
                let big = lib.tall(&skeleton);
                Some((lib.travels(plan.library.get(name)?)? * big, 1.0 / (big * scale).max(0.05).sqrt()))
            });
            if let Some((_, slower)) = said.filter(|(_, slower)| (slower - 1.0).abs() > 0.05) {
                clip.frames = clips::trimmed(&clip, 0.0, clip.seconds(), slower);
            }
            let said = said.map(|(pace, slower)| if (slower - 1.0).abs() > 0.05 { pace * slower } else { pace });
            // (The library's own figure goes at the library's own speeds: it is there to be the clips as they were made.)
            let authored = (creature.plan == "authored").then(|| crate::library::Library::read().ok()).flatten().and_then(|lib| lib.travels(plan.presets.get(name)?));
            if let Some(pace) = authored {
                clip.speed = Some(pace);
            }
            // A person goes at its clips' own speeds: its walk and its run are the library's,
            // played as they are with nothing mixed in. (Until 2026-11-12 it went at the speeds
            // Tripo's walk and run had gone, 3.6 m/s at a run where the clip is 5.2, and the
            // run was a mix of walk and run: set beside the library's own figure it was the
            // less natural, and at the clips' speeds the developer called the two about
            // equal.) Its sprint is a quarter faster than its run, a mix of the run and the
            // sprint: the sprint's own 8 m/s crosses the island in two minutes.
            if let Some(pace) = said {
                clip.speed = Some(if name == "sprint" { paces.get("run").map_or(pace, |run| (run * 1.25).min(pace)) } else { pace });
            }
            if let Some(pace) = said.or(clip.speed) {
                paces.insert(name.clone(), pace);
            }
        }
        // However it rests, none of it is in the ground: it is raised by as far as the
        // lowest of its skin is under the floor. (Six made rest poses were, by 4% to 16% of
        // the creature's height, five of them marked as looked at: the check for it was
        // written only after the developer saw the wolf's paws in the floor.)
        // An animal whose skin will not take much bending (`ease` under 1: a toad, a tortoise)
        // strikes with its whole body, which bends nothing: drawn back and thrown forward a
        // third of its length. Its made attack at a third of its turns could not be told from
        // standing (looked at 2026-11-13). (Feeding the same way, dipped, put its feet in the
        // ground: its eating is still faint.)
        if clip.source == "made" && creature.ease < 1.0 && owned.is_none() && crawler.is_none() && plan.rig != "humanoid" && name == "attack" {
            let n = clip.frames.len().max(2) as f32 - 1.0;
            let bump = |t: f32, from: f32, top: f32, to: f32| if t <= from || t >= to { 0.0 } else if t < top { (t - from) / (top - from) } else { (to - t) / (to - top) };
            for (i, pose) in clip.frames.iter_mut().enumerate() {
                let t = i as f32 / n;
                let moved = roles.forward * roles.length * (0.3 * bump(t, 0.3, 0.5, 0.9) - 0.1 * bump(t, 0.0, 0.25, 0.4)) + Vec3::Y * roles.height * 0.12 * bump(t, 0.3, 0.45, 0.7);
                pose.root_t += skeleton.base.inverse().transform_vector3(moved);
            }
        }
        // And feeds the same way: its body let down a little and forward, bobbing, with its
        // feet put back where they stand so that its legs bend under it. Kept only if its skin
        // takes it. (Without its feet put back they went into the ground with it.)
        if clip.source == "made" && creature.ease < 1.0 && owned.is_none() && crawler.is_none() && plan.rig != "humanoid" && name == "eat" {
            let (n, whole) = (clip.frames.len().max(2) as f32 - 1.0, clip.frames.clone());
            for (i, pose) in clip.frames.iter_mut().enumerate() {
                let t = i as f32 / n;
                let down = (t / 0.2).min((1.0 - t) / 0.2).clamp(0.0, 1.0);
                let moved = (roles.forward * roles.length * 0.05 - Vec3::Y * roles.height * (0.09 + 0.03 * (t * std::f32::consts::TAU * 6.0).sin())) * down;
                pose.root_t += skeleton.base.inverse().transform_vector3(moved);
                clips::stand_folded(&skeleton, &roles, pose);
            }
            let bad = clip.frames.iter().any(|f| f.rotations.iter().any(|r| !r.is_finite())) || crate::check::tears(&glb, &skeleton, &clip.frames) > plan.max_stretch.unwrap_or(1.2) * 0.95;
            if bad {
                clip.frames = whole;
            }
        }
        // On our own skeleton, eating that tears the skin is eased back toward standing until
        // it does not: a boar has next to no neck, and its head cannot be brought as low as
        // a deer's without pulling its skin to twice its length.
        if owned.is_some() && name == "eat" {
            let (bar, rest, whole) = (plan.max_stretch.unwrap_or(1.2), skeleton.rest(), clip.frames.clone());
            for much in [1.0f32, 0.85, 0.7, 0.55, 0.4, 0.25] {
                clip.frames = whole.iter().map(|f| Pose { rotations: f.rotations.iter().zip(&rest.rotations).map(|(r, at_rest)| at_rest.slerp(*r, much)).collect(), root_t: rest.root_t.lerp(f.root_t, much) }).collect();
                if crate::check::tears(&glb, &skeleton, &clip.frames) <= bar * 0.95 {
                    break;
                }
            }
        }
        // What is built of boxes is never under the ground: any moment its lowest box would
        // be, all of it is raised by that. (Rearing to strike, a beetle's, a spider's and a
        // scorpion's abdomen went into the floor, and rolling over to die their legs and a
        // scorpion's tail did. Seen only at full size, 2026-11-13.) Not its gaits, whose feet
        // are on the ground by how they are made.
        // (Nor is a blow or a flinch made for an animal on our own skeleton: thrown forward
        // and down to strike, a hedgehog's and a rabbit's fore foot went under the floor.)
        let struck = owned.is_some() && clip.source == "made" && matches!(name.as_str(), "attack" | "hurt");
        if struck || crawler.is_some() && !matches!(name.as_str(), "walk" | "run" | "sprint" | "idle" | "eat") {
            let under = crate::check::off_floor(&glb, &skeleton, &clip.frames);
            for (pose, under) in clip.frames.iter_mut().zip(under) {
                // (And what dies is let down onto it: limp, it lay a hand's breadth up.)
                pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * if name == "die" { under } else { under.max(0.0) });
            }
        }
        // And what has died rests on the ground, however its death was made: not under it
        // and not over it. (A frog, a toad and a tortoise made to roll onto their backs ended
        // a hand's breadth up in the air.) By how far the last of it is off, eased in over
        // the clip, so nothing jumps.
        if name == "die" && clip.source == "made" && crawler.is_none() {
            let off = crate::check::off_floor(&glb, &skeleton, &clip.frames[clip.frames.len() - 1..])[0];
            if std::env::var_os("FORGE_TRACE").is_some() {
                println!("die: the last of it is {off:.3} off the floor (file units; under it if over nothing), height {:.3}", roles.height);
            }
            let n = clip.frames.len() as f32;
            for (i, pose) in clip.frames.iter_mut().enumerate() {
                let k = (i as f32 / (n * 0.6)).min(1.0);
                pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * off * k * k);
            }
        }
        if name == "lie" {
            let under = crate::check::sunk(&glb, &skeleton, &clip.frames);
            for (pose, under) in clip.frames.iter_mut().zip(under) {
                pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * under);
            }
        }
        // Every gait begins at the same part of its stride: the moment one fore foot (a
        // person's foot) passes ahead of the other. The game mixes gaits at the same part of
        // their strides (`creatures::drive`), and a walk begun on one foot mixed with a run
        // begun on the other is legs that do neither.
        let pair: Vec<usize> = { let fore: Vec<usize> = roles.legs.iter().filter(|l| l.front).map(|l| l.foot()).collect(); if fore.len() >= 2 { fore } else { roles.legs.iter().map(|l| l.foot()).collect() } };
        if matches!(name.as_str(), "walk" | "run" | "sprint") && clip.looping && pair.len() >= 2 && clip.frames.len() > 3 {
            let ahead: Vec<f32> = clip.frames.iter().map(|pose| { let world = skeleton.pose_world(pose); (world[pair[0]].w_axis.truncate() - world[pair[1]].w_axis.truncate()).dot(roles.forward) }).collect();
            let n = clip.frames.len() - 1;
            if let Some(first) = (0..n).find(|&i| ahead[(i + n - 1) % n] <= 0.0 && ahead[i] > 0.0) {
                clip.frames.truncate(n);
                clip.frames.rotate_left(first);
                clip.frames.push(clip.frames[0].clone());
            }
        }
        // `FORGE_TRACE=1`: which foot leads through a gait, at eight moments of its stride: the
        // game mixes gaits at the same part of their strides, and they must agree which foot
        // that is (+ the first leg ahead of the second, - behind).
        if std::env::var_os("FORGE_TRACE").is_some() && matches!(name.as_str(), "walk" | "run" | "sprint") && roles.legs.len() >= 2 {
            let lead: String = (0..8).map(|i| {
                let world = skeleton.pose_world(&clip.frames[i * (clip.frames.len() - 1) / 8]);
                let ahead = (world[pair[0]].w_axis.truncate() - world[pair[1]].w_axis.truncate()).dot(roles.forward);
                if ahead > 0.0 { '+' } else { '-' }
            }).collect();
            eprintln!("{name} leads {lead}");
        }
        out.push(clip);
    }

    // Stand it at the origin, facing +Z, at its height.
    let turn = Quat::from_rotation_arc(roles.forward, Vec3::Z);
    let feet = roles.legs.iter().map(|l| skeleton.position(l.foot())).sum::<Vec3>() / roles.legs.len().max(1) as f32;
    let centre = if roles.legs.is_empty() { (lo + hi) / 2.0 } else { feet };
    let pivot = Vec3::new(centre.x, lo.y, centre.z);
    let offset = -(turn * (pivot * scale));
    let scene = glb.json["scene"].as_u64().unwrap_or(0) as usize;
    let roots = glb.json["scenes"][scene]["nodes"].clone();
    let nodes = glb.json["nodes"].as_array_mut().context("no nodes")?;
    nodes.push(json!({ "name": "creature", "children": roots, "translation": offset.to_array(), "rotation": turn.to_array(), "scale": [scale, scale, scale] }));
    let wrapper = nodes.len() - 1;
    glb.json["scenes"][scene]["nodes"] = json!([wrapper]);
    glb.json["asset"]["generator"] = Value::String("terra-cube forge".into());

    clips::write(&mut glb, &skeleton, &out);

    let walk = out.iter().find(|c| c.name == "walk");
    let run = out.iter().find(|c| c.name == "run");
    let sprint = out.iter().find(|c| c.name == "sprint");
    // A walk nobody can measure moves about a body length a cycle.
    let speed = |c: Option<&Clip>| c.map(|c| c.speed.or_else(|| ground_speed(&skeleton, &roles, c)).unwrap_or(roles.length / c.seconds()) * scale).unwrap_or(0.0);
    let name = |j: Option<&usize>| j.map(|&j| skeleton.joints[j].name.clone());
    let mut bones = std::collections::BTreeMap::new();
    for (role, joint) in [("root", Some(&skeleton.root)), ("head", roles.head.last()), ("neck", roles.head.first()), ("tail", roles.tail.first())] {
        if let Some(n) = name(joint) {
            bones.insert(role.to_string(), n);
        }
    }
    for (i, leg) in roles.legs.iter().enumerate() {
        bones.insert(format!("foot{i}"), skeleton.joints[leg.foot()].name.clone());
        // For the game to put the foot on the ground that is there (`cube_game::feet`): the
        // hip, the knee, and what stands on the ground (an upright figure's ankle, an
        // animal's toe), as the made gaits take them. A leg whose bones' names are not its
        // own (Tripo has given two bones one name) is left out.
        let tip = if plan.upright && leg.chain.len() >= 4 { leg.chain[2] } else { leg.foot() };
        let chain = [leg.chain[0], leg.chain[1.min(leg.chain.len() - 1)], tip].map(|j| skeleton.joints[j].name.clone());
        let own = |n: &String| skeleton.joints.iter().filter(|j| &j.name == n).count() == 1;
        if leg.chain.len() >= 3 && chain.iter().all(own) && chain[0] != chain[1] && chain[1] != chain[2] {
            bones.insert(format!("leg{i}"), chain.join(">"));
        }
    }
    // Its size as it stands (the first frame of idle), not as it rests: a T-pose is all arms.
    let stand = out.iter().find(|c| c.name == "idle").map_or_else(|| skeleton.rest(), |c| c.frames[0].clone());
    let (slo, shi) = crate::check::bounds(&glb, &skeleton, &stand).unwrap_or((lo, hi));
    // What a blow can strike, as it stands; and with its head down to feed and lying, where
    // it does those, since a head at the ground is not where a head held up was.
    let mut hit = crate::check::parts(&glb, &skeleton, &stand, &roles, scale).unwrap_or_default();
    for when in ["eat", "lie"] {
        if let Some(clip) = out.iter().find(|c| c.name == when) {
            let pose = &clip.frames[clip.frames.len() / 2];
            hit.extend(crate::check::parts(&glb, &skeleton, pose, &roles, scale).unwrap_or_default().into_iter().map(|b| crate::spec::HitBox { when: when.into(), ..b }));
        }
    }
    let meta = Meta {
        height: creature.height,
        length: crate::check::extent(&glb, &skeleton, &stand, roles.forward).unwrap_or((shi - slo).dot(roles.forward).abs()) * scale,
        width: crate::check::extent(&glb, &skeleton, &stand, roles.side).unwrap_or((shi - slo).dot(roles.side).abs()) * scale,
        walk_speed: creature.speeds.map_or_else(|| speed(walk), |s| s[0]),
        run_speed: creature.speeds.map_or_else(|| speed(run), |s| s[1]),
        sprint_speed: creature.speeds.map_or_else(|| speed(sprint), |s| s[2]),
        triangles: glb.triangles(),
        joints: skeleton.joints.len(),
        clips: out.iter().map(|c| (c.name.clone(), ClipMeta { seconds: c.seconds(), looping: c.looping, source: c.source.into(), speed: paces.get(&c.name).copied().unwrap_or(0.0) * scale })).collect(),
        bones,
        stride: (strode.is_some() && plan.rig == "humanoid").then(|| crate::grip::rests(&skeleton)),
        grip: if plan.rig == "humanoid" { crate::grip::grips(&skeleton, scale) } else { Default::default() },
        hit,
    };
    Ok(Finished { glb, skeleton, roles, clips: out, meta, scale })
}

/// Runs the finish for a creature and writes `model.glb` and `meta.toml`.
pub fn run(id: &str, style: &spec::Style) -> Result<Finished> {
    let creature: Creature = spec::load(&spec::dir(id).join("creature.toml"))?;
    let plan = style.plans.get(&creature.plan).with_context(|| format!("{id}: no plan `{}` in style.toml", creature.plan))?;
    let source = spec::work(id).join("rigged.glb");
    if !source.exists() && creature.built.is_none() {
        bail!("{id}: {} is missing; `cargo forge build {id}` fetches it", source.display());
    }
    let mut done = finish(&source, &creature, plan, style.checks.max_slide)?;
    done.glb.write(&spec::dir(id).join("model.glb"))?;
    spec::save(&spec::dir(id).join("meta.toml"), &done.meta, "# Measured by `cargo forge finish`; do not edit.\n")?;
    Ok(done)
}
