//! `cargo world`: the world and what lives in it, judged with no window. It makes the same
//! world the game does and steps the same simulation, then answers in numbers (`survey`,
//! `run`) or in one picture from above (`map`).

use cube_sim::{
    TICKS,
    life::{Act, Driver, Kind, Sim, Temper},
    survey,
    terrain::{self, Biome, Land, SEA},
};
use cube_sim::scale;
use cube_voxel::{Block, CELL, World, cells};
use glam::{IVec3, Vec3};
use std::path::Path;

const HELP: &str = "\
cargo world survey [--seed N] [--radius CHUNKS]   the land in numbers: ok, or FAIL lines
cargo world run [--seconds 120] [--seed N]        the creatures living in it for a while, with a
                                                  person walking about the start: a line a kind,
                                                  then ok or FAIL lines
cargo world years [N] [--seed N]                  the creatures by the day for N (3) years, nobody
                                                  watching: births, deaths and of what, each kind's
                                                  numbers year by year; FAIL if a kind dies out, overruns,
                                                  or (over 16 years or more) has not settled; under a kind
                                                  that fell by half, who hunted it
cargo world who [--seed N]                        where each group of creatures starts: kind, how
                                                  many, metres east and south of the start, biome
                                                  (for `cargo sandbox --wild --scene F --at X,Z`)
cargo world trunks [--seed N]                     how far what stops a body at a trunk is from the trunk
                                                  that is drawn, a metre up, round every trunk near the start
cargo world cost [--seed N]                       what making the world costs: a region's cells by part
                                                  (ground, what stands, cover) and a chunk's mesh, ms
cargo world dig [--holes 120]                     HOLES (120) holes and stones cut to the 5 cm bit all
                                                  over the land: what broken cells cost, and whether
                                                  creatures still live among them
cargo world fell [--seed N]                       a tree near the start cut through: that it comes
                                                  (`fell X,Z`: the tree nearest a place)
cargo world lying [--seed N]                      a tree felled and bucked: how far what a body stands on
                                                  is from each log as it is drawn, lying at its angle
cargo world hunks [--seed N]                      a tree and a boulder near the start worked as a person
                                                  works them, to the pieces that are carried: felled,
                                                  limbed, broken into logs, split; chipped, broken
cargo world quarry [--seed N]                     a boulder near the start cut through with a pick: that
                                                  it becomes cells, its top comes down and lies as stone
cargo world roots [--seed N]                      every rooted tree near the start: how far the lowest
                                                  wood of its foot hangs over the ground under it
cargo world joined [--seed N]                     every tree near the start as the cells it is once cut
                                                  into: whether all its wood is one body with its foot
                                                  loose whole, goes over, lies on the ground as cells,
                                                  leaves nothing in the air, and what each step costs
cargo world replay [--seed N]                     the world as its seed and what was done to it: a tree
                                                  felled and cut up, holes dug, earth put down; then the
                                                  land made again from the seed and the same edits done
                                                  to it: the two must be the same, cell for cell
cargo world grade [FILE] [--seed N]                where the ground is easy, steep and cliff, a line
                                                  for each band of height; with FILE, a picture of
                                                  it from above (green easy, yellow steep, red cliff)
cargo world life [--seed N]                       the ecology: how much of the island suits each
                                                  kind of small life and when it is about; where
                                                  each animal's groups live against what it wants
cargo world find deep|BIOME                       the nearest places to the start with water deep
                                                  enough to swim, or of a biome (for `--at X,Z`)
cargo world map FILE [--seconds 60] [--seed N]    a picture from above, with where each creature
                                                  went (dots: white person, yellow timid, red
                                                  hunter and hostile, orange wary)
The seed and radius default to the game's (cube_sim::terrain::SEED, RADIUS).";

struct Args {
    seed: u32,
    radius: i32,
    seconds: f32,
    file: Option<String>,
}

fn fail(message: &str) -> ! {
    eprintln!("world: {message}");
    std::process::exit(2);
}

/// One line of a check: quiet when it passes, unless something else failed.
struct Checks(Vec<(bool, String)>);

impl Checks {
    fn bar(&mut self, name: &str, value: f32, ok: bool, bar: &str) {
        self.0.push((ok, format!("{} {name} {value:.2} ({bar})", if ok { "ok" } else { "FAIL" })));
    }
    fn finish(self) -> ! {
        let failed = self.0.iter().any(|c| !c.0);
        let verbose = std::env::var_os("WORLD_VERBOSE").is_some();
        for (ok, line) in &self.0 {
            if !ok || verbose {
                println!("{line}");
            }
        }
        if failed {
            std::process::exit(1);
        }
        println!("ok");
        std::process::exit(0);
    }
}

fn kinds() -> Vec<Kind> {
    let kinds = Kind::load(&Path::new("assets").join("creatures"));
    if kinds.is_empty() {
        fail("no creatures under assets/creatures; run from the repository root (cargo world does)");
    }
    kinds
}

/// The world with its creatures and a person at the start who wanders, to be fled and chased.
fn living(args: &Args) -> (World, Land, Sim) {
    let (world, land) = made(args);
    let mut sim = Sim::new(kinds(), args.seed as u64);
    sim.populate(&world, &land);
    // As in the game: only what is near the person at the start is lived, the rest sleeps.
    sim.focus = Some(Vec3::new(0.25, land.height(0, 0) as f32 * CELL, 0.25));
    if let Some(person) = sim.kind("player") {
        sim.add(&world, person, Vec3::new(0.25, land.height(0, 0) as f32 * CELL, 0.25), 0.0, Driver::Mind);
    }
    (world, land, sim)
}

/// The lie of the whole island, and the cells of the part round the start that the game would
/// have made standing there: an island a kilometre across is not held whole in cells.
fn made(args: &Args) -> (World, Land) {
    let land = Land::new(args.seed, args.radius);
    let world = terrain::part(&land, (0, 0), terrain::NEAR.min(args.radius));
    (world, land)
}

fn survey_command(args: &Args) -> ! {
    let (world, land) = made(args);
    let s = survey::survey(&land);
    // (Trees and rocks are drawn as shapes by the game, not by the mesher: counted as it draws.)
    cube_voxel::mesh::MODELLED.store(true, std::sync::atomic::Ordering::Relaxed);
    // What the game draws standing at the start: each chunk meshed at the level its distance
    // gives it, as the game does. (All of the world at full detail is never drawn.)
    let faces_of = |c: IVec3, step: i32| [false, true].map(|liquid| if step == 1 { cube_voxel::mesh::mesh_chunk(&world, c, liquid) } else { cube_voxel::mesh::mesh_chunk_far(&world, c, liquid, step) }.indices.len() / 6).iter().sum::<usize>();
    let (mut drawn, mut near) = (0, 0);
    for &c in world.chunks.keys() {
        let away = ((c.as_vec3() + 0.5) * cube_voxel::CHUNK as f32 * CELL - Vec3::new(0.0, SEA as f32 * CELL, 0.0)).length();
        let faces = faces_of(c, if away < 40.0 { 1 } else if away < 104.0 { 2 } else { 4 });
        drawn += faces;
        near += if away < 40.0 { faces } else { 0 };
    }
    let shares: Vec<String> = Biome::ALL.iter().map(|b| format!("{} {:.0}%", b.name(), s.share[*b as usize] * 100.0)).collect();
    println!("land seed={} {:.0}m across, {:.2}m cells: {} · trees {} · highest {:.0}m · cliffs {:.0}% · reached on foot {:.0}%, swimming {:.0}% · {} chunks, {:.2}M faces drawn from the start ({:.2}M of them within 40 m)", args.seed, (land.span * 2) as f32 * CELL, CELL, shares.join(" "), s.trees, s.highest, s.cliffs * 100.0, s.on_foot * 100.0, s.swimming * 100.0, world.chunks.len(), drawn as f32 / 1e6, near as f32 / 1e6);
    let mut checks = Checks(Vec::new());
    let share = |b: Biome| s.share[b as usize];
    checks.bar("water", share(Biome::Water), (0.2..=0.6).contains(&share(Biome::Water)), "0.20 to 0.60 of the world");
    for biome in [Biome::Meadow, Biome::Forest, Biome::Rock] {
        checks.bar(biome.name(), share(biome), share(biome) >= 0.06, "at least 0.06");
    }
    checks.bar("trees", s.trees as f32, s.trees >= 400, "at least 400");
    // Scale: the bars are in cube_sim::scale, with the sizes they judge.
    println!("scale: a person is {}m · the middle tree {:.1} people tall, the tallest {:.0}m · {} rocks, the largest {:.0} times the smallest · easy ground {:.0}% of lowland meadow and forest", scale::PERSON, s.tree, s.tallest, land.rocks.len(), s.rocks, s.easy * 100.0);
    checks.bar("tree-height", s.tree, s.tree >= scale::BAR_TREE, "the middle tree at least 8 people tall");
    checks.bar("rock-sizes", s.rocks, s.rocks >= scale::BAR_ROCKS, "the largest rock at least 10 times the smallest");
    checks.bar("easy-ground", s.easy, s.easy >= scale::BAR_EASY, "at least 0.65 of lowland meadow and forest within 0.75 m over two");
    checks.bar("highest", s.highest, s.highest >= 40.0, "at least 40 m");
    checks.bar("cliffs", s.cliffs, (0.03..=0.35).contains(&s.cliffs), "0.03 to 0.35 of the land");
    checks.bar("drawn", drawn as f32 / 1e6, drawn <= 2_000_000, "at most 2 million faces from the start, with levels of detail");
    checks.bar("on-foot", s.on_foot, s.on_foot >= 0.5, "at least 0.50 of the land from the start");
    checks.bar("swimming", s.swimming, s.swimming >= 0.9, "at least 0.90 of the land from the start");
    checks.finish()
}

