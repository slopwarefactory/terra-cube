//! What a creature decides to do. A mind is one of a few states (resting, grazing, roaming,
//! fleeing, chasing, striking, watching) and a few numbers; [`think`] reads what is near and
//! sets what is asked of the body. No paths are planned: a creature heads for a point, looks
//! at the ground a step ahead and turns aside from walls, drops and deep water.
//!
//! Tempers ([`Temper`]): timid creatures run from people, hunters and monsters; hunters run
//! timid ones down, then tire; hostile ones go for people; wary ones face whatever comes
//! close and hit it if it comes closer. Nothing dies yet: a blow is a shove and a flinch.

use crate::{
    DT,
    life::{Act, Agent, Gait, Hit, Kind, Temper},
    rng::Rng,
};
use cube_voxel::{Block, CELL, World};
use glam::{IVec3, Vec2, Vec3};

/// How far off its den a creature that keeps one will bolt for it, metres.
pub const DEN: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Rest,
    Graze,
    Roam,
    Flee,
    Chase,
    Strike,
    Watch,
}

#[derive(Clone, Debug)]
pub struct Mind {
    pub state: State,
    /// Seconds left in the state.
    pub left: f32,
    /// The middle of the ground it keeps to.
    pub home: Vec2,
    /// Where it is roaming to.
    pub goal: Vec2,
    /// What it flees, chases, strikes or watches.
    pub target: Option<u32>,
    /// Seconds until it will chase or strike again.
    pub tired: f32,
    /// The side it last turned to round something, so it does not dither: 1 or -1.
    veer: f32,
    /// Where it was when progress was last checked, and the seconds since.
    mark: Vec2,
    since_mark: f32,
    hit_done: bool,
    /// How much it is about at this hour, 0 to 1 (its niche's hours): set each tick.
    pub about: f32,
    /// Where it roosts, once it has looked for somewhere (a kind that roosts, `Kind::roosts`):
    /// the top of a bough near where it lives, or nowhere if there is none.
    pub perch: Option<Option<glam::Vec3>>,
    /// Seconds it has been trying to get onto its perch.
    pub trying: f32,
    /// It is going to water to drink, and drinks when it gets there.
    pub to_water: bool,
    /// It is grazing its way along: at the end of this walk it grazes again.
    pub bout: bool,
    /// Seconds it has been at rest without a break: after a few it lies down, where it is
    /// out of its hours or has settled for a long rest.
    pub still: f32,
    /// It has settled for a long rest, fed and easy, by day.
    pub easy: bool,
}

impl Mind {
    pub fn new(home: Vec2, rng: &mut Rng) -> Self {
        Self { state: State::Rest, left: rng.between(0.0, 4.0), home, goal: home, target: None, tired: 0.0, veer: if rng.chance(0.5) { 1.0 } else { -1.0 }, mark: home, since_mark: 0.0, hit_done: false, about: 1.0, perch: None, trying: 0.0, to_water: false, bout: false, still: 0.0, easy: false}
    }
}

/// What a creature is plainly about, to another of its kind that sees it: the herd goes to
/// water when one of it does, and bolts when one of it bolts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Errand {
    None,
    /// Going to drink, there.
    Water(Vec2),
    /// Running from that one.
    Flee(u32),
}

impl Mind {
    pub fn errand(&self) -> Errand {
        match (self.state, self.target) {
            (State::Flee, Some(from)) => Errand::Flee(from),
            (State::Roam, _) if self.to_water => Errand::Water(self.goal),
            _ => Errand::None,
        }
    }
}

/// Another creature, as a mind sees it.
pub struct Seen {
    pub id: u32,
    pub pos: Vec3,
    /// The way it faces, radians, and what it is doing.
    pub facing: f32,
    pub act: crate::life::Act,
    pub vel: Vec3,
    pub temper: Temper,
    /// From its middle to its furthest edge over the ground.
    pub reach: f32,
    /// Which kind it is, and what it is about.
    pub kind: usize,
    pub errand: Errand,
}

