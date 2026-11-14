//! What a cut leaves standing on nothing comes loose and falls. A world that can be cut
//! anywhere must answer for what was resting on what was cut: a tree whose trunk is cut
//! through goes over, an overhang whose root is dug out drops, and each lies afterwards
//! where it fell, in cells again, to be cut again.
//!
//! How, in order:
//! 1. **Cut** ([`Loose::cut`]): a ball is dug out of the world, to the bit. A tree or a
//!    boulder it bites into is first made *wrought* (`Block::wrought`): they are drawn as
//!    whole shapes while they stand untouched (`cube_game::flora`), and a shape cannot show a
//!    notch or lie where it fell. Which limbs and leaf are the same tree is what `plants`
//!    noted as it grew them (`Limb::tree`): trees in a wood touch, and by the cells alone
//!    one tree cannot be told from the next. All of it becomes cells that are drawn as
//!    cells, and whoever draws shapes is told which ([`Cut::trees`], [`Cut::stones`]) so as
//!    to stop drawing it.
//! 2. **What is left unsupported** is looked for round the cut: from each solid cell beside
//!    it, through solid neighbours. Reaching more than [`HELD`] cells, it is held (the land
//!    is one great piece, and so is anything that stands on it). Fewer, and closed, it is
//!    loose. Leaves bear nothing, and nor does anything still a whole shape (an untouched
//!    tree is scenery: nothing hangs from it): two trees that touch do not hold each other
//!    up, and a loose piece takes with it the leaves that are its own.
//! 3. **A piece** ([`Piece`]) is those cells taken out of the world and kept as they were,
//!    moved as one body: tall on a small base (a tree) it goes over about its foot, away
//!    from the cut; anything else drops.
//! 4. **Landing**: when any of it meets the world it is put back, cell for cell where each
//!    now lies (turned as it fell), and the piece is gone. Stone and earth that come down
//!    hard do not land whole: they **shatter** into lumps that are thrown out, turn as
//!    they go, and land each on its own, as rubble. Wood does not shatter.
//!
//! **A tree's fall is staged** (the developer, 2026-11-12: what is cut down should behave in
//! a specific way, so it looks interesting): it goes the way it leans, more than the way it
//! was pushed; it hangs a moment and then goes faster and faster; and it comes down whole,
//! trunk and limbs, along the line it fell, to be cut into logs where it lies. Only the
//! outermost twigs of its crown snap off as it strikes, and drop where they are. Then it
//! **settles**: its top is down and its butt is still up at the height of the stump, so it
//! turns back about its top and slides a little way on, until its butt is on the ground
//! beside the stump and it lies flat. (Stopped where its top first struck, it lay as a ramp
//! from stump to tip, and a length cut out of its middle hung in the air.) The same cut
//! fells the same tree the same way: nothing is thrown about.
//!
//! **What is cut is still a thing** ([`Thing`]; the developer, 2026-11-12: a tree is to be
//! known as a tree still, since a world this size is made and dropped as it is walked and
//! cannot keep every box of it). Each tree or boulder cut into is noted: the shape it grew
//! (its limbs and leaf, a few hundred numbers), every bite taken out of it, and where it
//! lies if it has come down. That is all that need be kept of it: its cells can be made
//! again from the shape, put where it lies, and the bites taken again. Nothing keeps or
//! restores from it yet; it is what saving and dropping a region will use.
//!
//! **The world is its seed and what was done to it** ([`Edit`], [`Loose::journal`],
//! [`replay`]): every cut and every thing put down is noted with the moment it was done (in
//! steps of what was falling), a few numbers each. Done again in the same order to the land
//! as the seed makes it, they give the same world, cell for cell: nothing here is random and
//! a fall is the same fall. So a part of the world that has been changed need not be kept:
//! it can be dropped and made again. (Not yet used so by the game, which keeps every
//! changed region: `cargo world replay` proves it.)
//!
//! **Water runs into what is opened to it** ([`flow`]): from water beside a cut, sideways and
//! down into air and up no higher than it stood, so a hole dug at a lake's edge fills to the lake's level and a
//! trench leads it downhill. By the cell; a lake is not drained by it (the sea is the sea).
//!
//! No engine types and nothing random: the same cut fells the same tree. `cargo world fell`
//! judges it on a real tree with no window.
//!
//! Here: what everything is (a piece, a tree or boulder once cut into, an edit, the journal)
//! and what holds what. `work`: what each tool does. `hunks`: the pieces that are carried.
//! `fall`: coming loose, falling, breaking and landing. `rest`: what lies at rest as itself.