fn run_command(args: &Args) -> ! {
    let (world, land, mut sim) = living(args);
    // WORLD_TRACE=wolf: a line a second for one of that kind, the one that ends up stuck most.
    let traced = std::env::var("WORLD_TRACE").ok().and_then(|id| {
        let mut ahead = Sim::clone(&sim);
        (0..(args.seconds * TICKS as f32) as u32).for_each(|_| ahead.step(&world));
        ahead.agents.iter().filter(|(_, a)| ahead.kinds[a.kind].id == id).max_by(|a, b| a.1.log.stuck.total_cmp(&b.1.log.stuck)).map(|(&id, _)| id)
    });
    let mut last_stuck = 0.0;
    for tick in 0..(args.seconds * TICKS as f32) as u32 {
        sim.step(&world);
        if let Some(a) = traced.filter(|_| tick % TICKS == 0).and_then(|id| sim.agents.get(&id)) {
            println!("t={} pos={:.1},{:.1},{:.1} speed={:.1} {:?} {:?} left={:.1} target={:?} stuck={:.0} asks={:.1},{:.1}x{:.1} hurt={:.1}", tick / TICKS, a.body.pos.x, a.body.pos.y, a.body.pos.z, a.body.ground_speed(), a.mind.state, a.act, a.mind.left, a.mind.target, a.log.stuck, a.intent.dir.x, a.intent.dir.y, a.intent.speed, a.hurt);
            // Caught on something: the ground round it, a row a line, in cells over (+) or
            // under its feet; `^` where there is something solid within its height above that.
            if a.log.stuck > last_stuck {
                let cell = |v: f32| (v / CELL).floor() as i32;
                let (cx, level, cz) = (cell(a.body.pos.x), cell(a.body.pos.y + 0.01), cell(a.body.pos.z));
                let solid = |x: i32, y: i32, z: i32| world.get(IVec3::new(x, y, z)).solid();
                for dz in -3..=3 {
                    let row: Vec<String> = (-3..=3)
                        .map(|dx| {
                            let mut top = level + 3;
                            while top > level - 6 && !solid(cx + dx, top - 1, cz + dz) {
                                top -= 1;
                            }
                            let roofed = (0..(a.body.height / CELL).ceil() as i32 + 2).any(|up| solid(cx + dx, top + up, cz + dz));
                            format!("{:>3}{}", top - level, if roofed { '^' } else { ' ' })
                        })
                        .collect();
                    println!("    {}", row.join(""));
                }
            }
            last_stuck = a.log.stuck;
        }
    }
    let mut checks = Checks(Vec::new());
    println!("life seed={} creatures={} (lived, near the start: {}) seconds={}", args.seed, sim.agents.len(), sim.agents.values().filter(|a| a.awake).count(), args.seconds);
    // Whether the larger animals keep to the trails: the share of them standing on one now,
    // against the share of meadow and forest that is trail at all.
    let big: Vec<_> = sim.agents.values().filter(|a| a.awake && a.dead.is_none() && sim.kinds[a.kind].height > 0.5 && sim.kinds[a.kind].temper != cube_sim::life::Temper::Person).collect();
    let on = big.iter().filter(|a| land.trail((a.body.pos.x / CELL).floor() as i32, (a.body.pos.z / CELL).floor() as i32) > 0.0).count();
    let (mut ground, mut trail) = (0, 0);
    for z in (-land.span..land.span).step_by(7) {
        for x in (-land.span..land.span).step_by(7) {
            if matches!(land.biome(x, z), Biome::Meadow | Biome::Forest) {
                ground += 1;
                trail += (land.trail(x, z) > 0.0) as usize;
            }
        }
    }
    println!("trails: {} of {} larger animals on one ({:.0}%) · trails are {:.1}% of meadow and forest", on, big.len(), on as f32 * 100.0 / big.len().max(1) as f32, trail as f32 * 100.0 / ground.max(1) as f32);
    println!("grazed: {} tufts, flowers and ferns eaten by what grazed in sight", sim.eaten.len());
    // How alive it is, in one line: of the creatures lived near the watcher (not the watcher),
    // how much of their time was standing, and what happened among them a minute.
    {
        let lived: Vec<_> = sim.agents.values().filter(|a| a.driver == cube_sim::life::Driver::Mind && a.log.acts.iter().sum::<f32>() > args.seconds * 0.5).collect();
        let (time, minutes) = (lived.iter().map(|a| a.log.acts.iter().sum::<f32>()).sum::<f32>().max(1.0), args.seconds / 60.0);
        let share = |act: Act| lived.iter().map(|a| a.log.acts[act as usize]).sum::<f32>() / time;
        let count = |f: &dyn Fn(&cube_sim::life::Log) -> u32| lived.iter().map(|a| f(&a.log)).sum::<u32>() as f32;
        // (Lying: on its belly, and at rest out of its hours on its feet where it is one that does not lie down.)
        let lain = share(Act::Lie);
        let (idle, fast) = (share(Act::Idle), share(Act::Run) + share(Act::Sprint));
        let (drank, hunts, kills, fled, trips) = (count(&|l| l.drank), count(&|l| l.hunts), count(&|l| l.kills), count(&|l| l.fled), count(&|l| l.trips));
        println!("alive: {} lived · standing {:.0}% of the time, lying down {:.0}%, feeding {:.0}%, running {:.1}% · a minute: {:.1} set out for somewhere, {:.1} drank, {:.1} took fright, {:.1} hunts ({:.0} kills in all)", lived.len(), idle * 100.0, lain * 100.0, share(Act::Eat) * 100.0, fast * 100.0, trips / minutes, drank / minutes, fled / minutes, hunts / minutes, kills);
        if args.seconds >= 240.0 {
            checks.bar("alive standing", idle, idle <= 0.4, "creatures stand about at most two fifths of the time");
            checks.bar("alive errands", trips / minutes, (2.0..=60.0).contains(&(trips / minutes)), "from two to sixty a minute set out for somewhere with a reason (more is the same errand begun over and over)");
        }
    }
    for (k, kind) in sim.kinds.iter().enumerate() {
        // Of those that were lived at all: the rest of the island slept.
        let of: Vec<_> = sim.agents.values().filter(|a| a.kind == k && a.log.acts.iter().sum::<f32>() > args.seconds * 0.5).collect();
        if of.is_empty() {
            continue;
        }
        let n = of.len() as f32;
        let total = n * args.seconds;
        let sum = |f: &dyn Fn(&cube_sim::life::Log) -> f32| of.iter().map(|a| f(&a.log)).sum::<f32>();
        let mut acts: Vec<(Act, f32)> = Act::ALL.iter().map(|&act| (act, sum(&|l| l.acts[act as usize]) / total)).filter(|a| a.1 >= 0.005).collect();
        acts.sort_by(|a, b| b.1.total_cmp(&a.1));
        let acts: Vec<String> = acts.iter().map(|(act, part)| format!("{} {:.0}%", act.clip().0, part * 100.0)).collect();
        let (moved, stuck, swim, boxed) = (sum(&|l| l.moved) / n, sum(&|l| l.stuck) / total, sum(&|l| l.acts[Act::Swim as usize]) / total, sum(&|l| l.boxed) / total);
        let (inside, fell) = (sum(&|l| l.inside as f32), sum(&|l| l.fell as f32));
        println!("{} ×{}: went {:.0}m each · {} · fled {} · hit {} · drank {} · set out {} · hunts {}", kind.id, of.len(), moved, acts.join(" "), sum(&|l| l.fled as f32), sum(&|l| l.struck as f32), sum(&|l| l.drank as f32), sum(&|l| l.trips as f32), sum(&|l| l.hunts as f32));
        checks.bar(&format!("{} inside-ground", kind.id), inside, inside == 0.0, "0 ticks");
        // A kind that roosts, when the watch ends out of its hours: how many are up on
        // something, and how many still in the air (which is none that has found its bough).
        if kind.roosts && of.iter().any(|a| a.mind.about < 0.3) {
            let high = |a: &&cube_sim::life::Agent| a.body.pos.y - land.height((a.body.pos.x / CELL).floor() as i32, (a.body.pos.z / CELL).floor() as i32) as f32 * CELL > 2.0;
            let (up, flying, with) = (of.iter().filter(|a| a.body.grounded && high(a)).count(), of.iter().filter(|a| !a.body.grounded && a.mind.perch.flatten().is_some() && a.mind.trying > 20.0).count(), of.iter().filter(|a| a.mind.perch.flatten().is_some()).count());
            println!("  {} out of its hours: {up} of {} up on a bough, {flying} long in the air, {with} with a bough to go to", kind.id, of.len());
            // (Some are at it at any moment: one that cannot get onto its bough gives it up in
            // half a minute. All of them in the air is none of them ever landing.)
            checks.bar(&format!("{} aloft", kind.id), flying as f32, flying * 2 <= of.len(), "no more than half still in the air after twenty seconds at it");
        }
        checks.bar(&format!("{} fell-out", kind.id), fell, fell == 0.0, "0 times");
        checks.bar(&format!("{} stuck", kind.id), stuck, stuck <= 0.08, "at most 0.08 of the time");
        checks.bar(&format!("{} boxed-in", kind.id), boxed, boxed <= 0.1, "at most 0.10 of the time");
        checks.bar(&format!("{} swimming", kind.id), swim, swim <= 0.05, "at most 0.05 of the time");
        if args.seconds >= 60.0 {
            // That it gets about: three metres, or for a small thing eight times its height (a
            // frog that sits and waits goes two metres in five minutes and is not stuck). Not
            // asked of a kind that lay up through most of the watch: one boar asleep out of
            // its hours goes nowhere, and whether a thing cannot move is `stuck` and
            // `boxed-in`, above.
            // (At rest out of its hours, lying or on its feet where it is one that stands: `Log::lain`.)
            let (lain, need) = (sum(&|l| l.acts[Act::Lie as usize].max(l.lain)) / total, (kind.height * 8.0).min(3.0));
            if lain < 0.5 {
                checks.bar(&format!("{} went", kind.id), moved, moved >= need, &format!("at least {need:.1} m each"));
            }
        }
    }
    checks.finish()
}

/// The top block of a column and how high it is, looking down from the sky.
fn top(world: &World, x: i32, z: i32) -> (Block, i32) {
    let mut y = SEA + cells(55.0);
    while y > 0 && world.get(IVec3::new(x, y, z)) == Block::Air {
        y -= 1;
    }
    (world.get(IVec3::new(x, y, z)), y)
}

fn map_command(args: &Args) -> ! {
    let Some(file) = &args.file else { fail("map wants a file: cargo world map shots/map.png") };
    let (world, land, mut sim) = living(args);
    let side = (land.span * 2) as u32;
    // A pixel is a quarter of a metre.
    let scale = (CELL / 0.25).round().max(1.0) as u32;
    let mut picture = image::RgbImage::new(side * scale, side * scale);
    let tone = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    for z in 0..side as i32 {
        for x in 0..side as i32 {
            let (cx, cz) = (x - land.span, z - land.span);
            let (block, y) = top(&world, cx, cz);
            let (_, y_lit) = top(&world, cx - 1, cz - 1);
            // Lit from the north-west: a rise toward the light is bright, away from it dark.
            let shade = (1.0 + (y - y_lit) as f32 * 0.12).clamp(0.55, 1.5) * (0.8 + (y - SEA) as f32 * CELL * 0.012);
            let mut colour = block.colors().0;
            if block == Block::Water {
                let depth = (SEA - land.height(cx, cz)) as f32 * CELL * 2.0;
                colour = [0.10 - depth * 0.006, 0.34 - depth * 0.018, 0.50 - depth * 0.02];
            }
            let shade = if block == Block::Water { 1.0 } else { shade };
            let pixel = image::Rgb(colour.map(|c| tone(c * shade * 1.6)));
            for (dx, dz) in (0..scale).flat_map(|dx| (0..scale).map(move |dz| (dx, dz))) {
                picture.put_pixel(x as u32 * scale + dx, z as u32 * scale + dz, pixel);
            }
        }
    }
    let mut dot = |pos: Vec3, colour: [u8; 3], size: i32| {
        let (px, pz) = (((pos.x / CELL) + land.span as f32) * scale as f32, ((pos.z / CELL) + land.span as f32) * scale as f32);
        for dz in -size..=size {
            for dx in -size..=size {
                let (x, z) = (px as i32 + dx, pz as i32 + dz);
                if x >= 0 && z >= 0 && (x as u32) < side * scale && (z as u32) < side * scale {
                    picture.put_pixel(x as u32, z as u32, image::Rgb(colour));
                }
            }
        }
    };
    let colour = |temper: Temper| match temper {
        Temper::Person => [255, 255, 255],
        Temper::Timid => [255, 230, 60],
        Temper::Wary => [255, 140, 30],
        Temper::Hunter | Temper::Hostile => [230, 30, 40],
    };
    // Where each went, a fine dot every half second, then where it ended, a fat one.
    for tick in 0..(args.seconds * TICKS as f32) as u32 {
        sim.step(&world);
        if tick % (TICKS / 2) == 0 {
            for agent in sim.agents.values() {
                dot(agent.body.pos, colour(sim.kinds[agent.kind].temper).map(|c| c / 5 * 4), 0);
            }
        }
    }
    for agent in sim.agents.values() {
        dot(agent.body.pos, [0, 0, 0], 3);
        dot(agent.body.pos, colour(sim.kinds[agent.kind].temper), 2);
    }
    if let Some(dir) = Path::new(file).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = picture.save(file) {
        fail(&format!("saving {file}: {e}"));
    }
    println!("map {file} seed={} {}px a side, {:.0}m across, creatures={} after {}s", args.seed, side * scale, side as f32 * CELL, sim.agents.len(), args.seconds);
    std::process::exit(0);
}

/// Holes dug all over the land, to the bit, and what they cost: the cells that broke, the
/// memory for them, the faces added, how long a chunk takes to draw again, and whether the
/// creatures still live without ending up in the ground.
/// How far what stops a body is from what is drawn, at the trunks near the start. A metre
/// above each trunk's foot, from sixteen sides: how far out the first solid point is (what a
/// body meets) and how far out the first cube of the trunk as the game draws it near (cubes
/// of 10 cm, or 15 for a great trunk, of the same shape, `Limb::fill`). The gap is the one
/// less the other: more than nothing, a body is stopped by air; less, it walks into wood.
fn trunks_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    let mut gaps: Vec<f32> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // (Not the trunks at the edge of what was made: the part of one that stands in ground
    // not made is not there.)
    let inside = (2 * r) as f32 * CELL - 4.0;
    for limb in form.limbs.iter().filter(|l| l.roots > 0 && l.a.x.abs() < inside && l.a.z.abs() < inside && seen.insert((l.a.x.to_bits(), l.a.z.to_bits()))) {
        // What is drawn of the whole tree there: every limb of it that comes near, each at
        // the size the game draws a limb of its thickness near the eye.
        let up = limb.a.y + cube_sim::plants::Limb::UNDER + 1.0;
        let thick = limb.ra.max(limb.rb);
        let mut drawn = [std::collections::HashSet::new(), std::collections::HashSet::new()];
        for other in form.limbs.iter().filter(|o| o.ra.max(o.rb) >= 0.16 && (o.a - limb.a).with_y(0.0).length() < thick + 0.3 && o.a.y.min(o.b.y) < up + 0.3 && o.a.y.max(o.b.y) > up - 0.3) {
            let wide = other.ra.max(other.rb) > 0.5;
            other.fill_span(if wide { 0.15 } else { 0.1 }, None, (up - 0.3, up + 0.3), |c| {
                drawn[wide as usize].insert(c);
            });
        }
        let axis = limb.a.lerp(limb.b, ((up - limb.a.y) / (limb.b.y - limb.a.y).max(0.01)).clamp(0.0, 1.0));
        for k in 0..16 {
            let way = Vec3::new((k as f32 * std::f32::consts::TAU / 16.0).cos(), 0.0, (k as f32 * std::f32::consts::TAU / 16.0).sin());
            // (From a little way out, and only this trunk's own wood: a rock or the next
            // tree in the way is not this trunk's collision.)
            let out = thick * 5.0 + 0.5;
            let first = |hit: &dyn Fn(Vec3) -> bool| (0..(out * 100.0) as i32).map(|i| out - i as f32 * 0.01).find(|d| hit(Vec3::new(axis.x, up, axis.z) + way * *d));
            let (stopped, shown) = (first(&|p| world.bit((p / cube_voxel::BIT).floor().as_ivec3()) == limb.bark), first(&|p| drawn[0].contains(&(p / 0.1).floor().as_ivec3()) || drawn[1].contains(&(p / 0.15).floor().as_ivec3())));
            if let (Some(stopped), Some(shown)) = (stopped, shown) {
                gaps.push(stopped - shown);
            }
        }
    }
    // And what lies on the ground, from above: how far the top a body stands on is from the
    // top that is drawn, along every fallen trunk. (A body was seen to sink into one.)
    let mut tops: Vec<f32> = Vec::new();
    for limb in form.limbs.iter().filter(|l| l.roots == 0 && l.ra.max(l.rb) >= 0.2 && (l.b.y - l.a.y).abs() < 0.3 * l.a.distance(l.b) && l.a.x.abs() < inside && l.a.z.abs() < inside && l.a.y.min(l.b.y) < land.height((l.a.x / CELL).floor() as i32, (l.a.z / CELL).floor() as i32) as f32 * CELL + 1.5) {
        let g = limb.near();
        let mut drawn = std::collections::HashSet::new();
        limb.fill_span(g, None, (f32::MIN, f32::MAX), |c| {
            drawn.insert(c);
        });
        for part in [0.25, 0.5, 0.75] {
            let p = limb.a.lerp(limb.b, part);
            let from = p.y + limb.ra + 0.5;
            let first = |hit: &dyn Fn(Vec3) -> bool| (0..((limb.ra * 2.0 + 0.6) * 100.0) as i32).map(|i| from - i as f32 * 0.01).find(|y| hit(Vec3::new(p.x, *y, p.z)));
            if let (Some(stood), Some(shown)) = (first(&|q| world.bit((q / cube_voxel::BIT).floor().as_ivec3()) == limb.bark), first(&|q| drawn.contains(&(q / g).floor().as_ivec3()))) {
                tops.push(shown - stood);
            }
        }
    }
    tops.sort_by(f32::total_cmp);
    gaps.sort_by(f32::total_cmp);
    let mut checks = Checks(Vec::new());
    if !tops.is_empty() {
        // (All but the place in ten where other wood stands over it: its own bough, the stump beside it.)
        let (sunk, over) = (tops[tops.len() - 1] * 100.0, tops[tops.len() / 10] * 100.0);
        println!("logs: {} places on {} fallen trunks · a body stands from {:.0} cm above the wood that is drawn to {:.0} cm down in it", tops.len(), tops.len() / 3, -over, sunk);
        checks.bar("logs sunk", sunk, sunk < 8.0, "a body sinks no more than 8 cm into a fallen trunk");
        checks.bar("logs over", -over, -over < 8.0, "nor stands more than 8 cm over one");
    }
    if gaps.is_empty() {
        checks.bar("trunks", 0.0, false, "some trunks near the start");
        checks.finish()
    }
    let (n, at) = (gaps.len(), |part: f32| gaps[((gaps.len() - 1) as f32 * part) as usize] * 100.0);
    let mut off: Vec<f32> = gaps.iter().map(|g| g.abs()).collect();
    off.sort_by(f32::total_cmp);
    let mean = off[off.len() / 2] * 100.0;
    println!("trunks: {} sides of {} trunks · what stops a body is {mean:.1} cm from what is drawn, at the middle · from {:.0} cm inside it to {:.0} cm outside (the 5th and 95th of a hundred)", n, n / 16, at(0.05), at(0.95));
    checks.bar("trunks off", mean, mean < 4.0, "under 4 cm at the middle");
    checks.bar("trunks inside", at(0.05), at(0.05) > -8.0, "a body goes no more than 8 cm into wood that is drawn");
    checks.bar("trunks outside", at(0.95), at(0.95) < 8.0, "a body is stopped no more than 8 cm short of it");
    checks.finish()
}