/// How close `them` must come before `me` takes notice, metres: to flee it, chase it or
/// stand it off. 0 when it never does. A deer bolts from a wolf a long way off and lets a
/// bear or a person come nearer.
fn notice(me: Temper, them: Temper) -> f32 {
    match (me, them) {
        (Temper::Timid, Temper::Hunter | Temper::Hostile) => 12.0,
        (Temper::Timid, Temper::Person) => 8.0,
        (Temper::Timid, Temper::Wary) => 5.0,
        (Temper::Hunter, Temper::Timid) => 16.0,
        (Temper::Hostile, Temper::Person) => 14.0,
        (Temper::Wary, Temper::Person | Temper::Hunter | Temper::Hostile) => 7.0,
        _ => 0.0,
    }
}

/// How hungry a hunter is before it hunts (in meals: half way to needing one), and how
/// thirsty anything is before it goes to water (in days: three times a day, where until
/// 2026-11-12 it was 0.7, once in seventeen minutes of play, and of sixty-nine creatures
/// watched for five minutes eight drank).
pub const HUNTS: f32 = 0.5;
pub const THIRSTS: f32 = 0.3;

/// The nearest water's edge to a place, within sixty metres: the last dry ground before it,
/// looking out sixteen ways. Water stands at the sea's level everywhere, river and lake too.
fn water_near(world: &World, me: Vec2) -> Option<Vec2> {
    let level = crate::terrain::SEA - 1;
    let wet = |p: Vec2| world.get(IVec3::new(cell_of(p.x), level, cell_of(p.y))) == Block::Water;
    let mut best: Option<(f32, Vec2)> = None;
    for turn in 0..16 {
        let way = Vec2::from_angle(turn as f32 * std::f32::consts::TAU / 16.0);
        for step in 1..=30 {
            let far = step as f32 * 2.0;
            if best.is_some_and(|b| far >= b.0) {
                break;
            }
            if wet(me + way * far) {
                best = Some((far, me + way * (far - 1.0)));
                break;
            }
        }
    }
    best.map(|b| b.1)
}

/// How far a creature keeps from home when it roams, metres.
fn range(kind: &Kind) -> f32 {
    6.0 + kind.walk * 10.0
}

fn flat(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}