use crate::items::Stuff;
use crate::plants::Form;
use cube_voxel::{BIT, Bits, Block, CELL, SPLIT, World};
use glam::{IVec3, Quat, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

// By what each part is about. Each is this module's child, so it sees everything here; what
// one part has that another uses is `pub(super)`, and all of it is named here.
mod fall;
mod hunks;
mod rest;
mod work;

pub use hunks::*;
pub use rest::*;

/// How many blows of an axe a standing tree takes before it is through, by how thick its
/// trunk is where it is cut (metres, its radius): one to every 15 cm across, two at the
/// least and eight at the most. (The developer was not sure of this, 2026-11-13: "it might
/// need some kind of like.. durability so its not one chop in some cases".)
#[allow(non_snake_case)]
pub fn BLOWS(thick: f32) -> u32 {
    ((thick * 2.0 / 0.15).ceil() as u32).clamp(2, 8)
}
/// How long the logs are that a bare trunk is bucked into, metres. (At two, a tree of 17 m
/// was eight logs and, at three hunks a log, as many hunks as the developer had called too
/// many.)
pub const LOG: f32 = 3.0;

/// How many cells a side a piece to carry is: half a metre, of wood ([`Block::Billet`]) or of
/// stone ([`Block::Cobble`]). One size for both, and the one place it is said. (Called a hunk
/// here: a chunk is already sixteen cells of the world.) The developer, 2026-11-13:
/// "resources are effectively these chunks, that are kind of just larger voxels ... all of
/// similar size ... you have to carry them in your hands, probably one at a time"; the
/// size was left to us.
pub const HUNK: i32 = 2;
/// How many hunks a log splits into: two from a small one (one or two cells thick), three
/// from a large (three: a great tree's). Not by how much wood it has: the developer, having felled a tree that gave 26,
/// "it changed into too many", and then, "logs should probably be like... 2 blocks for a
/// small log and 3 for a large? shouldnt be an excessive amount by any means" (2026-11-13).
pub const SPLITS: [usize; 2] = [2, 3];
/// How many blows of a pick a boulder takes before it breaks into hunks, by how far across
/// it is at its widest (metres, its radius): three to five ("maybe that takes 3-5 chips
/// depending on size of rock").
#[allow(non_snake_case)]
pub fn CHIPS(wide: f32) -> u32 {
    3 + (wide > 0.8) as u32 + (wide > 1.5) as u32
}

/// What is kept by the cell it is in: the world's own quick map (`cube_voxel::ByPlace`). With
/// the standard hasher, which is made to withstand an attacker and not to be fast, a great
/// tree's landing asked a million times what of it was in a cell and took 60 ms doing it.
/// Its order is nothing to go by: as with any map here, what is done in an order is sorted
/// first.
pub type Cells<V> = cube_voxel::ByPlace<V>;

/// A piece of more cells than this is put into the world over several steps when it lands,
/// as many rows of cells a step as come to [`ROWS`] cells of its box: a tree of 86,000 cells
/// put in one step was 30 ms of it, two frames lost. Its cells cannot be seen (it is drawn
/// as the tree it is), so nobody sees them come.
const SPREAD: usize = 20_000;
/// As many cells of a great piece's box are gone through in a step.
const ROWS: i32 = 80_000;

/// A place of more cells than this is wide: looked at for what is loose in it, the land in it
/// is not begun from (`Loose::loosen`). A dug hole is a few thousand; a tree as it lies, a million.
const WIDE: i64 = 100_000;

/// How many cells a piece may be and still be looked through to its end: what reaches
/// further is held to be standing on the land. A great oak with its crown is some 12,000.
pub const HELD: usize = 30_000;
/// And how many cells of the land itself (soil, sand, stone in its beds): a lump of earth
/// bigger than this, some sixty cubic metres, is the land. Far fewer than [`HELD`], since
/// every spadeful is a search that ends here: at 30,000 a spadeful took 13 ms.
pub const LAND: usize = 4_000;
/// How far from its wood a piece's own leaves are gathered, cells.
const LEAFY: u8 = 10;
/// How fast stone or earth must come down to break up, metres a second: a drop of about
/// half a metre.
const SHATTERS: f32 = 4.5;
/// Metres a second a second that a loose thing falls at (as a body does: `body`).
const FALL: f32 = crate::matter::GRAVITY;

/// One cell of a piece: whole, or its bits.
#[derive(Clone)]
pub enum Part {
    Whole(Block),
    Bits(Box<Bits>),
}

/// Something loose: its cells where they were in the world (cell positions), and how it has
/// moved since. A point of it that was at `p` (metres) is now at [`Piece::now`]`(p)`.
#[derive(Clone)]
pub struct Piece {
    pub id: u32,
    pub cells: Cells<Part>,
    /// What it turns about (where it stood on the ground, for what goes over), how far that
    /// has moved, and how it is turned.
    pub pivot: Vec3,
    pub shift: Vec3,
    pub turn: Quat,
    vel: Vec3,
    /// Going over like a tree: which way, how far it has tipped (radians) and how fast.
    over: Option<(Vec3, f32, f32)>,
    /// How it turns as it flies, radians a second about each axis (a lump thrown from
    /// something shattered).
    spin: Vec3,
    /// It will break up if it lands hard: stone or earth, not yet broken.
    brittle: bool,
    /// Hunks and nothing else: it comes down where they lie two cells apart, and does not roll.
    hunk: bool,
    /// Which thing it is the fallen part of, if it is a tree gone over.
    thing: Option<u32>,
    /// How it was turned when it began the turn it is making (nothing, going over; as its
    /// top struck, settling), and whether it is settling. And the part of it that is not
    /// tried against the world: what is within so far of a place in it (its foot on the
    /// stump, going over; its top on the ground, settling).
    base: Quat,
    settling: bool,
    spare: (Vec3, f32),
    /// How tall it is, metres, and the places of it that are tried against the world.
    tall: f32,
    feel: Vec<Vec3>,
    /// What it weighs, kilograms: by what each cell of it is made of (`matter::density`).
    pub mass: f32,
    /// Seconds it has been loose.
    pub age: f32,
    /// Times a lump has rolled off what it came down on ([`Loose::step`]).
    rolls: u8,
    /// Come to rest, it keeps its own cells and the way it lies ([`Rested`]): a log.
    keeps: bool,
    /// It has touched, and is going over about where it touched until its weight is over
    /// what it rests on: the place on it (as it was made), how fast it turns and how far
    /// it has (radians), and how many times it has touched.
    tips: Option<(Vec3, f32, f32, u8, Vec3)>,
    /// A great piece part way into the world ([`SPREAD`]): the next row of cells of it to
    /// put, how many are put and lost so far, and how fast it came down.
    landing: Option<(i32, usize, usize, f32)>,
}

impl Piece {
    /// Which tree it is, if it is one gone over or settling ([`Thing::id`]).
    pub fn of(&self) -> Option<u32> {
        self.thing
    }
    pub fn now(&self, was: Vec3) -> Vec3 {
        self.pivot + self.shift + self.turn * (was - self.pivot)
    }
    fn was(&self, now: Vec3) -> Vec3 {
        self.pivot + self.turn.inverse() * (now - self.pivot - self.shift)
    }
}

/// Whether cells are wood, most of them (leaf is not counted either way).
fn woody(cells: &Cells<Part>) -> bool {
    let of = |want: &dyn Fn(Block) -> bool| cells.values().filter(|p| match p { Part::Whole(b) => want(*b), Part::Bits(bits) => bits.iter().any(|b| want(*b)) }).count();
    of(&|b| b.wood() || b == Block::Billet) * 2 > of(&|b| b.solid() && !b.foliage())
}

/// What cells weigh, kilograms: each by what it is made of.
fn mass(cells: &Cells<Part>) -> f32 {
    cells.values().map(|part| match part {
        Part::Whole(b) => crate::matter::density(*b) * CELL * CELL * CELL,
        Part::Bits(bits) => bits.iter().map(|b| crate::matter::density(*b)).sum::<f32>() * BIT * BIT * BIT,
    }).sum()
}

/// What has come down: where, how many cells of it were put back, and how many it had.
#[derive(Clone, Copy, Debug)]
pub struct Landed {
    pub at: Vec3,
    pub cells: usize,
    pub put: usize,
    /// Whole cells of what bears (wood, stone) that found no room where it came down.
    pub lost: usize,
    /// Metres a second it was going.
    pub speed: f32,
    /// A tree coming to rest after its top has already struck: that was the crash, and this
    /// is not heard again nor a second blow.
    pub settled: bool,
    /// What it weighs, kilograms (`matter::density`).
    pub mass: f32,
    /// It is wood, most of it: a log does not sound as a stone does.
    pub wood: bool,
    /// A long thing gone over (a tree): its other end, and how far to either side of the
    /// line from there to `at` it strikes. A lump strikes round `at`, by its size.
    pub along: Option<(Vec3, f32)>,
}

/// What a cut did: the chunks that changed; and whatever whole shapes it bit into, which are
/// now cells drawn as cells: each tree (which of the forms given, and its number there) and
/// each boulder (which form, and which of its stones).
#[derive(Default)]
pub struct Cut {
    pub changed: HashSet<IVec3>,
    pub trees: Vec<(usize, u32)>,
    pub stones: Vec<(usize, usize)>,
    /// How many cells were made wrought.
    pub worked: usize,
    /// How much was cut away, in bits: of earth, of wood and of stone.
    pub taken: [u32; 3],
}

/// A tree or a boulder that has been cut into, as little as says all of it: what it grew as,
/// what has been cut out of it, and where it lies if it has come down.
#[derive(Clone)]
pub struct Thing {
    pub id: u32,
    /// The shape it grew: a tree's limbs and leaf, or a boulder's stone.
    pub limbs: Vec<crate::plants::Limb>,
    pub clumps: Vec<crate::plants::Clump>,
    pub stone: Option<crate::plants::Stone>,
    /// Every bite taken out of it: where, how far across, and of what stuff.
    pub cuts: Vec<(Vec3, f32, Option<Stuff>)>,
    /// Come down: what it turned about, how far that moved, and how it is turned (a point of
    /// it that stood at `p` lies at pivot + shift + turn (p - pivot), as [`Piece::now`]).
    pub lies: Option<(Vec3, Vec3, Quat)>,
    /// Blows of an axe taken standing (or of a pick, a boulder), and whether its boughs and
    /// leaf have been knocked off since it fell ([`Loose::chop`]). From then on it is
    /// timber, and nothing more is kept of it: [`Loose::timber`].
    pub blows: u32,
    pub limbed: bool,
    /// A tree that has been cut through: it is going over, or is down.
    pub felled: bool,
    /// It is drawn as the cells it is: cut into with something that carves ([`Loose::cut`]),
    /// or broken into logs. Until then a tree an axe is at is drawn as the tree it grew as,
    /// standing, going over and lying, by whoever draws ([`Block::whole`]): at its first
    /// blow it had become cells a quarter of a metre a side, and lying it had been asked
    /// cell by cell where it was, and the developer, having felled one: "even at the
    /// beginning it doesnt really look like the tree anymore", "the tree also needs to
    /// stay ... roughly the same shape as it was, which should roughly be straight, when
    /// it falls" (2026-11-13).
    pub cells: bool,
    /// The notch an axe has cut in it as it stands, for whoever draws it (its cells are
    /// cut, and are not drawn): the middle of its trunk there, the way the blows go in,
    /// how thick it is there and how deep the notch has got.
    pub notch: Option<(Vec3, Vec3, f32, f32)>,
    /// Its stump stands as it grew, and is drawn so ([`Thing::stumped`]): from when it is
    /// cut through until an axe is taken to the stump. (Made cells drawn as cells when the
    /// tree went, it was a heap of them beside a tree drawn as a tree.)
    pub stump: bool,
}

impl Thing {
    /// A felled tree as it is to be drawn while it is still a tree ([`Thing::cells`]): its
    /// limbs and leaf as it grew, from where it was cut through up, with no roots; and,
    /// once its boughs and leaf are knocked off, its trunk and heavy limbs alone. In the
    /// places it grew in: whoever draws it moves it to where it is (`lies`, or the piece
    /// it is while it falls).
    pub fn form(&self) -> Form {
        let stout = self.limbs.iter().map(|l| l.ra.max(l.rb)).fold(0.0, f32::max) * 0.34;
        let trunk = self.limbs.iter().position(|l| l.roots > 0).unwrap_or(0);
        let limbs = self.limbs.iter().enumerate().filter(|(_, l)| !self.limbed || l.ra.max(l.rb) >= stout).map(|(n, l)| {
            if n != trunk {
                return *l;
            }
            // (From a cell and a half over the middle of its notch: `Loose::chop`.)
            let up = (((crate::plants::Limb::UNDER + 0.9) / (l.b.y - l.a.y).max(0.1)).min(0.8) + CELL * 1.5 / l.a.distance(l.b).max(0.1)).min(0.95);
            crate::plants::Limb { a: l.a.lerp(l.b, up), ra: l.ra + (l.rb - l.ra) * up, roots: 0, ..*l }
        }).collect();
        Form { limbs, clumps: if self.limbed { Vec::new() } else { self.clumps.clone() }, stones: Vec::new() }
    }
    /// Its stump as it is to be drawn while it stands ([`Thing::stump`]): its trunk from its
    /// roots up to where it was cut through.
    pub fn stumped(&self) -> Form {
        let limbs = self.limbs.iter().find(|l| l.roots > 0).or(self.limbs.first()).map(|l| {
            let up = ((crate::plants::Limb::UNDER + 0.9) / (l.b.y - l.a.y).max(0.1)).min(0.8) - CELL * 0.5 / l.a.distance(l.b).max(0.1);
            crate::plants::Limb { b: l.a.lerp(l.b, up), rb: l.ra + (l.rb - l.ra) * up, ..*l }
        });
        Form { limbs: limbs.into_iter().collect(), clumps: Vec::new(), stones: Vec::new() }
    }
    /// The length of its trunk an axe is at as it stands, between its stump and the rest: for
    /// whoever draws it, with the notch cut out of it.
    pub fn band(&self) -> Form {
        let limbs = self.limbs.iter().find(|l| l.roots > 0).or(self.limbs.first()).map(|l| {
            let at = ((crate::plants::Limb::UNDER + 0.9) / (l.b.y - l.a.y).max(0.1)).min(0.8);
            let (from, to) = (at - CELL * 0.5 / l.a.distance(l.b).max(0.1), (at + CELL * 1.5 / l.a.distance(l.b).max(0.1)).min(0.95));
            crate::plants::Limb { a: l.a.lerp(l.b, from), b: l.a.lerp(l.b, to), ra: l.ra + (l.rb - l.ra) * from, rb: l.ra + (l.rb - l.ra) * to, roots: 0, ..*l }
        });
        Form { limbs: limbs.into_iter().collect(), clumps: Vec::new(), stones: Vec::new() }
    }
    /// Whether a ball reaches it as it stood.
    fn reached(&self, middle: Vec3, radius: f32) -> bool {
        self.limbs.iter().any(|l| from_line(middle, l.a, l.b) < radius + l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL) || self.stone.as_ref().is_some_and(|s| ((middle - s.middle) / (s.radii + radius + CELL)).length_squared() < 1.0)
    }
    /// Whether a place is in or about it as it lies with its boughs on (a tree felled and not
    /// yet limbed): within a couple of metres of a limb of it, or in a mass of its leaf.
    fn lain_about(&self, p: Vec3) -> bool {
        let Some((pivot, shift, turn)) = self.lies.filter(|_| !self.limbed) else { return false };
        let was = pivot + turn.inverse() * (p - pivot - shift);
        self.limbs.iter().any(|l| from_line(was, l.a, l.b) < l.ra.max(l.rb) + 2.0) || self.clumps.iter().any(|c| ((was - c.middle) / (c.radii + 1.0)).length_squared() < 1.0)
    }
    /// Whether a place is in its trunk as it lies with its boughs on.
    fn lain_trunk(&self, p: Vec3) -> bool {
        let Some((pivot, shift, turn)) = self.lies.filter(|_| !self.limbed) else { return false };
        let was = pivot + turn.inverse() * (p - pivot - shift);
        self.limbs.iter().find(|l| l.roots > 0).or(self.limbs.first()).is_some_and(|l| from_line(was, l.a, l.b) < l.ra.max(l.rb) + CELL)
    }
    /// Where it stands on the ground: the foot of its rooted trunk, or of its lowest limb.
    fn foot(&self) -> Option<Vec3> {
        self.limbs.iter().find(|l| l.roots > 0).or_else(|| self.limbs.iter().min_by(|a, b| a.a.y.total_cmp(&b.a.y))).map(|l| l.a)
    }
}

/// Something done to the world by hand: a cut, or a thing put down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Cut { middle: Vec3, radius: f32, push: Vec3, stuff: Option<Stuff> },
    Put { middle: Vec3, radius: f32, block: Block },
    Chop { at: Vec3, push: Vec3 },
    Chip { at: Vec3, push: Vec3 },
    /// A hunk taken up, and one put down.
    Lift { at: Vec3 },
    Lay { at: Vec3, block: Block },
    /// A thing lying loose shoved along.
    Shove { at: Vec3, push: Vec3 },
}