/// What making the world costs, on one thread: the sixteen regions round the start made one
/// at a time and every chunk of them meshed, as the game does it.
fn cost_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    cube_sim::terrain::fill_cost();
    let started = std::time::Instant::now();
    for rz in -2..2 {
        for rx in -2..2 {
            cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
        }
    }
    let (all, parts) = (started.elapsed().as_secs_f64() * 1000.0 / 16.0, cube_sim::terrain::fill_cost().map(|s| s * 1000.0 / 16.0));
    let chunks: Vec<IVec3> = world.chunks.keys().copied().collect();
    let started = std::time::Instant::now();
    let faces: usize = chunks.iter().map(|&c| [false, true].map(|liquid| cube_voxel::mesh::mesh_chunk(&world, c, liquid).indices.len() / 6).iter().sum::<usize>()).sum();
    let mesh = started.elapsed().as_secs_f64() * 1000.0 / chunks.len().max(1) as f64;
    println!("cost: a region's cells {all:.0} ms (ground {:.0}, what stands {:.0} of which wood {:.0} and leaf {:.0}, cover {:.0}) · a chunk's mesh {mesh:.2} ms ({} chunks, {:.2}M faces) · {} cells broken to bits, {:.1} MB", parts[0], parts[1], parts[3], parts[4], parts[2], chunks.len(), faces as f64 / 1e6, world.broken(), world.broken() as f64 * 141.0 / 1e6);
    std::process::exit(0)
}

/// `cargo world quarry`: a boulder near the start cut through at its middle with a pick's
/// bites, and what follows judged: it becomes cells, stone is taken, what is over the cut
/// comes loose and comes down, its stone goes back into the world, and nothing is left in
/// the air.
fn quarry_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), mut form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    let inside = (2 * r) as f32 * CELL - 12.0;
    let Some(stone) = form.stones.iter().filter(|s| s.radii.min_element() > 0.7 && s.middle.x.abs() < inside && s.middle.z.abs() < inside).min_by(|a, b| a.middle.length().total_cmp(&b.middle.length())).cloned() else { fail("no boulder to quarry near the start") };
    let mut checks = Checks(Vec::new());
    let mut loose = cube_sim::loose::Loose::default();
    let stuff = Some(cube_sim::items::Stuff::Stone);
    let reach = stone.radii.x.max(stone.radii.z) + 0.5;
    let (mut bites, mut worked, mut taken) = (0, 0, 0u32);
    let n = (reach / 0.3).ceil() as i32;
    // A row of the pick's bites across its top: it is cells from the first, and stone goes.
    for step in -n..=n {
        let at = stone.middle + Vec3::new(step as f32 * 0.3, stone.radii.y * 0.5, 0.0);
        let cut = loose.cut(&mut world, at, 0.42, Vec3::X, &[&form], stuff);
        if !cut.stones.is_empty() {
            form.stones.retain(|s| s.middle != stone.middle);
        }
        (worked, taken, bites) = (worked + cut.worked, taken + cut.taken[cube_sim::items::Stuff::Stone as usize], bites + 1);
    }
    // (A boulder lies in the ground: cut through, its top rests on the bank beside it and
    // goes nowhere, which is right. So the earth is dug from round and under it with the
    // shovel, a level at a time, until it comes down. One that lies on rock does not, and
    // is not held to how it falls.)
    let earth = Some(cube_sim::items::Stuff::Earth);
    // (Its stone, in bits, round where it lies: what there is before it comes down and after.)
    let stone_bits = |world: &World| -> usize {
        let (lo, hi) = (((stone.middle - stone.radii - 4.0) / cube_voxel::BIT).floor().as_ivec3(), ((stone.middle + stone.radii + 4.0) / cube_voxel::BIT).floor().as_ivec3());
        (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))).filter(|b| matches!(world.bit(*b), Block::Shard | Block::Rock)).count()
    };
    let before = stone_bits(&world);
    let wide = ((reach + 1.0) / 0.3).ceil() as i32;
    'digging: for down in 1..=7 {
        for row in -wide..=wide {
            for step in -wide..=wide {
                let at = stone.middle + Vec3::new(step as f32 * 0.3, 0.3 - down as f32 * 0.4, row as f32 * 0.3);
                loose.cut(&mut world, at, 0.42, Vec3::X, &[&form], earth);
                bites += 1;
                if loose.pieces.iter().map(|p| p.cells.len()).sum::<usize>() > 20 {
                    break 'digging;
                }
            }
        }
    }
    checks.bar("quarry wrought", worked as f32, worked > 50, "the boulder became cells");
    checks.bar("quarry taken", taken as f32, taken > 500, "stone was cut out of it (bits)");
    let cells = loose.pieces.iter().map(|p| p.cells.len()).sum::<usize>();
    if cells <= 20 {
        println!("quarried a boulder {:.1} by {:.1} by {:.1} m, {:.0} m from the start: {bites} bites, {worked} cells made wrought, {taken} bits of stone taken · it lies on rock, and the shovel did not bring it down", stone.radii.x * 2.0, stone.radii.y * 2.0, stone.radii.z * 2.0, stone.middle.length());
        checks.finish()
    }
    let mut ticks = 0;
    while !loose.pieces.is_empty() && ticks < 20 * TICKS {
        loose.step(&mut world, cube_sim::DT);
        ticks += 1;
    }
    let took = ticks as f32 * cube_sim::DT;
    checks.bar("quarry seconds", took, took < 10.0, "it has come to rest within ten seconds");
    let (put, lost, fast) = (loose.landed.iter().map(|l| l.put).sum::<usize>(), loose.landed.iter().map(|l| l.lost).sum::<usize>(), loose.landed.iter().map(|l| l.speed).fold(0.0, f32::max));
    // (By its stone, not by cells put: what comes loose has earth and grass on it, and
    // broken on landing it is counted lump by lump. 98 to 100% on seven seeds; it was a half
    // to two thirds before what breaks kept its stuff and lay square: engine.md.)
    // (What lies in the grid, and what lies at rest as itself, whose stone is its own and
    // is in the world only as where it is: `loose::Rested`.)
    let own = loose.rested.iter().flat_map(|r| r.cells.values()).map(|p| match p { cube_sim::loose::Part::Whole(b) => if b.grown() == Block::Rock { 125 } else { 0 }, cube_sim::loose::Part::Bits(bits) => bits.iter().filter(|b| b.grown() == Block::Rock).count() }).sum::<usize>();
    let kept = (stone_bits(&world) + own) as f32 / before.max(1) as f32;
    checks.bar("quarry stone kept", kept, kept > 0.95, "nineteen twentieths of its stone is in the world again");
    // And nothing of it is left on nothing.
    let (lo, hi) = (((stone.middle - stone.radii - 3.0) / CELL).floor().as_ivec3(), ((stone.middle + stone.radii + 3.0) / CELL).floor().as_ivec3());
    loose.loosen(&mut world, lo, hi, Vec3::X);
    checks.bar("quarry hanging", loose.pieces.len() as f32, loose.pieces.is_empty(), "nothing is left in the air");
    println!("quarried a boulder {:.1} by {:.1} by {:.1} m, {:.0} m from the start: {bites} bites, {worked} cells made wrought, {taken} bits of stone taken · {cells} cells came loose, down in {took:.1} s at {fast:.1} m/s at the most, {put} cells put back in {} landings, {:.0}% of its stone there after, {lost} whole cells of stone found no room", stone.radii.x * 2.0, stone.radii.y * 2.0, stone.radii.z * 2.0, stone.middle.length(), loose.landed.len(), kept * 100.0);
    checks.finish()
}

/// `cargo world joined`: every tree near the start as the cells it would be once cut into,
/// and whether all its wood is one body with its foot (joined by faces, edges or corners,
/// as what holds a thing up is told). One that is not drops a part of itself the moment an
/// axe touches it: a birch was seen standing with a gap in its trunk and its top in the air.
/// `cargo world roots`: whether what stands on the land meets it. For every rooted tree
/// near the start, each column of bits (5 cm) round its foot that has its wood beginning
/// no higher than its foot: how far the lowest of that wood is over the
/// ground under it. A root should go into the ground wherever the ground is; on a slope
/// the downhill one hung over it (the developer saw one, 2026-11-13).
fn roots_command(args: &Args) -> ! {
    let (world, _, form) = grown(args);
    let inside = (2 * cube_sim::terrain::REGION) as f32 * CELL - 16.0;
    let bit = cube_voxel::BIT;
    let wood = |b: Block| b.wood();
    let (mut trees, mut hung, mut worst) = (0, 0, (0.0f32, Vec3::ZERO));
    for limb in form.limbs.iter().filter(|l| l.roots > 0 && l.a.x.abs() < inside && l.a.z.abs() < inside) {
        let foot = limb.a + Vec3::Y * cube_sim::plants::Limb::UNDER;
        let reach = limb.ra * 4.6 + 0.2;
        let (lo, hi) = (((foot - reach) / bit).floor().as_ivec3(), ((foot + reach) / bit).floor().as_ivec3());
        let mut gap = 0.0f32;
        let mut what = (Block::Air, Block::Air, 0.0f32, 0.0f32);
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                // The lowest wood of this column near its foot, and what is under it.
                // (No further out than its roots reach: what is in the corners of the square
                // round it is a neighbour's.)
                if (Vec3::new(x as f32 + 0.5, 0.0, z as f32 + 0.5) * bit).distance(foot.with_y(0.0)) > reach {
                    continue;
                }
                // (Wood that begins no higher than its foot: a root, and not the trunk of a
                // tree that leans or a neighbour's bough, which are over the ground by rights.)
                let (top, floor) = (((foot.y + 0.05) / bit) as i32, ((foot.y - 3.0) / bit) as i32);
                let Some(low) = (floor..top).find(|y| wood(world.bit(IVec3::new(x, *y, z)))) else { continue };
                // (Water is ground enough: a root may stand in it.)
                let under = (1..60).find(|d| { let b = world.bit(IVec3::new(x, low - d, z)); b.solid() || b == Block::Water }).unwrap_or(60);
                if (under - 1) as f32 * bit > gap {
                    gap = (under - 1) as f32 * bit;
                    what = (world.bit(IVec3::new(x, low - 1, z)), world.bit(IVec3::new(x, low - under, z)), (low as f32 * bit - foot.y), (Vec3::new(x as f32, 0.0, z as f32) * bit).distance(foot.with_y(0.0)));
                }
            }
        }
        if gap > 0.1 && std::env::var_os("WORLD_VERBOSE").is_some() {
            println!("hung {gap:.2} m: just under it {:?}, then {:?} · the wood {:.2} m from its foot's height, {:.2} m out · trunk {:.2} m thick", what.0, what.1, what.2, what.3, limb.ra * 2.0);
        }
        trees += 1;
        hung += (gap > 0.1) as usize;
        if gap > worst.0 {
            worst = (gap, foot);
        }
    }
    println!("roots: {trees} rooted trees · {hung} with wood of their foot more than 10 cm over the ground · the worst {:.2} m, at {:.1},{:.1}", worst.0, worst.1.x, worst.1.z);
    let mut checks = Checks(Vec::new());
    checks.bar("roots hung", hung as f32 / trees.max(1) as f32, hung * 8 <= trees, "under an eighth of the trees have wood at their foot over the ground, of 1 (it was three in five)");
    checks.bar("roots worst", worst.0, worst.0 <= 0.6, "none by more than 0.6 m");
    checks.finish()
}