fn cell_of(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

/// What a body stood at cell height `level` finds in the next column over.
enum Footing {
    /// Ground to stand on: its level.
    Floor(i32),
    Wall,
    Drop,
    /// Water it would have to swim.
    Deep,
}

/// How far up and down, in cells, a creature will go from one column to the next. At its
/// ease it takes what it can step up (half a metre) and drops of a metre, so it stays on the
/// ground it knows; in a hurry it jumps what a jump clears (a metre) and drops three.
#[derive(Clone, Copy)]
struct Nerve {
    up: i32,
    down: i32,
    /// What it walks up without a jump, in cells: more than this within `up` wants a jump.
    step: i32,
}

impl Nerve {
    /// At its ease, a creature that steps up `step` metres without jumping.
    fn easy(step: f32) -> Self {
        let up = (step / CELL) as i32;
        Nerve { up, down: up * 2, step: up }
    }
    /// The same creature in a hurry: it jumps what a jump clears with room (1.25 m) if that is more
    /// than its step, and drops three metres or six of its steps.
    fn hurried(step: f32) -> Self {
        let up = (step / CELL) as i32;
        Nerve { up: up.max((1.25 / CELL) as i32), down: (up * 6).max((3.0 / CELL) as i32), step: up }
    }
}

fn footing(world: &World, x: i32, z: i32, level: i32, height: f32, nerve: Nerve) -> Footing {
    let solid = |y: i32| world.get(IVec3::new(x, y, z)).solid();
    let mut at = level + nerve.up;
    if solid(at) {
        return Footing::Wall;
    }
    while !solid(at - 1) {
        at -= 1;
        if at < level - nerve.down {
            return Footing::Drop;
        }
    }
    let tall = (height / CELL).ceil() as i32;
    if (0..tall).any(|up| solid(at + up)) {
        return Footing::Wall;
    }
    // Nothing climbs into a bush or a tree.
    if at > level && {
        let under = world.get(IVec3::new(x, at - 1, z));
        under.foliage() || under.wood()
    } {
        return Footing::Wall;
    }
    let depth = (0..tall + 1).take_while(|&up| world.get(IVec3::new(x, at + up, z)) == Block::Water).count();
    if depth as f32 * CELL > height * 0.5 { Footing::Deep } else { Footing::Floor(at) }
}

/// Whether there is dry ground to stand on at a point, looking down from well over `near`.
fn dry_ground(world: &World, at: Vec2, near: f32, height: f32) -> bool {
    let (x, z) = (cell_of(at.x), cell_of(at.y));
    let mut y = cell_of(near + 12.0);
    let get = |y: i32| world.get(IVec3::new(x, y, z));
    // Down through the air, and through any tree's crown, to the ground.
    loop {
        while y > 0 && !get(y).solid() {
            y -= 1;
        }
        if !get(y).foliage() {
            break;
        }
        while y > 0 && get(y).solid() {
            y -= 1;
        }
    }
    y > 0 && !get(y).wood() && (get(y + 1) != Block::Water || height > 1.0)
}

/// How many metres of the straight line from a body to a point can be walked.
fn clear_way(world: &World, agent: &Agent, step: f32, to: Vec2) -> f32 {
    let from = flat(agent.body.pos);
    let (dir, whole) = ((to - from).normalize_or_zero(), from.distance(to));
    let (mut at, mut last, mut out) = (cell_of(agent.body.pos.y + 0.01), (cell_of(from.x), cell_of(from.y)), 0.0);
    while out < whole {
        out += CELL * 0.5;
        let p = from + dir * out;
        let column = (cell_of(p.x), cell_of(p.y));
        if column == last {
            continue;
        }
        last = column;
        match footing(world, column.0, column.1, at, agent.body.height, Nerve::easy(step)) {
            Footing::Floor(found) => at = found,
            _ => return out - 0.5,
        }
    }
    whole
}

/// The way to go to get toward `want` (a unit direction) from where the body is: straight on
/// if the ground a step ahead can be walked, else the nearest way round that can. Says whether
/// to jump. Zero when every way is shut.
fn steer(world: &World, agent: &Agent, speed: f32, want: Vec2, nerve: Nerve, veer: &mut f32) -> (Vec2, bool) {
    let body = &agent.body;
    let (pos, level) = (flat(body.pos), cell_of(body.pos.y + 0.01));
    // Where the ground is drawn lower than its cells a body stands down in them (it stands on
    // the ground as it is drawn, in blocks of up to a metre on a steep hillside): then the way
    // on is from the top of the cells it stands in, and out of a notch deeper than it steps it
    // jumps. (Looked for from its feet, every way round was a wall three cells high: a fox
    // that ran into such a notch stood in it for the rest of the day, `boxed-in`, seed 5.)
    let sunk = (0..6).take_while(|up| world.get(IVec3::new(cell_of(pos.x), level + up, cell_of(pos.y))).ground()).count() as i32;
    let level = level + sunk;
    let look = body.half + 0.3 + speed * 0.15;
    // Already swimming, water is no bar: it has to get out somewhere.
    let passes = |found: &Footing| matches!(found, Footing::Floor(_)) || body.swimming && matches!(found, Footing::Deep);
    for turn in [0.0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9, 2.6, -2.6] {
        let dir = Vec2::from_angle(turn * *veer).rotate(want);
        // The body is as wide as it is: its shoulders must clear the first step too.
        let side = dir.perp() * body.half * 0.9;
        let first = pos + dir * (body.half + 0.3);
        let mut ok = [first + side, first - side].iter().all(|p| passes(&footing(world, cell_of(p.x), cell_of(p.y), level, body.height, nerve)));
        // And the body itself, which is a box: it must be able to move, at its own level or
        // up what it will climb, along at least one of the axes this way mostly runs.
        let free = |by: Vec3| (0..=nerve.up + sunk).any(|up| !body.blocked(world, body.pos + by + Vec3::Y * (up as f32 * CELL))) || body.swimming;
        ok &= (dir.x.abs() > 0.35 && free(Vec3::X * dir.x.signum() * 0.1)) || (dir.y.abs() > 0.35 && free(Vec3::Z * dir.y.signum() * 0.1));
        let (mut at, mut jump) = (level, sunk > nerve.step);
        let mut out = body.half + 0.25;
        let mut last = (cell_of(pos.x), cell_of(pos.y));
        while ok && out <= look + 0.5 {
            let p = pos + dir * out;
            let column = (cell_of(p.x), cell_of(p.y));
            out += CELL;
            if column == last {
                continue;
            }
            last = column;
            let found = footing(world, column.0, column.1, at, body.height, nerve);
            ok = passes(&found);
            if let Footing::Floor(found) = found {
                jump |= found - at > nerve.step && out < body.half + 1.5;
                at = found;
            }
        }
        if ok {
            if turn != 0.0 {
                *veer = (turn * *veer).signum();
            }
            return (dir, jump);
        }
    }
    (Vec2::ZERO, false)
}

/// One tick of one mind: sets the agent's `intent`, `gait`, `turn_to` and `doing`, and gives
/// back the blow it lands this tick, if any.
pub fn think(id: u32, agent: &mut Agent, kind: &Kind, seen: &[Seen], world: &World, trails: Option<u32>, rng: &mut Rng) -> Option<Hit> {
    let me = flat(agent.body.pos);
    let mut m = agent.mind.clone();
    m.left -= DT;
    m.tired = (m.tired - DT).max(0.0);

    // The nearest thing it minds, with the gap between their edges.
    let my_reach = agent.body.half + agent.body.long.length();
    let noticed = seen
        .iter()
        .filter(|s| s.id != id && (s.pos.y - agent.body.pos.y).abs() < 6.0)
        .map(|s| (s, (flat(s.pos).distance(me) - s.reach - my_reach).max(0.0)))
        .filter(|(s, gap)| *gap < notice(kind.temper, s.temper))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let gap_to = |s: &Seen| (flat(s.pos).distance(me) - s.reach - my_reach).max(0.0);
    let strike_at = 0.45 + kind.height * 0.15;

    // What it sees changes what it is doing.
    match (kind.temper, noticed) {
        (Temper::Timid, Some((s, _))) => {
            if m.state != State::Flee {
                agent.log.fled += 1;
            }
            m.state = State::Flee;
            m.target = Some(s.id);
            m.left = 2.0;
        }
        // What hunts hunts from hunger: fed, it lets a rabbit go by, and a person too, unless
        // they come within a few steps of it. (What is hostile needs no reason.)
        (Temper::Hunter, Some((_, gap))) if agent.hunger < HUNTS && gap > 2.5 && !matches!(m.state, State::Chase | State::Strike) => {}
        (Temper::Hunter | Temper::Hostile, Some((s, _))) if m.tired == 0.0 && !matches!(m.state, State::Chase | State::Strike) => {
            m.state = State::Chase;
            m.target = Some(s.id);
            m.left = 14.0;
            agent.log.hunts += (kind.temper == Temper::Hunter) as u32;
        }
        (Temper::Wary, Some((s, gap))) if m.state != State::Strike => {
            m.target = Some(s.id);
            if gap < strike_at + 0.5 && m.tired == 0.0 {
                m.state = State::Strike;
                m.left = 0.8;
                m.hit_done = false;
            } else if m.state != State::Watch {
                m.state = State::Watch;
                m.left = 3.0;
            } else {
                m.left = m.left.max(1.5);
            }
        }
        _ => {}
    }

    // What its own kind is about, near it, it is about too: one bolts and the herd bolts from
    // the same thing; one sets off to drink and the rest, if they could do with a drink, go
    // with it. (Until 2026-11-12 each decided alone, and a herd was creatures standing near
    // one another.)
    if !matches!(m.state, State::Flee | State::Chase | State::Strike) {
        let mates = seen.iter().filter(|s| s.id != id && s.kind == agent.kind && flat(s.pos).distance(me) < 18.0);
        for mate in mates {
            match mate.errand {
                // (Only from what is still near: with no such bound two of a herd kept each
                // other running for as long as what had frightened them was anywhere.)
                Errand::Flee(from) if kind.temper == Temper::Timid && seen.iter().any(|s| s.id == from && flat(s.pos).distance(me) < 26.0) => {
                    agent.log.fled += 1;
                    (m.state, m.target, m.left, m.to_water, m.bout) = (State::Flee, Some(from), 3.0, false, false);
                    break;
                }
                Errand::Water(to) if !m.to_water && agent.thirst > THIRSTS * 0.5 && matches!(m.state, State::Rest | State::Graze) => {
                    let beside = to + Vec2::from_angle(rng.between(0.0, std::f32::consts::TAU)) * rng.between(0.5, 2.5);
                    (m.state, m.left, m.goal, m.to_water, m.bout) = (State::Roam, 60.0, beside, true, false);
                    agent.log.trips += 1;
                    break;
                }
                _ => {}
            }
        }
    }

    let target = m.target.and_then(|t| seen.iter().find(|s| s.id == t));

    // What the state asks of the body: where to head and how fast, or what to face.
    let (mut heading, mut gait, mut face, mut doing, mut hit) = (None, Gait::Walk, None, None, None);
    let toward = |p: Vec2| (p - me).try_normalize();
    let bearing = |p: Vec2| (p.x - me.x).atan2(p.y - me.y);
    match m.state {
        State::Rest => {
            // Out of its hours it mostly stays where it is: a badger lies up by day, a pony
            // stands the night out. (It still flees and fights: those are not chosen here.)
            if m.left <= 0.0 && rng.unit() > m.about.max(0.12) {
                m.left = rng.between(6.0, 14.0);
            }
            // (Lying up out of its hours is not standing about: it is counted apart.)
            if m.about < 0.3 {
                agent.log.lain += DT;
            }
            // At rest a few seconds out of its hours, or settled for a long rest by day, it
            // lies down. (Until 2026-11-12 resting was standing: a badger by day and a pony
            // by night stood where they were, and nothing on the island ever lay down.)
            if m.still > 4.0 && (m.about < 0.3 || m.easy) && kind.temper != Temper::Hostile {
                doing = Some(Act::Lie);
            }
            // And what keeps a den lies up in it, out of its hours and at its mouth: under
            // the ground, a while at a time.
            if kind.den && m.still > 4.0 && m.about < 0.3 && me.distance(m.home) < 2.5 {
                agent.under = 20.0;
            }
            // Thirst comes first: to the nearest water it can find, and drink. (None in reach,
            // and it makes do: there is dew, and rain.)
            if m.left <= 0.0 && agent.thirst >= THIRSTS && kind.diet != crate::life::Diet::None {
                match water_near(world, me) {
                    Some(to) => {
                        (m.state, m.left, m.goal, m.to_water) = (State::Roam, 60.0, to, true);
                        agent.log.trips += 1;
                    }
                    None => agent.thirst = 0.3,
                }
            }
            // What hunts and is hungry goes looking: toward the nearest of what it hunts that
            // is anywhere about, at a walk, until it is near enough to be seen and run down.
            // (It hunted only what came within sixteen metres of where it stood: in five
            // minutes of three bears, a fox and three goblins, nothing was struck.)
            if m.left <= 0.0 && kind.temper == Temper::Hunter && agent.hunger >= HUNTS && m.tired == 0.0 && m.about > 0.3 {
                let prey = seen.iter().filter(|s| s.temper == Temper::Timid && (s.pos.y - agent.body.pos.y).abs() < 8.0).map(|s| (flat(s.pos), flat(s.pos).distance(me))).filter(|p| p.1 < 70.0).min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((at, far)) = prey {
                    let toward = me + (at - me) * ((far - 10.0).max(0.0) / far.max(0.1));
                    let reach = clear_way(world, agent, kind.step(), toward);
                    if reach >= 2.0 {
                        (m.state, m.left, m.goal) = (State::Roam, 20.0, me + (toward - me).clamp_length_max(reach));
                        agent.log.trips += 1;
                    }
                }
            }
            // Out of its hours and away from where it lies up, it goes home.
            if m.left <= 0.0 && m.state == State::Rest && m.about < 0.3 && me.distance(m.home) > if kind.den { 1.5 } else { 8.0 } {
                let reach = clear_way(world, agent, kind.step(), m.home);
                if reach >= 2.0 {
                    (m.state, m.left, m.goal) = (State::Roam, 40.0, me + (m.home - me).clamp_length_max(reach));
                    agent.log.trips += 1;
                }
            }
            if m.left <= 0.0 && m.state == State::Rest {
                // What eats plants grazes because it is hungry: full, it does not; empty, it does little else.
                let grazes = matches!(kind.diet, crate::life::Diet::Plants | crate::life::Diet::All) && kind.temper != Temper::Hostile;
                let roll = rng.unit();
                if grazes && roll < ((agent.hunger - 0.15) * 1.6).clamp(0.0, 0.95) {
                    m.state = State::Graze;
                    m.left = rng.between(8.0, 18.0);
                    m.bout = true;
                } else if roll < 0.9 || me.distance(m.home) > range(kind) {
                    m.state = State::Roam;
                    m.left = rng.between(8.0, 16.0);
                    // Somewhere dry near home that it can walk most of the way to; else it stays.
                    m.goal = me;
                    // The larger animals go by the trails when one is in reach: of the places
                    // it thinks of first, it takes only one that lies on a trail. That is
                    // what a trail is; before this they were drawn and nothing walked them.
                    let by_trail = trails.filter(|_| kind.height > 0.5);
                    for n in 0..24 {
                        let to = m.home + Vec2::from_angle(rng.between(0.0, std::f32::consts::TAU)) * rng.between(2.0, range(kind));
                        if n < 16 && by_trail.is_some_and(|seed| crate::terrain::worn(to.x, to.y, seed) < 0.3) {
                            continue;
                        }
                        if by_trail.is_none() && n >= 8 {
                            break;
                        }
                        if !dry_ground(world, to, agent.body.pos.y, kind.height) {
                            continue;
                        }
                        let far = clear_way(world, agent, kind.step(), to);
                        if far >= 2.0 {
                            m.goal = me + (to - me).clamp_length_max(far);
                            break;
                        }
                    }
                } else {
                    m.left = rng.between(2.0, 5.0);
                }
                // Fed, watered and at home in its hours, now and then it settles for a long
                // rest where it is and lies down in the sun.
                m.easy = false;
                if agent.hunger < 0.2 && agent.thirst < THIRSTS * 0.7 && rng.chance(0.25) {
                    (m.state, m.left, m.easy) = (State::Rest, rng.between(25.0, 60.0), true);
                }
            }
        }
        State::Graze => {
            doing = Some(Act::Eat);
            // Four hours' grazing is a meal: what grazes is at it for a third of its day and
            // more. (At two hours, a deer ate for a sixth of the time it was watched and stood
            // for two thirds.)
            agent.hunger = (agent.hunger - DT / (crate::sky::DAY / 6.0)).max(0.0);
            if m.left <= 0.0 {
                // Still hungry, it moves on a few steps to fresh grazing and eats again: a
                // grazing animal goes over the ground as it feeds. (It ate where it stood,
                // stood a while, and rolled the dice again.)
                let on = (m.bout && agent.hunger > 0.15).then(|| m.home + Vec2::from_angle(rng.between(0.0, std::f32::consts::TAU)) * rng.between(2.0, range(kind))).filter(|to| me.distance(*to) < 14.0 && dry_ground(world, *to, agent.body.pos.y, kind.height));
                match on.map(|to| (to, clear_way(world, agent, kind.step(), to))).filter(|(_, far)| *far >= 2.0) {
                    Some((to, far)) => {
                        (m.state, m.left, m.goal) = (State::Roam, 8.0, me + (to - me).clamp_length_max(far.min(8.0)));
                    }
                    None => {
                        (m.state, m.left, m.bout) = (State::Rest, rng.between(1.0, 4.0), false);
                    }
                }
            }
        }
        State::Roam => {
            heading = toward(m.goal).filter(|_| me.distance(m.goal) > 0.8);
            // Come to the water it set out for, it drinks: head down a few seconds.
            if m.to_water && (heading.is_none() || agent.body.swimming) {
                (m.to_water, m.state, m.left) = (false, State::Graze, 6.0);
                agent.thirst = 0.0;
                agent.log.drank += 1;
            } else if (heading.is_none() || m.left <= 0.0) && m.bout {
                (m.state, m.left) = (State::Graze, rng.between(8.0, 18.0));
            } else if heading.is_none() || m.left <= 0.0 {
                // (Water it set out for and did not come to: it makes do a while, as where there is none.)
                if std::mem::take(&mut m.to_water) {
                    agent.thirst = agent.thirst.min(THIRSTS * 0.4);
                }
                m.state = State::Rest;
                m.left = rng.between(2.0, 6.0);
            }
        }
        State::Flee => match target {
            Some(t) if m.left > 0.0 => {
                let gap = gap_to(t);
                // Away from it, and a little across the way it is coming.
                let away = (me - flat(t.pos)).try_normalize().unwrap_or(Vec2::X);
                heading = Some(away);
                gait = if gap < 6.0 { Gait::Sprint } else { Gait::Run };
                // What keeps a den bolts for it, if it is near and not past what it runs
                // from, and at its mouth goes to ground: for a while nothing sees or reaches it.
                let den = m.home - me;
                if kind.den && den.length() < DEN && den.normalize_or_zero().dot(away) > -0.3 {
                    heading = Some(den.normalize_or(away));
                    if den.length() < 0.9 {
                        agent.under = rng.between(25.0, 45.0);
                        (m.state, m.target, m.left) = (State::Rest, None, 0.0);
                    }
                }
            }
            _ => {
                // It got away: this is where it lives now. (A bird goes back to where it rose from.)
                // (Nor what keeps a den: its den is where it lives.)
                if !kind.flies && !kind.den {
                    m.home = me;
                }
                m.target = None;
                m.state = State::Rest;
                m.left = rng.between(2.0, 5.0);
            }
        },
        State::Chase => match target {
            // (Nor is what has gone up to roost chased: a fox stood under a pheasant's tree
            // with no way on, a third of its time. What flies off low is still run after.)
            Some(t) if m.left > 0.0 && gap_to(t) < 30.0 && t.pos.y - agent.body.pos.y < kind.height + 2.5 => {
                let gap = gap_to(t);
                // (What has gone up out of reach is not struck at: a bird on the wing.)
                let reached = t.pos.y - agent.body.pos.y < kind.height + 0.8;
                if gap < strike_at && m.tired == 0.0 && reached {
                    m.state = State::Strike;
                    m.left = 0.8;
                    m.hit_done = false;
                } else if gap < strike_at * 0.6 {
                    // Close enough: it waits its moment facing them.
                    face = Some(bearing(flat(t.pos)));
                } else {
                    // Lead it a little: where it will be, not where it is.
                    heading = toward(flat(t.pos) + flat(t.vel) * (gap / kind.speed(Gait::Sprint)).min(1.0));
                    gait = if gap < 8.0 { Gait::Sprint } else { Gait::Run };
                }
            }
            _ => {
                m.target = None;
                m.tired = rng.between(12.0, 20.0);
                m.state = State::Rest;
                m.left = rng.between(3.0, 6.0);
            }
        },
        State::Strike => {
            doing = Some(Act::Attack);
            face = target.map(|t| bearing(flat(t.pos)));
            // The blow lands part way through the swing, if what it was aimed at is still there.
            if m.left <= 0.45 && !m.hit_done {
                m.hit_done = true;
                if let Some(t) = target.filter(|t| gap_to(t) < strike_at + 0.6 && t.pos.y - agent.body.pos.y < kind.height + 0.8) {
                    let away = (flat(t.pos) - me).try_normalize().unwrap_or(Vec2::X);
                    let force = 3.0 + kind.height * 2.0;
                    hit = Some(Hit { on: t.id, by: id, harm: kind.harm(), push: Vec3::new(away.x * force, 3.0, away.y * force), part: crate::life::Part::Trunk });
                }
            }
            if m.left <= 0.0 {
                (m.state, m.tired, m.left) = match kind.temper {
                    Temper::Hunter => (State::Rest, rng.between(15.0, 25.0), rng.between(3.0, 6.0)),
                    // It stays on them, and takes a breath between blows.
                    Temper::Hostile => (State::Chase, rng.between(1.5, 2.5), 10.0),
                    _ => (State::Watch, 1.5, 3.0),
                };
            }
        }
        State::Watch => {
            face = target.map(|t| bearing(flat(t.pos)));
            if m.left <= 0.0 || target.is_none() {
                m.target = None;
                m.state = State::Rest;
                m.left = rng.between(1.0, 3.0);
            }
        }
    }

    // Out of its depth: whatever it meant to do, it makes for home ground.
    if agent.body.swimming && heading.is_none() {
        heading = toward(m.home);
        doing = None;
    }

    let speed = kind.speed(gait);
    let nerve = if matches!(m.state, State::Flee | State::Chase) || agent.body.swimming { Nerve::hurried(kind.step()) } else { Nerve::easy(kind.step()) };
    // (On the wing it goes where it means to: steering is for feet, and reads the air under
    // a bird as a drop it will not step off.)
    let aloft = kind.flies && !agent.body.grounded && !agent.body.swimming;
    let (dir, jump) = heading.map_or((Vec2::ZERO, false), |want| if aloft { (want, false) } else { steer(world, agent, speed, want, nerve, &mut m.veer) });
    if heading.is_some() && dir == Vec2::ZERO {
        // No way on from here. A roam is given up; a chase or a flight waits for one to open.
        agent.log.boxed += DT;
        if m.state == State::Roam {
            // (And water it cannot get to it gives up on: it set out for it again every few
            // seconds, two hundred times in five minutes among ten deer.)
            if std::mem::take(&mut m.to_water) {
                agent.thirst = agent.thirst.min(THIRSTS * 0.4);
            }
            (m.state, m.bout) = (State::Rest, false);
            m.left = rng.between(0.5, 2.0);
        }
    }
    agent.intent.dir = dir;
    agent.intent.speed = speed;
    agent.gait = gait;
    agent.turn_to = face;
    if jump {
        agent.want_jump = 0.12;
    }
    // Held until the top of the jump, or it is cut to a hop that clears nothing.
    agent.hold_jump = jump || agent.body.jumping;
    // (How long it has been at rest; and lying is held from tick to tick, not for a time
    // that runs out and has it on its feet for a moment.)
    m.still = if m.state == State::Rest && dir == Vec2::ZERO { m.still + DT } else { 0.0 };
    if m.state != State::Rest {
        m.easy = false;
    }
    match doing {
        Some(Act::Lie) => agent.doing = Some((Act::Lie, 1.0)),
        Some(act) if agent.doing.is_none_or(|d| d.0 != act) => agent.doing = Some((act, m.left.max(0.1))),
        None => agent.doing = None,
        _ => {}
    }

    // Walking and getting nowhere for half a second: try the other way round, and give up a roam.
    if dir == Vec2::ZERO {
        (m.mark, m.since_mark) = (me, 0.0);
    }
    m.since_mark += DT;
    if m.since_mark >= 0.5 {
        if dir != Vec2::ZERO && me.distance(m.mark) < speed * 0.15 {
            agent.log.stuck += m.since_mark;
            m.veer = -m.veer;
            if m.state == State::Roam {
                if std::mem::take(&mut m.to_water) {
                    agent.thirst = agent.thirst.min(THIRSTS * 0.4);
                }
                (m.state, m.bout) = (State::Rest, false);
                m.left = rng.between(0.5, 2.0);
            } else {
                agent.want_jump = 0.12;
            }
        }
        m.mark = me;
        m.since_mark = 0.0;
    }
    agent.mind = m;
    hit
}