impl Edit {
    /// Where it was done.
    pub fn at(&self) -> Vec3 {
        match *self {
            Edit::Cut { middle, .. } | Edit::Put { middle, .. } => middle,
            Edit::Chop { at, .. } | Edit::Chip { at, .. } | Edit::Lift { at } | Edit::Lay { at, .. } | Edit::Shove { at, .. } => at,
        }
    }
}

/// Does a journal's edits to a world again, each at its moment, and lets what falls come to
/// rest. `forms` are the shapes of what grows there, as the land was made.
pub fn replay(world: &mut World, forms: &[&Form], journal: &[(u64, Edit)]) -> Loose {
    let mut again = Replay::default();
    while !again.advance(world, forms, journal, std::time::Duration::MAX) {}
    again.loose
}

/// A journal being done again, a little at a time: for whoever cannot spare the whole of it
/// in one go (the game, on a frame: a tree felled again is some tens of milliseconds).
#[derive(Default)]
pub struct Replay {
    pub loose: Loose,
    next: usize,
    settled: u32,
}

impl Replay {
    /// Does as much more of it as `budget` allows. Says whether it is all done.
    pub fn advance(&mut self, world: &mut World, forms: &[&Form], journal: &[(u64, Edit)], budget: std::time::Duration) -> bool {
        let began = std::time::Instant::now();
        while let Some((at, edit)) = journal.get(self.next) {
            while self.loose.steps < *at {
                self.loose.step(world, crate::DT);
                if began.elapsed() > budget {
                    return false;
                }
            }
            match *edit {
                Edit::Cut { middle, radius, push, stuff } => drop(self.loose.cut(world, middle, radius, push, forms, stuff)),
                Edit::Put { middle, radius, block } => drop(self.loose.put(world, middle, radius, block)),
                Edit::Chop { at, push } => drop(self.loose.chop(world, at, push, forms)),
                Edit::Chip { at, push } => drop(self.loose.chip(world, at, push, forms)),
                Edit::Lift { at } => drop(self.loose.lift(world, at)),
                Edit::Lay { at, block } => drop(self.loose.lay(world, at, block)),
                Edit::Shove { at, push } => drop(self.loose.shove(world, at, push)),
            }
            self.next += 1;
            if began.elapsed() > budget {
                return false;
            }
        }
        while !self.loose.pieces.is_empty() && self.settled < 60 * crate::TICKS {
            self.loose.step(world, crate::DT);
            self.settled += 1;
            if began.elapsed() > budget {
                return false;
            }
        }
        true
    }
}