fn joined_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    let inside = (2 * r) as f32 * CELL - 16.0;
    let mut trees: std::collections::BTreeMap<u32, Vec<cube_sim::plants::Limb>> = Default::default();
    for limb in &form.limbs {
        trees.entry(limb.tree).or_default().push(*limb);
    }
    let (mut whole, mut broken, mut worst) = (0, 0, (0usize, Vec3::ZERO, 0usize));
    let (mut leafed, mut lacking) = (0, 0);
    for limbs in trees.values().filter(|l| l.iter().any(|l| l.roots > 0 && l.a.x.abs() < inside && l.a.z.abs() < inside)) {
        let mut cells: std::collections::HashSet<IVec3> = Default::default();
        // (As the world has them: a cell of its trunk that another tree's leaf was written
        // over is leaf, and bears nothing.)
        for limb in limbs {
            limb.fill(CELL, |c| {
                let b = world.get(c);
                if b.foliage() {
                    leafed += 1;
                } else if b.wood() || b == Block::Broken && world.bits_at(c).is_some_and(|bits| bits.iter().any(|b| b.wood())) {
                    // (A broken cell with wood in it: one with none is the ground's, broken by something else.)
                    cells.insert(c);
                } else if b == Block::Air {
                    // (Its shape has a cell here and the world has none. Its roots in the
                    // ground are the ground's.)
                    lacking += 1;
                }
            });
        }
        let foot = limbs.iter().find(|l| l.roots > 0).unwrap();
        let Some(start) = cells.iter().copied().min_by_key(|c| ((c.as_vec3() + 0.5) * CELL).distance_squared(foot.a + Vec3::Y * (cube_sim::plants::Limb::UNDER + 0.5)) as i64 * 1000 + c.y as i64) else { continue };
        let (mut seen, mut queue) = (std::collections::HashSet::from([start]), std::collections::VecDeque::from([start]));
        while let Some(c) = queue.pop_front() {
            for d in (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z)))) {
                // (Or through the swell of its foot, which is written to the bit wider than
                // its shape at the size of a cell and is its wood all the same, as a cut
                // takes it, `loose::work`: without it ten clumps of stems each had one cell
                // at the ground that no cell of their own shape touched.)
                let n = c + d;
                let swell = (n.y as f32) * CELL < foot.a.y + 1.0 && (world.get(n) == Block::Broken || world.get(n).wood()) && ((n.as_vec3() + 0.5) * CELL).with_y(0.0).distance(foot.a.with_y(0.0)) < foot.ra.max(foot.rb) * 2.6 + 1.0;
                if (cells.contains(&n) || swell) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        // (Not a root's end lying on the ground by its foot: that is joined to it through
        // the earth, and stays where it is when the tree is cut.)
        let root = |c: &IVec3| (c.y as f32) * CELL < foot.a.y + 0.75 && world.get(*c - IVec3::Y).ground();
        let apart = cells.difference(&seen).filter(|c| !root(c)).count();
        if apart > 0 && std::env::var_os("WORLD_VERBOSE").is_some() {
            for c in cells.difference(&seen).filter(|c| !root(c)).take(2) {
                let round: Vec<String> = (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| IVec3::new(x, y, z)))).map(|d| format!("{:?}", world.get(*c + d)).chars().take(2).collect()).collect();
                println!("apart: {:?} at {c:?}, {:.1} m up its tree · round it {}", world.get(*c), c.y as f32 * CELL - foot.a.y, round.join(" "));
            }
        }
        if apart == 0 {
            whole += 1;
        } else {
            broken += 1;
            if apart > worst.0 {
                worst = (apart, foot.a, cells.len());
            }
        }
    }
    println!("joined: {} trees as cells · {whole} of one body · {broken} with wood not joined to their foot (the worst {} of its {} cells, standing at {:.1},{:.1}) · {leafed} cells of wood that are leaf in the world, {lacking} that are air there", whole + broken, worst.0, worst.2, worst.1.x, worst.1.z);
    let mut checks = Checks(Vec::new());
    checks.bar("joined apart", broken as f32, broken == 0, "no tree has wood that is not joined to its foot");
    checks.finish()
}

/// A tree near the start is cut through at the foot, and what follows is judged: it comes
/// loose as one piece, goes over, and lies on the ground as cells with nothing left in the air.
fn fell_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), mut form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    // A grown tree well inside what was made: a trunk as thick as a person can cut through.
    let inside = (2 * r) as f32 * CELL - 12.0;
    // (`cargo world fell X,Z`: the tree nearest a place, whatever its girth: the one the game's
    // player would be chopping there.)
    let place = args.file.as_deref().and_then(|f| f.split_once(',')).and_then(|(x, z)| Some(Vec3::new(x.parse().ok()?, 0.0, z.parse().ok()?)));
    let chosen = place.and_then(|at| form.limbs.iter().copied().filter(|l| l.roots > 0 && l.b.y - l.a.y > 3.0 && l.a.x.abs() < inside - 16.0 && l.a.z.abs() < inside - 16.0).min_by(|a, b| a.a.with_y(0.0).distance(at).total_cmp(&b.a.with_y(0.0).distance(at))));
    let Some(trunk) = chosen.or_else(|| form.limbs.iter().copied().filter(|l| l.roots > 0 && (0.18..0.4).contains(&l.ra) && (l.b.y - l.a.y) > 5.0 && l.a.x.abs() < inside && l.a.z.abs() < inside).min_by(|a, b| a.a.length().total_cmp(&b.a.length()))) else { fail("no tree to fell near the start") };
    // (All of it, not its first length of trunk: that is 6 m of a tree that stands 15.)
    let tall = form.limbs.iter().filter(|l| l.tree == trunk.tree).map(|l| l.b.y).fold(trunk.b.y, f32::max) - trunk.a.y;
    let mut checks = Checks(Vec::new());
    let mut loose = cube_sim::loose::Loose::default();
    let solid = |world: &World, of: &dyn Fn(Block) -> bool| -> usize { world.chunks.values().map(|c| (0..cube_voxel::CHUNK.pow(3)).filter(|i| of(c.get(IVec3::new(i % 32, i / 1024, i / 32 % 32)))).count()).sum() };
    let wood_before = solid(&world, &|b| b.wood());
    // Through it a metre up, in bites, from one side to the other.
    // (Where the trunk is at that height, not over its foot: one that leans is a bite's width
    // aside by then, and on seed 1 sixty-three bites went past it.)
    let up = trunk.a + (trunk.b - trunk.a) * ((cube_sim::plants::Limb::UNDER + 1.0) / (trunk.b.y - trunk.a.y));
    let started = std::time::Instant::now();
    let (mut bites, mut worked) = (0, 0);
    // With an axe, as a person does: blows at its trunk until it is through (`Loose::chop`).
    // It must stand until the last of the blows its girth asks.
    // (As thick as it is where it is cut: `Loose::chop` cuts 1.4 m up it.)
    let needs = cube_sim::loose::BLOWS(trunk.ra + (trunk.rb - trunk.ra) * (1.4 / (trunk.b.y - trunk.a.y).max(0.1)).min(0.8));
    let mut early = 0;
    for blow in 1..=12 {
        let cut = loose.chop(&mut world, up - Vec3::X * (trunk.ra + 0.2), Vec3::X, &[&form]);
        worked += cut.worked;
        // (As the game does: what is cells now is no longer among the shapes.)
        form.limbs.retain(|l| !cut.trees.contains(&(0, l.tree)));
        form.clumps.retain(|c| !cut.trees.contains(&(0, c.tree)));
        bites += 1;
        if !loose.pieces.is_empty() {
            early = needs.saturating_sub(blow);
            break;
        }
    }
    checks.bar("fell blows", bites as f32, early == 0 && bites as u32 <= needs, "it stands until the last of the blows its girth asks, and no longer");
    let cutting = started.elapsed().as_secs_f32() * 1000.0 / bites as f32;
    // (The tree, and with a great one a bough or two that were its own only by touching.)
    checks.bar("fell pieces", loose.pieces.len() as f32, (1..=3).contains(&loose.pieces.len()), "it comes loose, in three pieces at the most");
    let cells = loose.pieces.iter().map(|p| p.cells.len()).max().unwrap_or(0);
    checks.bar("fell piece cells", cells as f32, cells > 200, "the tree, not a chip of it");
    let started = std::time::Instant::now();
    let (mut ticks, mut worst) = (0u32, 0.0f32);
    while !loose.pieces.is_empty() && ticks < 30 * TICKS {
        let began = std::time::Instant::now();
        loose.step(&mut world, cube_sim::DT);
        worst = worst.max(began.elapsed().as_secs_f32() * 1000.0);
        ticks += 1;
    }
    let falling = started.elapsed().as_secs_f32() * 1000.0;
    let took = ticks as f32 * cube_sim::DT;
    checks.bar("fell seconds", took, (0.8..12.0).contains(&took), "between 0.8 and 12 s to come down");
    // (The tree itself: the most of it that was put back at once. Its top striking is noted
    // first, with nothing put, and its twigs after.)
    let struck = loose.landed.iter().map(|l| l.speed).fold(0.0, f32::max);
    let down = loose.landed.iter().copied().max_by_key(|l| l.put).map(|l| cube_sim::loose::Landed { speed: struck, ..l });
    let kept = down.map_or(0.0, |d| d.put as f32 / d.cells.max(1) as f32);
    // (Half, since its twigs give way under it and are crushed where they go into the
    // ground: it was seven in ten while it lay propped on them, and before that eight.)
    // (What does not go back is leaf come down into another
    // tree's leaf or onto the ground, a fifth of one tree that fell into a thicket, and no
    // loss. What must all go back is the wood, which is held to its own bar.)
    checks.bar("fell kept", kept, kept > 0.4, "at least two fifths of its cells are put back");
    let lost = loose.landed.iter().map(|l| l.lost).sum::<usize>() as f32 / down.map_or(1, |d| d.cells.max(1)) as f32;
    // (A twentieth: its lesser boughs give way under it now, and what of them is driven
    // into the ground is lost. It was a hundredth while they held it up.)
    checks.bar("fell wood lost", lost, lost < 0.05, "under a twentieth of its cells are wood that found no room");
    // Lying: its middle is low, where a standing tree's is half its height up.
    // (Half of all it stood, not of its first length of trunk, which is 6 m of a tree that
    // stands 15: by that two trees lying right on seeds 11 and 13 were failed.)
    let ground = land.height((trunk.a.x / CELL) as i32, (trunk.a.z / CELL) as i32) as f32 * CELL;
    let high = down.map_or(99.0, |d| d.at.y - ground);
    checks.bar("fell lies", high, high < tall * 0.5, "its middle under half the height it stood");
    // And its trunk on the ground from end to end: how high each end of the trunk it was cut
    // from now is over the ground under it, by where the tree came to rest (`Thing.lies`).
    // (A felled birch lay level a metre and a half up, its butt on its stump and its top on
    // its own boughs, and passed every bar here: the developer saw it, 2026-11-13.)
    let ends = loose.things.iter().filter_map(|t| t.lies.map(|l| (t, l))).find(|(t, _)| t.limbs.iter().any(|l| l.a == trunk.a)).map(|(t, (pivot, shift, turn))| {
        let top = t.limbs.iter().filter(|l| l.ra.max(l.rb) > trunk.ra * 0.5).map(|l| l.b).fold(trunk.b, |a, b| if b.y > a.y { b } else { a });
        let now = |p: Vec3| pivot + shift + turn * (p - pivot);
        // (The air under it, by the world as it now is: from just under its wood down to
        // whatever is there. By the land's height where it lies, a trunk lying on a bank
        // that the land's own figure put lower read 0.8 m up along all its length.)
        let thick = trunk.ra.max(trunk.rb) + 0.15;
        let over = |p: Vec3| { let q = now(p) - Vec3::Y * thick; if world.get((q / CELL).floor().as_ivec3()).solid() { 0.0 } else { world.raycast(q, Vec3::NEG_Y, 30.0).unwrap_or(30.0) } };
        (over(trunk.a + (trunk.b - trunk.a) * 0.25), over(trunk.a.lerp(top, 0.5)), over(top))
    });
    if let Some((butt, middle, top)) = ends {
        println!("under its trunk there is {butt:.1} m of air near its butt, {middle:.1} m at its middle, {top:.1} m at its top");
        // (On the ground somewhere along it and near it at its middle. Not at every point: a
        // trunk is stiff, and felled up a bank or across a hump it lies on two places with
        // its butt or its top in the air, which is right: `LOOSE_TRACE=1` says what each
        // time it was stopped by. Propped on its own twigs, as every one was, its middle
        // was 1.4 to 2.4 m up and no part of it within a metre.)
        let low = butt.min(middle).min(top);
        checks.bar("fell trunk down", low.max(middle - 1.5), low < 0.3 && middle < 1.8, "its trunk is on the ground somewhere along it, and within 1.8 m of it at its middle");
    }
    // And over: how far from upright it has come to rest, which a height alone does not say
    // (a tree put back standing where it stood has its middle under half its height too,
    // and passed, 2026-11-13). Forty degrees: one felled up a steep bank rests at 45.
    let over = loose.things.iter().filter_map(|t| t.lies).map(|(_, _, turn)| (turn * Vec3::Y).angle_between(Vec3::Y).to_degrees()).fold(0.0, f32::max);
    checks.bar("fell over", over, over > 40.0, "it has gone over by more than 40 degrees");
    // And nothing of it is left in the air: looked through again, nothing more comes loose.
    let (lo, hi) = (((trunk.a - Vec3::splat(14.0)) / CELL).as_ivec3(), ((trunk.b + Vec3::splat(14.0)) / CELL).as_ivec3());
    loose.loosen(&mut world, lo.with_y((ground / CELL) as i32 - 2), hi, Vec3::X);
    checks.bar("fell hanging", loose.pieces.len() as f32, loose.pieces.is_empty(), "nothing left loose in the air");
    let wood_after = solid(&world, &|b| b.wood());
    println!(
        "felled a tree {:.0} m tall, {:.0} cm through, {:.0} m from the start: {bites} blows ({cutting:.1} ms each, {worked} cells made wrought) · a piece of {cells} cells came down in {took:.1} s at {:.1} m/s ({falling:.0} ms in all, worst step {worst:.1} ms) · {:.0}% of it put back · wood cells {wood_before} to {wood_after}",
        tall,
        trunk.ra * 200.0,
        trunk.a.with_y(0.0).length(),
        down.map_or(0.0, |d| d.speed),
        kept * 100.0
    );
    checks.finish()
}

/// The land round the start made from the seed, with the shapes of what grows on it.
fn grown(args: &Args) -> (World, Land, cube_sim::plants::Form) {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    (world, land, form)
}

/// `cargo world lying`: a tree felled, limbed and bucked, and of each log that lies at its
/// own angle ([`cube_sim::loose::Rested`]) how far the top of what stops a body is from the
/// top of the log as it is drawn, at every 5 cm of it.
fn lying_command(args: &Args) -> ! {
    use cube_sim::loose::Loose;
    use cube_voxel::BIT;
    let (mut world, _, mut form) = grown(args);
    let inside = (2 * cube_sim::terrain::REGION) as f32 * CELL - 12.0;
    let within = |p: Vec3| p.x.abs() < inside && p.z.abs() < inside;
    let Some(trunk) = form.limbs.iter().copied().filter(|l| l.roots > 0 && (0.18..0.4).contains(&l.ra) && (l.b.y - l.a.y) > 5.0 && within(l.a)).min_by(|a, b| a.a.length().total_cmp(&b.a.length())) else { fail("no tree to fell near the start") };
    let mut loose = Loose::default();
    let up = trunk.a + (trunk.b - trunk.a) * ((cube_sim::plants::Limb::UNDER + 1.0) / (trunk.b.y - trunk.a.y));
    for blow in 0..60 {
        if !loose.rested.is_empty() {
            break;
        }
        let lies = loose.things.iter().any(|t| t.lies.is_some() && t.limbs.first().is_some_and(|l| l.a == trunk.a));
        // (As `hunks` aims: at cut wood that is not a tree still standing.)
        let line = |p: Vec3, a: Vec3, b: Vec3| (a + (b - a) * ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0)).distance(p);
        let stands = |p: Vec3| loose.things.iter().any(|t| t.lies.is_none() && t.limbs.iter().any(|l| line(p, l.a, l.b) < l.ra.max(l.rb) * 2.6 + 0.6));
        let mut wood = world.chunks.iter().flat_map(|(at, c)| (0..cube_voxel::CHUNK.pow(3)).map(|i| IVec3::new(i % 32, i / 1024, i / 32 % 32)).filter(|l| { let b = c.get(*l); b.wood() && !b.shaped() }).map(|l| ((*at * cube_voxel::CHUNK + l).as_vec3() + 0.5) * CELL).collect::<Vec<_>>()).filter(|p| (1.5..30.0).contains(&p.with_y(0.0).distance(trunk.a.with_y(0.0))) && !stands(*p)).collect::<Vec<_>>();
        wood.sort_by(|a, b| (a.x, a.z, a.y).partial_cmp(&(b.x, b.z, b.y)).unwrap());
        let (at, way) = match wood.get(blow * 37 % wood.len().max(1)).copied() {
            Some(at) if lies => (at, Vec3::NEG_Y),
            _ => (up - Vec3::X * (trunk.ra + 0.2), Vec3::X),
        };
        let cut = loose.chop(&mut world, at, way, &[&form]);
        form.limbs.retain(|l| !cut.trees.contains(&(0, l.tree)));
        form.clumps.retain(|c| !cut.trees.contains(&(0, c.tree)));
        if std::env::var_os("LYING_TRACE").is_some() {
            println!("blow {blow}: lies {lies}, wood {}, at {at:.1?}, things {}, pieces {}", wood.len(), loose.things.len(), loose.pieces.len());
        }
        let mut ticks = 0;
        while !loose.pieces.is_empty() && ticks < 30 * TICKS {
            loose.step(&mut world, cube_sim::DT);
            ticks += 1;
        }
    }
    let mut checks = Checks(Vec::new());
    // (Over, under: the worst the body stands above the log drawn and in it; bare: places
    // of a log with nothing to stand on.)
    let (mut over, mut under, mut sum, mut places, mut bare, mut tilt, mut covered, mut wood) = (0.0f32, 0.0f32, 0.0f32, 0u32, 0u32, 0.0f32, 0u32, 0u32);
    for log in &loose.rested {
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for cell in log.cells.keys() {
            for corner in 0..8 {
                let p = log.now((*cell + IVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1)).as_vec3() * CELL);
                (lo, hi) = (lo.min(p), hi.max(p));
            }
        }
        tilt = tilt.max((log.turn * Vec3::X).y.abs().asin().to_degrees());
        // How much of its wood is where something else is: every 5 cm of it.
        for cell in log.cells.keys() {
            for i in 0..125 {
                let bit = (log.now((*cell * 5 + IVec3::new(i % 5, i / 25, i / 5 % 5)).as_vec3() * BIT + BIT / 2.0) / BIT).floor().as_ivec3();
                let mine = loose.owner.get(&bit.div_euclid(IVec3::splat(cube_voxel::SPLIT))) == Some(&log.id) && world.bit(bit).whole();
                let b = world.bit(bit);
                (covered, wood) = (covered + (!mine && b.solid() && !b.foliage() && !b.shaped()) as u32, wood + 1);
            }
        }
        let was = (over, under, bare, places);
        let (lo, hi) = ((lo / BIT).floor().as_ivec3(), (hi / BIT).ceil().as_ivec3());
        // (Against the log drawn there or 5 cm to any side: at a log's steep side 5 cm
        // across is its whole depth, and that is not a body standing wrong.)
        let at = |x: i32, y: f32, z: i32| Vec3::new((x as f32 + 0.5) * BIT, y, (z as f32 + 0.5) * BIT);
        let top = |x: i32, z: i32| (0..=((hi.y - lo.y) * 5)).map(|i| hi.y as f32 * BIT - i as f32 * 0.01).find(|y| log.cells.contains_key(&(log.was(at(x, *y, z)) / CELL).floor().as_ivec3()));
        let drawn: std::collections::HashMap<(i32, i32), f32> = (lo.z..hi.z).flat_map(|z| (lo.x..hi.x).map(move |x| (x, z))).filter_map(|(x, z)| top(x, z).map(|y| ((x, z), y))).collect();
        for (&(x, z), &here) in &drawn {
            places += 1;
            // (Its own: in a cell it has, and inside it as it lies. Two logs that lie against
            // each other share cells, and a cell says one of them.)
            let own = |y: i32| loose.owner.get(&IVec3::new(x, y, z).div_euclid(IVec3::splat(cube_voxel::SPLIT))) == Some(&log.id) && world.bit(IVec3::new(x, y, z)).whole() && log.cells.contains_key(&(log.was((IVec3::new(x, y, z).as_vec3() + 0.5) * BIT) / CELL).floor().as_ivec3());
            let solid = |y: i32| world.bit(IVec3::new(x, y, z)).solid();
            // Up from just under its top through whatever is there: the top of that is where
            // a body stands. Something else's (the ground it sank in, a log across it) is
            // not this log standing a body wrong.
            let start = ((here - 0.03) / BIT).floor() as i32;
            let stands = if solid(start) {
                let y = (start..start + 40).find(|y| !solid(y + 1)).unwrap_or(start);
                if !own(y) {
                    places -= 1;
                    continue;
                }
                Some(y)
            } else {
                (lo.y..start).rev().find(|y| own(*y))
            };
            match stands {
                Some(y) => {
                    let stands = (y + 1) as f32 * BIT;
                    let gap = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).filter_map(|(dx, dz)| drawn.get(&(x + dx, z + dz))).map(|d| stands - d).min_by(|a, b| a.abs().total_cmp(&b.abs())).unwrap_or(0.0);
                    (over, under, sum) = (over.max(gap), under.max(-gap), sum + gap.abs());
                }
                None => bare += 1,
            }
        }
        if std::env::var_os("LYING_TRACE").is_some() {
            println!("log {} cells {} owned {} tilt {:.0} shift {:.2?} · over {:.0} in {:.0} bare {} of {}", log.id, log.cells.len(), loose.owner.values().filter(|o| **o == log.id).count(), (log.turn * Vec3::X).y.abs().asin().to_degrees(), log.shift, over * 100.0, under * 100.0, bare - was.2, places - was.3);
            (over, under) = (was.0, was.1);
        }
    }
    // A person walked across each log at its middle, from either side, on foot and then
    // with a jump: over it or stopped by it, never in it and never held fast.
    let tuning = cube_sim::body::Tuning::default();
    let (mut walks, mut stepped, mut jumped, mut inside, mut stuck) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for log in &loose.rested {
        let middle = log.now(log.cells.keys().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / log.cells.len().max(1) as f32);
        let across = (log.turn * Vec3::X).cross(Vec3::Y).with_y(0.0).normalize_or(Vec3::Z);
        for (side, jumps) in [(1.0, false), (-1.0, false), (1.0, true), (-1.0, true)] {
            let from = middle - across * side * 2.0;
            let Some(down) = world.raycast(from + Vec3::Y * 3.0, Vec3::NEG_Y, 12.0) else { continue };
            let mut body = cube_sim::body::Body::new(from + Vec3::Y * (3.2 - down), 0.3, 1.75);
            // (Not from inside a tree that stands beside it, nor where one stands in the way:
            // a log is not a metre and a half high.)
            let tall = (-8..=8).any(|i| world.bit(((middle + across * (i as f32 * 0.25) + Vec3::Y * 1.5) / BIT).floor().as_ivec3()).solid());
            if tall || body.blocked(&world, body.pos) {
                continue;
            }
            let (mut far, mut still, mut was) = (f32::MIN, 0u32, 0u32);
            for tick in 0..6 * TICKS {
                let ahead = (body.pos - middle).dot(across) * side;
                // (A jump when the log has stopped him, as a person does.)
                let jump = jumps && body.grounded && tick > 40 && (-1.6..0.5).contains(&ahead) && body.vel.with_y(0.0).dot(across) * side < 0.5;
                let before = body.pos;
                body.step(&world, cube_sim::body::Intent { dir: if tick < 30 { glam::Vec2::ZERO } else { glam::Vec2::new(across.x, across.z) * side }, speed: 2.0, jump, hold: jumps, ..Default::default() }, &tuning);
                was += body.blocked(&world, body.pos) as u32;
                if std::env::var("LYING_TRACE").is_ok_and(|v| v == log.id.to_string()) && tick % 20 == 0 {
                    println!("    {tick}: {:.2?} vel {:.2?} grounded {}", body.pos, body.vel, body.grounded);
                }
                still = if tick > 60 && body.pos.distance(before) < 1e-4 { still + 1 } else { 0 };
                far = far.max((body.pos - middle).dot(across) * side);
            }
            walks += 1;
            inside += was;
            if std::env::var_os("LYING_TRACE").is_some() {
                println!("  walk log {} side {side} jump {jumps}: got {far:.2} m past its middle, {was} ticks inside, {still} ticks still, ends {:.2?} (log middle {middle:.2?})", log.id, body.pos);
            }
            (stepped, jumped) = (stepped + (!jumps && far > 1.0) as u32, jumped + (jumps && far > 1.0) as u32);
            // (Stopped by a log too high to step on is right; stopped on top of it is not.)
            stuck += (far > -0.3 && far <= 1.0 && still > 60) as u32;
        }
    }
    checks.bar("in a log", inside as f32, inside == 0, "ticks a body walking over the logs was inside something (0)");
    checks.bar("held fast", stuck as f32, stuck == 0, "walks that ended stopped on top of a log (0)");
    checks.bar("jumped", jumped as f32 / (walks / 2).max(1) as f32, jumped * 10 >= walks / 2 * 9, "of the walks with a jump, as many got over (0.9)");
    checks.bar("logs", loose.rested.len() as f32, !loose.rested.is_empty(), "the tree came to logs that lie as themselves");
    checks.bar("stands over", over * 100.0, over <= 0.08, "cm at worst a body stands above a log as it is drawn (8)");
    checks.bar("stands in", under * 100.0, under <= 0.08, "cm at worst a body stands in a log as it is drawn (8)");
    checks.bar("in something", covered as f32 / wood.max(1) as f32, covered * 100 <= wood * 3, "of a log's wood, as much lies in the ground or another log (0.03)");
    checks.bar("still falling", loose.pieces.len() as f32, loose.pieces.is_empty(), "every piece came to rest");
    checks.bar("bare", bare as f32 / places.max(1) as f32, bare * 20 <= places, "of a log, as much has nothing to stand on (0.05)");
    println!("{} logs, the steepest {tilt:.0} degrees from level · what a body stands on is {:.1} cm from the log drawn at the mean, {:.0} over and {:.0} in it at worst · {:.1}% of a log bare, {:.1}% of its wood in the ground or another log · walked across {} times: {stepped} over on foot, {jumped} with a jump", loose.rested.len(), sum / places.max(1) as f32 * 100.0, over * 100.0, under * 100.0, bare as f32 / places.max(1) as f32 * 100.0, covered as f32 / wood.max(1) as f32 * 100.0, walks / 2);
    checks.finish()
}