/// Everything loose in a world, and what has landed since it was last asked for.
#[derive(Default, Clone)]
pub struct Loose {
    pub pieces: Vec<Piece>,
    /// For the length of one felling cut: no body is too large to be looked through to its
    /// end ([`HELD`]). A tree is known to be a tree.
    whole: bool,
    pub landed: Vec<Landed>,
    /// The same, once the simulation has seen who was under each (`Sim::step`): for whoever
    /// makes the sound of it.
    pub heard: Vec<Landed>,
    /// For whoever shows and sounds it, to take as they are noted: each cut (where, and how
    /// much of earth, wood and stone it took), and each thing that came loose (where it
    /// stood, how many cells, and whether it is going over like a tree).
    pub cuts: Vec<(Vec3, [u32; 3])>,
    pub begun: Vec<(Vec3, usize, bool)>,
    /// What a blow knocked off a fallen tree, for whoever shows it to throw and let fade:
    /// where, and of what.
    pub shed: Vec<(Vec3, Block)>,
    /// Every tree and boulder that has been cut into ([`Thing`]).
    pub things: Vec<Thing>,
    /// Hunks that have come loose where they had just come to rest, by their corner, and
    /// how many times running: one that does it four times is left where it is
    /// ([`Loose::loosen`]).
    restless: HashMap<IVec3, u8>,
    /// What lies at rest as itself ([`Rested`]), and whose each cell of the world is that
    /// one of them is in.
    pub rested: Vec<Rested>,
    pub owner: HashMap<IVec3, u32>,
    /// Every edit made, with how many steps had been taken when it was ([`replay`]).
    pub journal: Vec<(u64, Edit)>,
    steps: u64,
    next: u32,
}