/// A world is its seed and what was done to it (`cube_sim::loose::replay`): edits are made
/// to the land, the land is made again and the edits done again, and the two are compared.
/// `cargo world hunks`: what is carried and built with, made as a person makes it, on real
/// land with no window. A tree near the start takes an axe's blows until nothing more comes
/// of them (felled, limbed, broken into logs, each log split: `Loose::chop`), and a boulder
/// a pick's until it breaks (`Loose::chip`). Held to: its wood and its stone are in hunks,
/// every cell of a hunk is a whole hunk that lies on something, one can be taken up and put
/// down, and the same blows on the same land give the same.
fn hunks_command(args: &Args) -> ! {
    use cube_sim::loose::{CHIPS, HUNK, Loose, hunk_at};
    let (mut world, _, mut form) = grown(args);
    let inside = (2 * cube_sim::terrain::REGION) as f32 * CELL - 12.0;
    let within = |p: Vec3| p.x.abs() < inside && p.z.abs() < inside;
    let Some(trunk) = form.limbs.iter().copied().filter(|l| l.roots > 0 && (0.18..0.4).contains(&l.ra) && (l.b.y - l.a.y) > 5.0 && within(l.a)).min_by(|a, b| a.a.length().total_cmp(&b.a.length())) else { fail("no tree to fell near the start") };
    let Some(stone) = form.stones.iter().filter(|s| s.radii.min_element() > 0.5 && within(s.middle)).min_by(|a, b| a.middle.length().total_cmp(&b.middle.length())).cloned() else { fail("no boulder near the start") };
    let cells = |world: &World, of: &dyn Fn(Block) -> bool| -> Vec<IVec3> { world.chunks.iter().flat_map(|(at, c)| (0..cube_voxel::CHUNK.pow(3)).map(|i| IVec3::new(i % 32, i / 1024, i / 32 % 32)).filter(|l| of(c.get(*l))).map(|l| *at * cube_voxel::CHUNK + l).collect::<Vec<_>>()).collect() };
    let rest = |loose: &mut Loose, world: &mut World| {
        let mut ticks = 0;
        while !loose.pieces.is_empty() && ticks < 30 * TICKS {
            loose.step(world, cube_sim::DT);
            ticks += 1;
        }
    };
    let (mut checks, mut loose) = (Checks(Vec::new()), Loose::default());
    // The tree: at its trunk while it stands, then along it as it lies, a different place
    // each blow, until eight blows running have done nothing more.
    let tall = form.limbs.iter().filter(|l| l.tree == trunk.tree).map(|l| l.b.y).fold(trunk.b.y, f32::max) - trunk.a.y;
    let up = trunk.a + (trunk.b - trunk.a) * ((cube_sim::plants::Limb::UNDER + 1.0) / (trunk.b.y - trunk.a.y));
    // The cut wood that lies round it, away from its stump, with how much each cell holds in
    // bits (a thin trunk is mostly cells part filled): its own, wherever it has come to
    // rest. (Not what stands: the blow that first makes it cells makes cells of a neighbour
    // whose boughs it reaches, and that one is not this tree's timber.)
    let timber = |world: &World, loose: &Loose| -> Vec<(IVec3, usize)> {
        let line = |p: Vec3, a: Vec3, b: Vec3| (a + (b - a) * ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0)).distance(p);
        let stands = |p: Vec3| loose.things.iter().any(|t| t.lies.is_none() && t.limbs.iter().any(|l| line(p, l.a, l.b) < l.ra.max(l.rb) * 2.6 + 0.6));
        let held = |c: &IVec3| if world.get(*c) == Block::Broken { world.bits_at(*c).map_or(0, |b| b.iter().filter(|b| b.wood() && !b.shaped()).count()) } else { 125 };
        let mut wood: Vec<(IVec3, usize)> = cells(world, &|b| b.wood() && !b.shaped() || b == Block::Broken).into_iter().map(|c| (c, (c.as_vec3() + 0.5) * CELL)).filter(|(_, p)| (1.5..30.0).contains(&p.with_y(0.0).distance(trunk.a.with_y(0.0))) && !stands(*p)).map(|(c, _)| (c, held(&c))).filter(|c| c.1 > 0).collect();
        wood.sort_by_key(|c| (c.0.x, c.0.z, c.0.y));
        wood
    };
    let bits = |wood: &[(IVec3, usize)]| wood.iter().map(|w| w.1).sum::<usize>();
    // (`logs`: the wood, in bits, that was in the logs that were split.)
    let (mut did, mut idle, mut logs) = ([0u32; 4], 0, 0usize);
    for blow in 0..160 {
        let mine = |loose: &Loose| loose.things.iter().find(|t| t.limbs.first().is_some_and(|l| l.a == trunk.a)).map(|t| (t.lies.is_some(), t.blows, t.limbed));
        let (before, wood) = (mine(&loose), timber(&world, &loose));
        let (was, hunks) = (bits(&wood), cells(&world, &|b| b == Block::Billet).len());
        // (At its wood where it lies, as a person aims.)
        let down = before.is_some_and(|t| t.0);
        let at = if down { wood.get(blow * 37 % wood.len().max(1)).map_or(up, |c| (c.0.as_vec3() + 0.5) * CELL) } else { up - Vec3::X * (trunk.ra + 0.2) };
        let cut = loose.chop(&mut world, at, if down { Vec3::NEG_Y } else { Vec3::X }, &[&form]);
        form.limbs.retain(|l| !cut.trees.contains(&(0, l.tree)));
        form.clumps.retain(|c| !cut.trees.contains(&(0, c.tree)));
        rest(&mut loose, &mut world);
        let (after, now, split) = (mine(&loose), bits(&timber(&world, &loose)), cells(&world, &|b| b == Block::Billet).len() > hunks);
        // Which of the four things a blow does this one did, if any.
        let stage = if after.map(|t| t.1) != before.map(|t| t.1) { Some(0) } else if before.is_some_and(|t| !t.2) && down { Some(1) } else if split { Some(3) } else if now != was { Some(2) } else { None };
        // (By the simulation's own count of what it took out of the log.)
        logs += if stage == Some(3) { cut.taken[cube_sim::items::Stuff::Wood as usize] as usize } else { 0 };
        idle = if stage.is_some() { 0 } else { idle + 1 };
        if let Some(stage) = stage {
            did[stage] += 1;
        }
        if idle >= 12 {
            break;
        }
    }
    let billets = cells(&world, &|b| b == Block::Billet);
    let left = bits(&timber(&world, &loose));
    let per = HUNK.pow(3) as usize;
    checks.bar("hunks of wood", (billets.len() / per) as f32, billets.len() >= per, "the tree came to hunks");
    // (Two or three from each log that was split: `SPLITS`.)
    let each = billets.len() as f32 / per as f32 / did[3].max(1) as f32;
    checks.bar("hunks a log", each, (1.5..=3.0).contains(&each), "two or three hunks from each log split");
    checks.bar("wood left in logs", left as f32 / (left + logs).max(1) as f32, left * 3 < logs, "under a quarter of the timber is left unsplit, of 1");
    // The boulder: a pick at its top until it breaks.
    // (Its stone to the bit, in cells' worth: its outside is broken cells, part stone.)
    let stone_cells = |world: &World| {
        let (reach, bit) = (stone.radii.max_element() + 0.3, cube_voxel::BIT);
        let (lo, hi) = (((stone.middle - reach) / bit).floor().as_ivec3(), ((stone.middle + reach) / bit).floor().as_ivec3());
        (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))).filter(|b| ((b.as_vec3() + 0.5) * bit).distance(stone.middle) < reach && world.bit(*b).grown() == Block::Rock).count() / 125
    };
    let (whole, needs) = (stone_cells(&world), CHIPS(stone.radii.max_element()));
    // (At its top where that is, each blow: faces are struck off a boulder, and it is lower
    // than it is round; and each blow leaves a hollow.)
    let over = stone.middle + Vec3::Y * (stone.radii.y + 1.0);
    let mut stood = true;
    for blow in 1..=needs {
        stood &= cells(&world, &|b| b == Block::Cobble).is_empty();
        let top = over - Vec3::Y * (world.raycast(over, Vec3::NEG_Y, stone.radii.y * 2.0 + 1.0).unwrap_or(1.15) + 0.05);
        let hit = loose.chip(&mut world, top, Vec3::NEG_Y, &[&form]);
        if hit.is_none() {
            checks.bar("chip struck", blow as f32, false, "every blow at its top is at the boulder");
        }
        form.stones.retain(|s| s.middle != stone.middle);
        rest(&mut loose, &mut world);
    }
    let cobbles = cells(&world, &|b| b == Block::Cobble);
    checks.bar("boulder stood", stood as u32 as f32, stood, "it was whole until its last blow");
    checks.bar("hunks of stone", (cobbles.len() / per) as f32, cobbles.len() >= per, "the boulder came to hunks");
    checks.bar("stone in hunks", cobbles.len() as f32 / whole.max(1) as f32, (0.5..1.2).contains(&(cobbles.len() as f32 / whole.max(1) as f32)), "as much stone in hunks as was in it, of 1");
    checks.bar("boulder left", stone_cells(&world) as f32 / whole.max(1) as f32, stone_cells(&world) * 10 < whole, "under a tenth of it is left where it lay, of 1");
    // Every cell of a hunk is a whole hunk, and every hunk lies on something. (As they
    // are now: two that lay on the boulder came down when it broke, and were looked for
    // where they had been.)
    let billets = cells(&world, &|b| b == Block::Billet);
    let all: Vec<IVec3> = billets.iter().chain(&cobbles).copied().collect();
    let broken = all.iter().filter(|c| hunk_at(&world, **c).is_none()).count();
    let hanging = all.iter().filter_map(|c| hunk_at(&world, *c)).filter(|(corner, _)| !(0..HUNK * HUNK).any(|i| world.get(*corner + IVec3::new(i % HUNK, -1, i / HUNK)).solid())).count() / per;
    if std::env::var_os("WORLD_VERBOSE").is_some() {
        for c in all.iter().filter(|c| hunk_at(&world, **c).is_none()) {
            println!("no hunk: {:?} at {c:?} · under it {:?} · beside it {:?}", world.get(*c), world.get(*c - IVec3::Y), [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].map(|d| world.get(*c + d)));
        }
    }
    checks.bar("cells of no hunk", broken as f32, broken == 0, "every cell of a hunk is part of a whole one");
    checks.bar("hunks in the air", hanging as f32, hanging == 0, "every hunk lies on something");
    // One taken up and put down three metres off.
    let mut carried = 0;
    if let Some(from) = billets.iter().max_by_key(|c| c.y).map(|c| (c.as_vec3() + 0.5) * CELL) {
        let lifted = loose.lift(&mut world, from).map(|l| l.0);
        rest(&mut loose, &mut world);
        let gone = cells(&world, &|b| b == Block::Billet).len() + per == billets.len();
        let to = from + Vec3::new(3.0, 1.5, 0.0);
        let laid = lifted.is_some_and(|b| loose.lay(&mut world, to, b).is_some());
        rest(&mut loose, &mut world);
        let back = cells(&world, &|b| b == Block::Billet);
        carried = (lifted.is_some() && gone && laid && back.len() == billets.len() && back.iter().all(|c| hunk_at(&world, *c).is_some())) as u32;
        if carried == 0 && std::env::var_os("WORLD_VERBOSE").is_some() {
            println!("carried: from {from:.1} lifted {lifted:?} · gone {gone} · laid {laid} at {to:.1} · {} cells then, {} now · {} of no hunk", billets.len(), back.len(), back.iter().filter(|c| hunk_at(&world, **c).is_none()).count());
        }
    }
    checks.bar("carried", carried as f32, carried == 1, "a hunk is taken up, is gone from where it lay, and is put down whole elsewhere");
    // And again from the seed and the journal.
    let (mut again, _, form2) = grown(args);
    cube_sim::loose::replay(&mut again, &[&form2], &loose.journal);
    let differ = [Block::Billet, Block::Cobble].iter().map(|k| { let (a, b): (std::collections::HashSet<IVec3>, std::collections::HashSet<IVec3>) = (cells(&world, &|b| b == *k).into_iter().collect(), cells(&again, &|b| b == *k).into_iter().collect()); a.symmetric_difference(&b).count() }).sum::<usize>();
    checks.bar("replay differs", differ as f32, differ == 0, "the same blows on the same land put every hunk where it was");
    println!("a tree {tall:.0} m tall: felled in {} blows, limbed in {}, bucked in {}, {} logs split · {} hunks of wood · a boulder {:.1} m across broke at the {needs}th blow into {} hunks · {} edits", did[0], did[1], did[2], did[3], billets.len() / per, stone.radii.max_element() * 2.0, cobbles.len() / per, loose.journal.len());
    checks.finish()
}

fn replay_command(args: &Args) -> ! {
    use cube_sim::items::Stuff;
    let (mut world, land, form) = grown(args);
    let inside = (2 * cube_sim::terrain::REGION) as f32 * CELL - 12.0;
    let Some(trunk) = form.limbs.iter().copied().filter(|l| l.roots > 0 && (0.18..0.4).contains(&l.ra) && (l.b.y - l.a.y) > 5.0 && l.a.x.abs() < inside && l.a.z.abs() < inside).min_by(|a, b| a.a.length().total_cmp(&b.a.length())) else { fail("no tree to fell near the start") };
    let mut loose = cube_sim::loose::Loose::default();
    let rest = |loose: &mut cube_sim::loose::Loose, world: &mut World| {
        let mut n = 0;
        while !loose.pieces.is_empty() && n < 60 * TICKS {
            loose.step(world, cube_sim::DT);
            n += 1;
        }
    };
    // A tree felled; holes dug and earth put down, some while it falls; its trunk cut across.
    // (Where the trunk is at that height, not over its foot: one that leans is a bite's width
    // aside by then, and on seed 1 sixty-three bites went past it.)
    let up = trunk.a + (trunk.b - trunk.a) * ((cube_sim::plants::Limb::UNDER + 1.0) / (trunk.b.y - trunk.a.y));
    for _ in 0..12 {
        loose.chop(&mut world, up - Vec3::X * (trunk.ra + 0.2), Vec3::X, &[&form]);
        if !loose.pieces.is_empty() {
            break;
        }
    }
    let ground = |x: f32, z: f32| Vec3::new(x, land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL, z);
    for k in 0..12 {
        for _ in 0..10 {
            loose.step(&mut world, cube_sim::DT);
        }
        let at = ground(trunk.a.x - 6.0 + k as f32, trunk.a.z + 4.0);
        if k % 3 == 2 {
            loose.put(&mut world, at + Vec3::Y * 0.2, 0.3, Block::Dirt);
        } else {
            loose.cut(&mut world, at, 0.5, Vec3::X, &[&form], Some(Stuff::Earth));
        }
    }
    rest(&mut loose, &mut world);
    let lies = loose.things.iter().find_map(|t| t.lies).map(|(pivot, shift, _)| pivot + shift);
    if let Some(at) = lies {
        for k in 0..6 {
            loose.cut(&mut world, at + Vec3::new(0.0, 0.3 - 0.1 * (k / 3) as f32, (k % 3) as f32 * 0.25 - 0.25), 0.3, Vec3::X, &[&form], Some(Stuff::Wood));
        }
    }
    rest(&mut loose, &mut world);
    // Made again from the seed, and the same done to it.
    let started = std::time::Instant::now();
    let (mut again, _, form) = grown(args);
    let made = started.elapsed().as_secs_f32();
    let started = std::time::Instant::now();
    cube_sim::loose::replay(&mut again, &[&form], &loose.journal);
    let replayed = started.elapsed().as_secs_f32() * 1000.0;
    let (mut differ, mut changed) = (0usize, 0usize);
    let (fresh, _, _) = grown(args);
    for (place, chunk) in &world.chunks {
        for i in 0..cube_voxel::CHUNK.pow(3) {
            let l = IVec3::new(i % 32, i / 1024, i / 32 % 32);
            let c = *place * cube_voxel::CHUNK + l;
            differ += (chunk.get(l) != again.get(c) || world.bits_at(c).map(|b| b.to_vec()) != again.bits_at(c).map(|b| b.to_vec())) as usize;
            changed += (chunk.get(l) != fresh.get(c) || world.bits_at(c).map(|b| b.to_vec()) != fresh.bits_at(c).map(|b| b.to_vec())) as usize;
        }
    }
    let mut checks = Checks(Vec::new());
    checks.bar("replay felled", lies.is_some() as u8 as f32, lies.is_some(), "a tree came down and lies");
    checks.bar("replay differs", differ as f32, differ == 0, "0 cells differ from the world as it was played");
    checks.bar("replay changed", changed as f32, changed > 1000, "the edits changed the world at all");
    let bytes = loose.journal.len() * std::mem::size_of::<(u64, cube_sim::loose::Edit)>();
    println!("replay: {} edits ({bytes} bytes) changed {changed} cells · made again from the seed ({made:.1} s) and the edits done again ({replayed:.0} ms): {differ} cells differ", loose.journal.len());
    checks.finish()
}