/// Bears weight: solid, not leaf, and not part of a whole shape still untouched.
fn bears(b: Block) -> bool {
    b.solid() && !b.foliage() && !b.shaped()
}

/// Whether a cell has anything in it that bears weight, or (with `leaf`) any leaf.
fn has(world: &World, cell: IVec3, leaf: bool) -> bool {
    let want = |b: Block| if leaf { b.foliage() && !b.shaped() } else { bears(b) };
    match world.get(cell) {
        Block::Broken => world.bits_at(cell).is_some_and(|bits| bits.iter().any(|b| want(*b))),
        whole => want(whole),
    }
}

/// How many cells water may run into from one cut: a pit or a length of trench, not a valley.
const FLOWS: usize = 6_000;

/// Water beside or in a box of cells runs into the air it can reach: sideways, down, and up
/// no higher than it stood. What grows low (grass, a flower) is drowned. Gives the chunks
/// changed.
pub fn flow(world: &mut World, lo: IVec3, hi: IVec3) -> HashSet<IVec3> {
    let mut queue: VecDeque<IVec3> = VecDeque::new();
    for y in lo.y - 1..=hi.y + 1 {
        for z in lo.z - 1..=hi.z + 1 {
            for x in lo.x - 1..=hi.x + 1 {
                let cell = IVec3::new(x, y, z);
                if world.get(cell) == Block::Water {
                    queue.push_back(cell);
                }
            }
        }
    }
    // (It finds its own level: up, too, as far as the highest of the water it came from. A
    // pit joined to a pool below the pool's top fills to the top.)
    let level = queue.iter().map(|c| c.y).max().unwrap_or(i32::MIN);
    let (mut changed, mut filled) = (HashSet::new(), 0);
    while let Some(cell) = queue.pop_front() {
        for side in [IVec3::NEG_Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y] {
            let next = cell + side;
            let there = world.get(next);
            if filled < FLOWS && next.y <= level && (there == Block::Air || there.sprig()) {
                world.set(next, Block::Water);
                changed.insert(next.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                queue.push_back(next);
                filled += 1;
            }
        }
    }
    changed
}

/// How far a point is from the line between two others.
/// How far a point is outside a limb's wood (inside it, less than nothing): from its line,
/// less how thick it is there. (By its thickest anywhere, a great tree that tapers from a
/// metre to a hand's breadth owned every cell within a metre of its top, a birch's trunk
/// beside it among them: the birch's own top was no longer joined to its foot and came
/// down by itself when the birch was first cut into, 2026-11-13.)
fn outside(p: Vec3, limb: &(Vec3, Vec3, f32, f32)) -> f32 {
    let (a, b, ra, rb) = *limb;
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along) - (ra + (rb - ra) * along)
}