fn dig_command(args: &Args) -> ! {
    let (mut world, land, mut sim) = living(args);
    let faces = |world: &World, chunk: IVec3| [false, true].map(|liquid| cube_voxel::mesh::mesh_chunk(world, chunk, liquid).indices.len() / 6).iter().sum::<usize>();
    let before: usize = world.chunks.keys().map(|&c| faces(&world, c)).sum();
    let holes = args.seconds as usize;
    let mut rng = cube_sim::rng::Rng::new(args.seed as u64 + 99);
    let (mut dug, mut touched) = (0, std::collections::HashSet::new());
    let started = std::time::Instant::now();
    while dug < holes {
        let (x, z) = (rng.below(land.span as usize * 2) as i32 - land.span, rng.below(land.span as usize * 2) as i32 - land.span);
        if land.biome(x, z) == Biome::Water {
            continue;
        }
        let across = rng.between(0.3, 1.6);
        let middle = Vec3::new(x as f32, land.height(x, z) as f32, z as f32) * CELL;
        // Every fourth is a stone built up instead, unless that would bury a creature.
        let clear = sim.agents.values().all(|a| a.body.pos.distance(middle) > across + 1.0);
        world.ball(middle, across / 2.0, if dug % 4 == 3 && clear { Block::Stone } else { Block::Air });
        touched.insert((middle / CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        dug += 1;
    }
    let digging = started.elapsed().as_secs_f32() * 1000.0 / holes as f32;
    let started = std::time::Instant::now();
    let redrawn: usize = touched.iter().map(|&c| faces(&world, c)).sum();
    let remesh = started.elapsed().as_secs_f32() * 1000.0 / touched.len().max(1) as f32;
    let after: usize = world.chunks.keys().map(|&c| faces(&world, c)).sum();
    for _ in 0..120 * TICKS {
        sim.step(&world);
    }
    let (inside, fell): (u32, u32) = sim.agents.values().fold((0, 0), |(i, f), a| (i + a.log.inside, f + a.log.fell));
    let stuck = sim.agents.values().map(|a| a.log.stuck).sum::<f32>() / (sim.agents.len() as f32 * 120.0);
    println!(
        "dug {holes} holes and stones 0.3 to 1.6 m across: {} cells broke ({:.1} a hole, {:.1} MB) · faces {:.2}M to {:.2}M · {digging:.2} ms a hole, {remesh:.1} ms to draw a changed chunk again ({} faces)",
        world.broken(),
        world.broken() as f32 / holes as f32,
        world.broken() as f32 * 141.0 / 1e6,
        before as f32 / 1e6,
        after as f32 / 1e6,
        redrawn / touched.len().max(1)
    );
    let mut checks = Checks(Vec::new());
    checks.bar("inside-ground", inside as f32, inside == 0, "0 ticks, over two minutes of the creatures living among them");
    checks.bar("fell-out", fell as f32, fell == 0, "0 times");
    checks.bar("stuck", stuck, stuck <= 0.08, "at most 0.08 of the time");
    checks.bar("remesh", remesh, remesh <= 8.0, "at most 8 ms a chunk, half a frame");
    checks.finish()
}

/// Where the ground is easy, steep and cliff: a line for each band of height, and with a
/// file, a picture from above (green easy, yellow steep, red cliff).
fn grade_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let bands = [4.0, 8.0, 12.0, scale::CONIFER_LINE, scale::ROCK_LINE, scale::SNOW_LINE, f32::MAX];
    let mut from = 0.0;
    for (top, (n, [easy, steep, cliff])) in bands.iter().zip(survey::grades(&land, &bands)) {
        let name = if *top == f32::MAX { format!("over {from:.0}m") } else { format!("{from:.0} to {top:.0}m") };
        println!("{name}: {:.0}% of dry land · easy {:.0}% steep {:.0}% cliff {:.0}%", n as f32 / ((land.span * land.span * 4) as f32 * (1.0 - survey::survey(&land).share[0])) * 100.0, easy * 100.0, steep * 100.0, cliff * 100.0);
        from = *top;
    }
    if let Some(file) = &args.file {
        let side = (land.span * 2) as u32;
        let mut picture = image::RgbImage::new(side, side);
        for (x, z, pixel) in picture.enumerate_pixels_mut() {
            let (cx, cz) = (x as i32 - land.span, z as i32 - land.span);
            let lift = (((land.height(cx, cz) - SEA) as f32 * CELL) * 1.6).clamp(0.0, 110.0) as u8;
            *pixel = image::Rgb(match (land.biome(cx, cz), survey::grade(&land, cx, cz)) {
                (Biome::Water, _) => [20, 40, 90],
                (_, 0) => [lift / 2, 110 + lift, lift / 2],
                (_, 1) => [225, 200, 60],
                _ => [220, 50, 40],
            });
        }
        if let Err(e) = picture.save(file) {
            fail(&format!("could not write {file}: {e}"));
        }
        println!("{file}");
    }
    std::process::exit(0)
}

/// The ecology in numbers: for each kind of small life, how much of the island suits it and
/// how much it is about at four hours; then for each animal, where its groups ended up
/// against what it asks for. A kind that fits nowhere, or an animal far from what it wants,
/// is a line that says so.
fn life_command(args: &Args) -> ! {
    use cube_sim::ecology::{Ecology, SMALL_LIFE};
    let (_, land, sim) = living(args);
    let eco = Ecology::new(&land);
    let span = land.span as f32 * CELL;
    let places: Vec<_> = (0..(span as i32)).flat_map(|z| (0..(span as i32)).map(move |x| (x, z))).map(|(x, z)| eco.place(&land, x as f32 * 2.0 - span, z as f32 * 2.0 - span)).collect();
    let mut checks = Checks(Vec::new());
    for niche in SMALL_LIFE {
        let home = places.iter().filter(|p| niche.suits(p) > 0.5).count() as f32 / places.len() as f32;
        let hours: Vec<String> = [6.0, 12.0, 18.0, 23.0].iter().map(|h| format!("{:.0}h {:.0}%", h, niche.about(cube_sim::sky::at(3, *h)) * 100.0)).collect();
        println!("{:<13} at home on {:>4.1}% of the island · about: {}", niche.name, home * 100.0, hours.join(" "));
        checks.bar(niche.name, home, home >= 0.005, "somewhere to live: at least 0.5% of the island");
    }
    for (i, kind) in sim.kinds.iter().enumerate() {
        let homes: Vec<_> = sim.agents.values().filter(|a| a.kind == i).map(|a| eco.place(&land, a.mind.home.x, a.mind.home.y)).collect();
        if homes.is_empty() || kind.id == "player" {
            continue;
        }
        let mean = |f: fn(&cube_sim::ecology::Place) -> f32| homes.iter().map(f).sum::<f32>() / homes.len() as f32;
        println!("{:<13} ×{:<3} {:?} · water {:?}: {:.0}m off · cover {:?}: {:.0}% · hours {:?}", kind.id, homes.len(), kind.habitat, kind.water, mean(|p| p.water), kind.cover, mean(|p| p.cover) * 100.0, kind.hours);
    }
    checks.finish()
}

fn main() {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_default();
    let mut args = Args { seed: terrain::SEED, radius: terrain::RADIUS, seconds: if command == "map" { 60.0 } else { 120.0 }, file: None };
    while let Some(a) = it.next() {
        let mut number = |name: &str| it.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or_else(|| fail(&format!("{name} wants a number")));
        match a.as_str() {
            "--seed" => args.seed = number("--seed") as u32,
            "--radius" => args.radius = number("--radius") as i32,
            "--seconds" | "--holes" => args.seconds = number(&a),
            // (A place may begin with a minus: `-45,-45`.)
            other if (!other.starts_with('-') || other.contains(',')) && args.file.is_none() => args.file = Some(other.into()),
            other => fail(&format!("unknown argument {other}\n{HELP}")),
        }
    }
    match command.as_str() {
        "survey" => survey_command(&args),
        "run" => run_command(&args),
        "map" => map_command(&args),
        "dig" => dig_command(&args),
        "cost" => cost_command(&args),
        "trunks" => trunks_command(&args),
        "fell" => fell_command(&args),
        "joined" => joined_command(&args),
        "roots" => roots_command(&args),
        "quarry" => quarry_command(&args),
        "hunks" => hunks_command(&args),
        "lying" => lying_command(&args),
        "replay" => replay_command(&args),
        "grade" => grade_command(&args),
        "life" => life_command(&args),
        "drawn" => {
            // How far the ground that is drawn is from the ground a body stands on: the cells
            // are what stops a body, and they are drawn as blocks of other sizes at heights
            // of their own (`mesh::eased_under`). Over the land round the start, column by
            // column; and from each column to the next, how high the step is that is drawn
            // against the step a body takes.
            let land = Land::new(args.seed, args.radius);
            let mut world = World::default();
            let r = cube_sim::terrain::REGION;
            for rz in -2..2 {
                for rx in -2..2 {
                    cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
                }
            }
            let reach = 2 * r - 16;
            let drawn = |x: i32, z: i32| {
                let top = land.height(x, z);
                cube_voxel::mesh::eased_under(&world, Vec3::new((x as f32 + 0.5) * CELL, top as f32 * CELL, (z as f32 + 0.5) * CELL)).map(|y| (y, top as f32 * CELL))
            };
            let (mut off, mut steps, mut stood) = (Vec::new(), Vec::new(), Vec::new());
            for z in (-reach..reach).step_by(3) {
                for x in (-reach..reach).step_by(3) {
                    let (Some(here), Some(next)) = (drawn(x, z), drawn(x + 1, z)) else { continue };
                    off.push((here.0 - here.1).abs());
                    steps.push((next.0 - here.0).abs());
                    stood.push((next.1 - here.1).abs());
                }
            }
            if off.is_empty() {
                fail("no ground round the start to measure");
            }
            // (`WORLD_VERBOSE=1`: where the ground is drawn half a metre and more under its
            // cells, with the cells' heights and the drawn tops round each, in cells.)
            if std::env::var_os("WORLD_VERBOSE").is_some() {
                let mut listed = 0;
                for z in -reach..reach {
                    for x in -reach..reach {
                        let Some(here) = drawn(x, z) else { continue };
                        if here.1 - here.0 < 0.5 || land.height(x, z) <= SEA || listed >= 6 {
                            continue;
                        }
                        listed += 1;
                        println!("at {x},{z} ({:.1},{:.1} m) {:?}, slope {}: drawn {:.0} cm under its cells; cells and drawn tops round it:", (x as f32 + 0.5) * CELL, (z as f32 + 0.5) * CELL, land.biome(x, z), land.slope(x, z), (here.1 - here.0) * 100.0);
                        for dz in -3..=3 {
                            println!("   {}", (-3..=3).map(|dx| format!("{:>3}/{:<5.1}", land.height(x + dx, z + dz) - land.height(x, z), drawn(x + dx, z + dz).map_or(f32::NAN, |d| d.0 / CELL - land.height(x, z) as f32))).collect::<Vec<_>>().join(" "));
                        }
                    }
                }
            }
            for all in [&mut off, &mut steps, &mut stood] {
                all.sort_by(f32::total_cmp);
            }
            let at = |all: &Vec<f32>, part: f32| all[((all.len() - 1) as f32 * part) as usize] * 100.0;
            println!("drawn: {} columns · the ground is drawn {:.0} cm from where a body stands on it at the middle, {:.0} cm at the 95th of a hundred, {:.0} cm at the worst", off.len(), at(&off, 0.5), at(&off, 0.95), at(&off, 1.0));
            println!("steps from a column to the next: drawn {:.0} cm at the 95th and {:.0} cm at the worst, the cells' {:.0} cm and {:.0} cm", at(&steps, 0.95), at(&steps, 1.0), at(&stood, 0.95), at(&stood, 1.0));
            // And where a body comes to rest, let fall on each: against the highest ground
            // drawn under any part of it, which is what it should be standing on.
            let (mut rest, mut sunk) = (Vec::new(), 0.0f32);
            let tuning = cube_sim::body::Tuning::default();
            for z in (-reach..reach).step_by(7) {
                for x in (-reach..reach).step_by(7) {
                    let Some(here) = drawn(x, z) else { continue };
                    if land.height(x, z) <= SEA {
                        continue;
                    }
                    let mut body = cube_sim::body::Body::new(Vec3::new((x as f32 + 0.5) * CELL, here.0.max(here.1) + 1.0, (z as f32 + 0.5) * CELL), 0.3, 1.75);
                    for _ in 0..90 {
                        body.step(&world, cube_sim::body::Intent::default(), &tuning);
                    }
                    // (Only where nothing but ground is under it: a trunk or a rock is not this measure's.)
                    let under: Vec<Option<(f32, f32)>> = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).filter(|(dx, dz)| ((*dx as f32 * CELL).abs() - CELL / 2.0 < 0.3) && ((*dz as f32 * CELL).abs() - CELL / 2.0 < 0.3)).map(|(dx, dz)| drawn(x + dx, z + dz)).collect();
                    if !body.grounded || under.iter().any(|u| u.is_none()) || (-2..=2).any(|dz| (-2..=2).any(|dx| (0..8).any(|dy| matches!(world.get(IVec3::new(x + dx, land.height(x + dx, z + dz) + dy, z + dz)), cube_voxel::Block::Wood | cube_voxel::Block::Birch | cube_voxel::Block::Rock | cube_voxel::Block::Broken)))) {
                        continue;
                    }
                    let highest = under.iter().flatten().map(|u| u.0).fold(f32::MIN, f32::max);
                    rest.push((body.pos.y - highest).abs());
                    sunk = sunk.max(here.0 - body.pos.y);
                }
            }
            rest.sort_by(f32::total_cmp);
            if rest.is_empty() {
                fail("no body came to rest on open ground");
            }
            println!("a body let fall on {} places rests {:.0} cm from the highest ground drawn under it at the 95th, {:.0} cm at the 99th, {:.0} cm at the worst ({} of them over 2 cm); under the ground drawn at its middle by {:.0} cm at the worst", rest.len(), at(&rest, 0.95), at(&rest, 0.99), at(&rest, 1.0), rest.iter().filter(|r| **r > 0.02).count(), sunk * 100.0);
            let mut checks = Checks(Vec::new());
            checks.bar("rests on what is drawn", at(&rest, 0.95), at(&rest, 0.95) < 1.0, "within a centimetre at the 95th place of a hundred");
            checks.bar("in the drawn ground", sunk * 100.0, sunk * 100.0 < 1.0, "never under the ground drawn at its middle");
            checks.finish()
        }
        "find" => {
            // The nearest places to the start of a kind: `deep` water to swim in, or a biome.
            let what = args.file.clone().unwrap_or_default();
            let land = Land::new(args.seed, args.radius);
            let mut found = Vec::new();
            for ring in 1..(land.span as f32 * CELL / 4.0) as i32 {
                for turn in 0..128 {
                    let (out, way) = (ring as f32 * 4.0, turn as f32 * std::f32::consts::TAU / 128.0);
                    let (x, z) = (way.cos() * out, way.sin() * out);
                    let (cx, cz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
                    let depth = (SEA - land.height(cx, cz)) as f32 * CELL;
                    // Deep, and open water for ten metres each way: somewhere to swim, not a pothole.
                    let open = [(10.0, 0.0), (-10.0, 0.0), (0.0, 10.0), (0.0, -10.0)].iter().all(|(dx, dz)| (SEA - land.height(((x + dx) / CELL).floor() as i32, ((z + dz) / CELL).floor() as i32)) as f32 * CELL > 1.2);
                    // A hillside to walk up: open meadow that climbs a fifth to a half of a metre
                    // in every metre for ten, some way; the way is given as the `--yaw` that walks up it.
                    let up = |yaw: f32| (0..10).all(|i| {
                        let h = |d: f32| (land.height(((x - yaw.sin() * d) / CELL).floor() as i32, ((z - yaw.cos() * d) / CELL).floor() as i32), land.biome(((x - yaw.sin() * d) / CELL).floor() as i32, ((z - yaw.cos() * d) / CELL).floor() as i32));
                        let (a, b) = (h(i as f32), h(i as f32 + 1.0));
                        b.1 == Biome::Meadow && (0.2..=0.5).contains(&((b.0 - a.0) as f32 * CELL))
                    });
                    let climb = (what == "slope").then(|| (0..16).map(|k| k as f32 * std::f32::consts::TAU / 16.0).find(|yaw| up(*yaw))).flatten();
                    let depth = climb.map_or(depth, |yaw| -100.0 - yaw);
                    let hit = if what == "slope" { climb.is_some() && land.trees.iter().all(|t| ((t.x - cx) as f32).hypot((t.z - cz) as f32) * CELL > 9.0) } else if what == "deep" { depth > 2.0 && open } else if what == "trail" {
                        // A stretch in the open, to be seen: meadow, and no tree within seven metres.
                        land.trail(cx, cz) > 0.8 && land.biome(cx, cz) == Biome::Meadow && land.trees.iter().all(|t| ((t.x - cx) as f32).hypot((t.z - cz) as f32) * CELL > 7.0)
                    } else { Biome::named(&what) == Some(land.biome(cx, cz)) };
                    if hit && found.len() < 3 && found.iter().all(|f: &(f32, f32, f32)| (f.0 - x).hypot(f.1 - z) > 30.0) {
                        found.push((x, z, depth));
                    }
                }
            }
            if found.is_empty() {
                fail(&format!("no `{what}` on this island (deep, trail, slope, or one of water shore meadow forest rock snow)"));
            }
            for (x, z, depth) in found {
                println!("{what} at {x:.0},{z:.0} · {:.0}m from the start{}", x.hypot(z), if depth <= -100.0 { format!(" · up it is --yaw {:.2}", -depth - 100.0) } else if depth > 0.0 { format!(" · {depth:.1}m of water") } else { String::new() });
            }
        }
        "years" => {
            // The island's creatures by the day and nothing else (`Sim::day`), for years: who
            // is born, who dies and of what, and whether the numbers hold.
            let years: usize = args.file.as_deref().and_then(|v| v.parse().ok()).unwrap_or(3);
            let (world, land, mut sim) = living(&args);
            // Nobody is watching: nothing is lived by the second.
            sim.agents.retain(|_, a| sim.kinds[a.kind].temper != cube_sim::life::Temper::Person);
            sim.agents.values_mut().for_each(|a| a.awake = false);
            let count = |sim: &Sim| {
                let mut n = vec![0usize; sim.kinds.len()];
                sim.agents.values().for_each(|a| n[a.kind] += 1);
                n
            };
            let mut rows = vec![count(&sim)];
            let mut all = cube_sim::life::Tally::default();
            let mut kills: std::collections::BTreeMap<(usize, usize), u32> = Default::default();
            let mut grazing = (0.0f32, 1.0f32, 0.0f32);
            // From the first day of a spring, so a year is a year.
            sim.when = sim.when.floor() - sim.when.floor().rem_euclid(cube_sim::sky::YEAR) + cube_sim::sky::YEAR;
            for _ in 0..years {
                for _ in 0..cube_sim::sky::YEAR as usize {
                    let t = sim.day(&world, Some(&land));
                    (all.born, all.old, all.hunted, all.starved, all.winter, all.moved, all.followed) = (all.born + t.born, all.old + t.old, all.hunted + t.hunted, all.starved + t.starved, all.winter + t.winter, all.moved + t.moved, all.followed + t.followed);
                    grazing = (grazing.0 + t.forage.0, grazing.1.min(t.forage.1), grazing.2 + 1.0);
                    for pair in t.taken {
                        *kills.entry(pair).or_default() += 1;
                    }
                    for (kind, n) in t.ends {
                        let e = all.ends.entry(kind).or_default();
                        *e = [e[0] + n[0], e[1] + n[1], e[2] + n[2], e[3] + n[3]];
                    }
                    sim.when += 1.0;
                }
                rows.push(count(&sim));
            }
            let total = |row: &[usize]| row.iter().sum::<usize>();
            println!("years {years} seed={}: {} creatures at the start, {} at the end · born {} · died of age {}, hunted {}, starved {}, of winter {} · groups that moved {} · grazers that followed the grass {}", args.seed, total(&rows[0]), total(&rows[years]), all.born, all.old, all.hunted, all.starved, all.winter, all.moved, all.followed);
            println!("  grazing: {:.0}% left on the ground that is grazed, on the whole; {:.0}% on the barest piece at its barest", grazing.0 / grazing.2.max(1.0) * 100.0, grazing.1 * 100.0);
            let mut checks = Checks(Vec::new());
            for (k, kind) in sim.kinds.iter().enumerate() {
                if rows[0][k] == 0 {
                    continue;
                }
                let line: Vec<String> = rows.iter().map(|r| r[k].to_string()).collect();
                println!("  {:<12} {:<7} {}", kind.id, format!("{:?}", kind.diet).to_lowercase(), line.join(" > "));
                // What has fallen to under half: who took it.
                if rows[years][k] * 2 < rows[0][k] {
                    let mut by: Vec<(u32, &str)> = kills.iter().filter(|((_, prey), _)| *prey == k).map(|((hunter, _), n)| (*n, sim.kinds[*hunter].id.as_str())).collect();
                    by.sort_by_key(|b| std::cmp::Reverse(b.0));
                    let e = all.ends.get(&k).copied().unwrap_or_default();
                    println!("    born {} · died of age {}, starved {}, of winter {} · taken by: {}", e[0], e[1], e[2], e[3], by.iter().map(|(n, id)| format!("{id} {n}")).collect::<Vec<_>>().join(", "));
                }
                // A kind that began as more than a handful is still there, and none has overrun the island.
                checks.bar(&format!("{} lasts", kind.id), rows[years][k] as f32, rows[0][k] < 6 || rows[years][k] >= 2, "a kind that began with six or more still has two");
                // Over a few years, none has overrun the island; over many, each has come to
                // a number it keeps (the island is begun with fewer of most kinds than it
                // carries, so growth alone is not a fault: growth that does not stop is).
                if years < 16 {
                    checks.bar(&format!("{} holds", kind.id), rows[years][k] as f32, rows[years][k] <= rows[0][k] * 6 + 10, "no kind grows past six times what it began as");
                } else {
                    let (was, now) = (rows[years - 8][k] as f32, rows[years][k] as f32);
                    checks.bar(&format!("{} settles", kind.id), now, (now - was).abs() <= was * 0.35 + 10.0, "within a third of what it was eight years before");
                }
            }
            let (first, last) = (total(&rows[0]) as f32, total(&rows[years]) as f32);
            checks.bar("creatures", last, last > first * 0.4 && last < first * 3.0, "between two fifths and three times as many as at the start");
            checks.finish()
        }
        "who" => {
            let (_, land, sim) = living(&args);
            let mut groups: std::collections::BTreeMap<(usize, i32, i32), usize> = Default::default();
            for agent in sim.agents.values() {
                *groups.entry((agent.kind, (agent.mind.home.x * 10.0) as i32, (agent.mind.home.y * 10.0) as i32)).or_default() += 1;
            }
            for ((kind, x, z), n) in groups {
                let biome = land.biome((x as f32 / 10.0 / CELL).floor() as i32, (z as f32 / 10.0 / CELL).floor() as i32);
                println!("{} ×{n} at {:.0},{:.0} {}", sim.kinds[kind].id, x as f32 / 10.0, z as f32 / 10.0, biome.name());
            }
        }
        _ => {
            println!("{HELP}");
            std::process::exit(if command.is_empty() || command.contains("help") { 0 } else { 2 });
        }
    }
}