fn from_line(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along)
}

/// `LOOSE_TRACE=1`: a line of what was done, for whoever is finding out why.
fn trace(what: std::fmt::Arguments) {
    if std::env::var_os("LOOSE_TRACE").is_some() {
        eprintln!("loose: {what}");
    }
}

/// Takes what a tool's stuff is out of a box of cells, where `of` says a place is its own:
/// whole cells of it, and broken cells that hold nothing else. Says how much, in bits.
fn emptied(world: &mut World, lo: IVec3, hi: IVec3, stuff: impl Fn(Block) -> bool, of: impl Fn(Vec3) -> bool, changed: &mut HashSet<IVec3>) -> u32 {
    let mut bits = 0;
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                if !of((cell.as_vec3() + 0.5) * CELL) {
                    continue;
                }
                // (A broken cell that holds something else too gives up only its bits of this.)
                if world.get(cell) == Block::Broken && world.bits_at(cell).is_some_and(|b| b.iter().any(|b| stuff(*b)) && !b.iter().all(|b| *b == Block::Air || stuff(*b))) {
                    for i in 0..SPLIT * SPLIT * SPLIT {
                        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / SPLIT % SPLIT, i / (SPLIT * SPLIT));
                        if stuff(world.bit(bit)) {
                            world.set_bit(bit, Block::Air);
                            bits += 1;
                        }
                    }
                    changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                    continue;
                }
                let held = match world.get(cell) {
                    Block::Broken => world.bits_at(cell).filter(|b| b.iter().all(|b| *b == Block::Air || stuff(*b))).map_or(0, |b| b.iter().filter(|b| stuff(**b)).count() as u32),
                    b if stuff(b) => (SPLIT * SPLIT * SPLIT) as u32,
                    _ => 0,
                };
                if held > 0 {
                    world.set(cell, Block::Air);
                    changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                    bits += held;
                }
            }
        }
    }
    bits
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Every cell round a cell: by a face, an edge or a corner. What bears is joined through any
/// of them: a limb a hand thick that climbs aslant passes from cell to cell by their edges,
/// and by faces alone half the limbs of a wood hang in the air unjoined.
fn round() -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| IVec3::new(x, y, z)))).filter(|d| *d != IVec3::ZERO)
}

/// Where a lump of earth let go at `middle` comes to rest: straight down onto what is under
/// it, and from there down any side that falls away by more than `STEEP`, a lump's width at
/// a time, until it lies on ground no steeper. From the cells alone, in a fixed order: the
/// same put on the same world rests in the same place, which the journal's replay needs.
/// (Until 2026-11-13 it stayed where it was let go: each lump on the one before, earth
/// could be built out into the air like stone.)
fn heaped(world: &World, middle: Vec3, radius: f32) -> Vec3 {
    const STEEP: f32 = 0.45;
    let under = |at: Vec3| world.raycast(at, Vec3::NEG_Y, 48.0);
    let Some(mut drop) = under(middle) else { return middle };
    let mut at = middle;
    for _ in 0..24 {
        let sides = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].map(|d| at + d * radius * 2.0);
        // (A side it can get to: not through what stands beside it.)
        let lower = sides.into_iter().filter(|s| !world.get((*s / CELL).floor().as_ivec3()).solid()).filter_map(|s| under(s).map(|d| (d, s))).filter(|(d, _)| *d > drop + STEEP).max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((d, side)) = lower else { break };
        (drop, at) = (d, side);
    }
    // (Sunk a third into what it lies on, so it is one with it.)
    at - Vec3::Y * (drop - radius * 0.6).max(0.0)
}

#[cfg(test)]
mod tests;
