//! What nothing holds comes loose, falls, breaks, tips and comes to rest: the step of every
//! piece in the air, and how what lands is the world's again.

use super::*;

/// Turns a piece to the nearest of the ways a cube can lie and moves it to whole cells.
pub(super) fn square(piece: &mut Piece) {
    let m = glam::Mat3::from_quat(piece.turn);
    let snap = |v: Vec3, not: Option<usize>| -> (usize, Vec3) {
        let a = v.abs().to_array();
        let i = (0..3).filter(|i| Some(*i) != not).max_by(|x, y| a[*x].total_cmp(&a[*y])).unwrap_or(0);
        let mut out = [0.0; 3];
        out[i] = v.to_array()[i].signum();
        (i, Vec3::from(out))
    };
    let (ix, x) = snap(m.x_axis, None);
    let (_, y) = snap(m.y_axis, Some(ix));
    let turn = Quat::from_mat3(&glam::Mat3::from_cols(x, y, x.cross(y))).normalize();
    // (Where the piece's own origin now is: that is what must be on a cell's corner.)
    let at = piece.pivot + piece.shift - turn * piece.pivot;
    // (Hunks to the squares hunks lie on: [`hunk_at`].)
    let wide = if piece.hunk { HUNK as f32 } else { 1.0 };
    let to = Vec3::new((at.x / CELL / wide).round() * wide, (at.y / CELL - 0.25).ceil(), (at.z / CELL / wide).round() * wide) * CELL;
    (piece.turn, piece.shift) = (turn, piece.shift + (to - at));
}

/// Puts a piece back into the world where it now lies: every cell of the world it reaches is
/// asked what of the piece is there, so it goes back without gaps however it is turned.
/// Only into what is not solid already. Gives how many cells were written, their chunks, the
/// box of cells it reached, and how many whole cells of what bears found no room.
pub(super) fn land(world: &mut World, piece: &Piece, rows: Option<(i32, i32)>) -> (usize, HashSet<IVec3>, IVec3, IVec3, usize) {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for cell in piece.cells.keys() {
        for corner in 0..8 {
            let p = piece.now((*cell + IVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1)).as_vec3() * CELL);
            (lo, hi) = (lo.min(p), hi.max(p));
        }
    }
    let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
    let at = |now: Vec3| (piece.was(now) / CELL).floor().as_ivec3();
    let (mut put, mut changed, mut lost) = (0, HashSet::new(), 0);
    // (Along a row of the world, where each cell was in the piece is a step on from the last:
    // one turn a row and an add a cell. And nothing of the piece is outside the box of its
    // own cells: no map is asked. Turned and asked cell by cell, a great tree's million
    // cells of box were 35 ms of the step it landed in.)
    let (first, last) = piece.cells.keys().fold((IVec3::MAX, IVec3::MIN), |(a, b), c| (a.min(*c), b.max(*c)));
    let along = piece.turn.inverse() * Vec3::X * CELL;
    // (All of it, or the rows asked for: a great piece is put a few rows a step.)
    let (from, to) = rows.map_or((lo.y, hi.y), |(from, many)| (from, (from + many - 1).min(hi.y)));
    for y in from..=to {
        for z in lo.z..=hi.z {
            let mut was = piece.was((IVec3::new(lo.x, y, z).as_vec3() + 0.5) * CELL);
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                let from = (was / CELL).floor().as_ivec3();
                was += along;
                if from.cmplt(first).any() || from.cmpgt(last).any() {
                    continue;
                }
                let Some(part) = piece.cells.get(&from) else { continue };
                let there = world.get(cell);
                match part {
                    // (Wood and stone go where leaf is: a limb fallen through a crown is not
                    // cut in two by it.)
                    Part::Whole(b) if (!there.solid() || (there.foliage() && !b.foliage())) && there != Block::Broken => {
                        world.set(cell, *b);
                        put += 1;
                    }
                    // (Into a cell that is part filled, what of it there is room for, to the
                    // bit: the ground a felled trunk lies on is such cells, and a log cut
                    // from it that dropped a hand's width found no room for two thirds of
                    // itself, which was gone. Seed 2, 2026-11-13.)
                    Part::Whole(b) if there == Block::Broken && !b.hunk() => {
                        let mut any = false;
                        for i in 0..SPLIT * SPLIT * SPLIT {
                            let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                            if !world.bit(bit).solid() {
                                world.set_bit(bit, *b);
                                any = true;
                            }
                        }
                        put += any as usize;
                        lost += (!any && bears(*b)) as usize;
                    }
                    // (What bears and found no room: wood into wood, stone into the ground.
                    // Leaf that comes down into leaf or onto the ground is only crushed.)
                    // (It lies on what is there, in the first room over it: a boulder that
                    // came down lost a cell of stone for every cell of ground it met by
                    // less than half, a fifth of itself, 2026-11-13.)
                    Part::Whole(b) => {
                        if bears(*b) {
                            // (On what bears: over leaf it would be left in the air.)
                            match (1..=4).map(|up| cell + IVec3::Y * up).find(|c| world.get(*c) == Block::Air && has(world, *c - IVec3::Y, false) && !piece.cells.contains_key(&at((c.as_vec3() + 0.5) * CELL))) {
                                Some(room) => {
                                    world.set(room, *b);
                                    changed.insert(room.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                                    put += 1;
                                }
                                None => lost += 1,
                            }
                        }
                        continue;
                    }
                    Part::Bits(_) => {
                        // To the bit: each bit of this cell is asked for on its own.
                        let mut any = false;
                        for i in 0..SPLIT * SPLIT * SPLIT {
                            let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                            let was = piece.was((bit.as_vec3() + 0.5) * BIT);
                            let from = (was / CELL).floor().as_ivec3();
                            let block = match piece.cells.get(&from) {
                                Some(Part::Whole(b)) => *b,
                                Some(Part::Bits(bits)) => {
                                    let l = ((was / BIT).floor().as_ivec3()).rem_euclid(IVec3::splat(SPLIT));
                                    bits[(l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize]
                                }
                                None => Block::Air,
                            };
                            if block.solid() && !world.bit(bit).solid() {
                                world.set_bit(bit, block);
                                any = true;
                            } else if bears(block) && let Some(room) = (1..=2 * SPLIT).map(|up| bit + IVec3::Y * up).find(|b| !world.bit(*b).solid() && bears(world.bit(*b - IVec3::Y))) {
                                // (And a bit of stone that finds its place taken, the same.)
                                world.set_bit(room, block);
                                changed.insert(room.div_euclid(IVec3::splat(SPLIT)).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                                any = true;
                            }
                        }
                        put += any as usize;
                    }
                }
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
    }
    (put, changed, lo, hi, lost)
}

impl Loose {
    /// Looks through what is solid in a box of cells for anything no longer held, and takes
    /// each such thing out of the world as a piece.
    pub fn loosen(&mut self, world: &mut World, lo: IVec3, hi: IVec3, push: Vec3) -> HashSet<IVec3> {
        let mut changed = HashSet::new();
        let began = std::time::Instant::now();
        let wide = (hi - lo + 1).as_i64vec3().element_product() > WIDE;
        // Every cell looked through, and in which search: one that meets an earlier search's
        // cells is of the same body, and that body was held (a loose one is out of the world).
        let mut seen: Cells<u32> = Cells::default();
        let mut search = 0;
        // A tree holds only itself up. Where two that have been cut into stand close, a cell
        // is the one's whose wood it is nearest, and what is one tree's is not joined to
        // what is another's: a birch cut clean through hung by its twigs from the tree half
        // a metre off, which the same cut had made cells, and never fell (seed 1,
        // 2026-11-13). Only looked into where two such trees stand.
        let trees: Vec<(u32, Vec<(Vec3, Vec3, f32, f32)>)> = self.things.iter().filter(|t| t.lies.is_none() && !t.limbs.is_empty()).map(|t| (t.id, t.limbs.iter().map(|l| (l.a, l.b, l.ra, l.rb)).collect())).collect();
        let mut whose: HashMap<IVec3, u32> = HashMap::new();
        let mut of = |cell: IVec3| -> u32 {
            *whose.entry(cell).or_insert_with(|| {
                let p = (cell.as_vec3() + 0.5) * CELL;
                trees.iter().filter_map(|(id, limbs)| limbs.iter().map(|l| outside(p, l)).min_by(|x, y| x.total_cmp(y)).map(|d| (d, *id))).filter(|(d, _)| *d < 0.5).min_by(|x, y| x.0.total_cmp(&y.0)).map_or(0, |(_, id)| id)
            })
        };
        let rivals = trees.len() > 1;
        // Where to begin from: every cell of the place; or of a wide one, taken chunk by chunk,
        // only what is neither air nor the land (above): a tree as it lies is in a box of a
        // million cells, nearly all of them air or ground, and asking the world for each was
        // 40 to 60 ms.
        let starts: Vec<IVec3> = if wide {
            let size = cube_voxel::CHUNK;
            let (from, to) = (lo.div_euclid(IVec3::splat(size)), hi.div_euclid(IVec3::splat(size)));
            let mut starts = Vec::new();
            for cy in from.y..=to.y {
                for cz in from.z..=to.z {
                    for cx in from.x..=to.x {
                        let at = IVec3::new(cx, cy, cz);
                        let Some(chunk) = world.chunks.get(&at) else { continue };
                        let (a, b) = ((lo - at * size).max(IVec3::ZERO), (hi - at * size).min(IVec3::splat(size - 1)));
                        for y in a.y..=b.y {
                            for z in a.z..=b.z {
                                for x in a.x..=b.x {
                                    let block = chunk.get(IVec3::new(x, y, z));
                                    // (What bears, or may: leaf is begun from by nothing.)
                                    if block == Block::Broken || bears(block) && !block.ground() {
                                        starts.push(at * size + IVec3::new(x, y, z));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            starts.sort_unstable_by_key(|c| (c.y, c.z, c.x));
            starts
        } else {
            (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))).collect()
        };
        for from in starts {
            if seen.contains_key(&from) || !has(world, from, false) {
                continue;
            }
            // (Over a wide place, not from the land itself: the land is looked at
            // where it was dug, which is a small place. Round a great tree as it
            // lies every cell of the ground under it was looked through, 460,000 to
            // 620,000 of them, each time it landed, was limbed and was bucked: 60 to
            // 170 ms, a frame lost each time. What stands on the land still finds
            // it under itself.)
            if wide && world.get(from).ground() {
                continue;
            }
            search += 1;
            // Through what bears weight, until it is plainly the land's or it ends.
            let (mut own, mut queue, mut held, mut land) = (vec![from], VecDeque::from([from]), false, 0);
            seen.insert(from, search);
            'through: while let Some(cell) = queue.pop_front() {
                // (Earth is joined to earth by its faces, which is enough for a
                // thing with no thin parts and a quarter of the looking.)
                let earth = world.get(cell).ground();
                land += earth as usize;
                if land > LAND {
                    held = true;
                    break;
                }
                for side in round().filter(|d| !earth || d.abs().element_sum() == 1) {
                    let next = cell + side;
                    if !has(world, next, false) {
                        // (What lies on a boulder nobody has cut lies on something:
                        // a boulder as it grew is no body to look through, and a
                        // hunk that came down on one was let go again the moment it
                        // landed, for as long as anyone waited. Seed 5, 2026-11-13.)
                        // (Not so a tree nobody has cut: a felled one would hang on
                        // its neighbour's boughs.)
                        // (A hunk does lie on one's foot: it is no part of any tree.)
                        // (Its outside is cells part filled: by what is in them.)
                        let under = |of: &dyn Fn(Block) -> bool| match world.get(next) {
                            Block::Broken => world.bits_at(next).is_some_and(|bits| bits.iter().any(|b| of(*b))),
                            b => of(b),
                        };
                        if side == IVec3::NEG_Y && (under(&|b| b == Block::Rock) || world.get(cell).hunk() && under(&|b| b.wood())) {
                            held = true;
                            break 'through;
                        }
                        continue;
                    }
                    // A hunk is a body of its own, held by what is under it and by
                    // nothing beside it: joined as cells are, one put against
                    // another that lay on the ground hung in the air (four of a
                    // tree's twelve did, `cargo world hunks`, 2026-11-13).
                    let (this, that) = (world.get(cell), world.get(next));
                    if (this.hunk() || that.hunk()) && side != IVec3::NEG_Y && !(this == that && hunk_at(world, cell).map(|h| h.0) == hunk_at(world, next).map(|h| h.0)) {
                        continue;
                    }
                    if rivals && !earth && !world.get(next).ground() {
                        let (mine, its) = (of(cell), of(next));
                        if mine != 0 && its != 0 && mine != its {
                            continue;
                        }
                    }
                    match seen.get(&next) {
                        Some(by) if *by == search => continue,
                        Some(_) => {
                            held = true;
                            break 'through;
                        }
                        None => {}
                    }
                    seen.insert(next, search);
                    own.push(next);
                    queue.push_back(next);
                }
                if own.len() > HELD && !self.whole {
                    held = true;
                    break;
                }
            }
            if !held {
                // Each hunk of it falls as itself, apart from the rest: a log's end
                // that lay on one came down with it as one piece, which landed as
                // cells do, and the hunk's cells that met the ground were put
                // wherever there was room (seed 7, 2026-11-13).
                // And what lay at rest as itself falls as itself, all of it.
                let mut again: Vec<u32> = own.iter().filter_map(|c| self.owner.get(c).copied()).collect();
                again.sort_unstable();
                again.dedup();
                // (A thing at rest is one body: a crumb of it that nothing holds, a
                // bit that found room apart from the rest, does not let all of it
                // go. A log with one bit of itself in a bush came loose, fell where
                // it lay and came to rest, for ever: what the developer saw as a
                // hunk "infinitely falling on itself", found 2026-11-13 on a great
                // tree at -32,-40.)
                again.retain(|id| {
                    let all = self.rested.iter().find(|r| r.id == *id).map_or(0, |r| r.placed.len());
                    let loose = own.iter().filter(|c| self.owner.get(*c) == Some(id)).count();
                    if loose * 2 < all {
                        trace(format_args!("loosen: {loose} of the {all} cells of piece {id} at rest are held by nothing, and it stays"));
                    }
                    loose * 2 >= all
                });
                for id in again {
                    changed.extend(self.rouse(world, id));
                }
                let own: Vec<IVec3> = own.into_iter().filter(|c| world.get(*c) != Block::Air).collect();
                let mut apart: HashMap<Option<IVec3>, Vec<IVec3>> = HashMap::new();
                for cell in own {
                    apart.entry(hunk_at(world, cell).map(|h| h.0)).or_default().push(cell);
                }
                let mut apart: Vec<(Option<IVec3>, Vec<IVec3>)> = apart.into_iter().collect();
                apart.sort_by_key(|(corner, _)| corner.map(|c| (c.y, c.z, c.x)));
                for (corner, cells) in apart {
                    // The leaf of a tree that lies felled is the tree's: it is drawn as
                    // the whole tree it was until an axe limbs it, and its leaf bears
                    // nothing, so each mass of it with a few twigs in it was a body
                    // nothing held, came loose and fell away from the tree that was
                    // drawn (four of 1,200 to 2,000 cells off one great tree, a step
                    // of 100 ms, and cells nobody could see on the ground).
                    if corner.is_none() && cells.iter().all(|c| match world.get(*c) { Block::Broken => world.bits_at(*c).is_some_and(|b| b.iter().all(|b| !b.solid() || b.whole())), b => b.whole() }) && cells.first().is_some_and(|c| self.things.iter().any(|t| t.lain_about((c.as_vec3() + 0.5) * CELL))) && !cells.iter().any(|c| self.things.iter().any(|t| t.lain_trunk((c.as_vec3() + 0.5) * CELL))) {
                        continue;
                    }
                    // A hunk that comes loose again from where it has just come to
                    // rest, four times running, is left there: it is on something,
                    // whatever the rules above make of it, and let go it would fall
                    // onto the same place for ever. (What the developer saw fall for
                    // ever, 2026-11-13, was found to be a log let go for a crumb of
                    // itself, below; this stays for a hunk that does the like, and
                    // `LOOSE_TRACE=1` says what was under it.)
                    if let Some(corner) = corner {
                        let times = self.restless.entry(corner).or_insert(0);
                        *times += 1;
                        if *times > 4 {
                            if *times == 5 {
                                trace(format_args!("a hunk at {corner} would not settle, and is left there: under it {:?}", hunk().filter(|d| d.y == 0).map(|d| world.get(corner + d - IVec3::Y)).collect::<Vec<_>>()));
                            }
                            continue;
                        }
                    }
                    changed.extend(self.take(world, cells, push));
                }
            }
        }
        if began.elapsed().as_millis() >= 8 {
            trace(format_args!("loosen: {:.0} ms looking at {:?} cells, {} looked through", began.elapsed().as_secs_f32() * 1000.0, hi - lo + 1, seen.len()));
        }
        changed
    }

    /// Takes cells out of the world as one piece, with the leaves that are theirs.
    pub(super) fn take(&mut self, world: &mut World, mut own: Vec<IVec3>, push: Vec3) -> HashSet<IVec3> {
        let began = std::time::Instant::now();
        // Its leaves: what foliage is within a few cells of it, through foliage.
        let mut near: Cells<u8> = own.iter().map(|c| (*c, 0)).collect();
        // (A hunk has none: one that came down beside leaf lying on the ground took it up,
        // was no longer hunks and nothing else, and landed as cells do. Seed 7, 2026-11-13.)
        let bare = own.iter().all(|c| world.get(*c).hunk());
        let mut queue: VecDeque<IVec3> = if bare { VecDeque::new() } else { own.iter().copied().collect() };
        while let Some(cell) = queue.pop_front() {
            let far = near[&cell];
            if far >= LEAFY {
                continue;
            }
            for side in SIDES {
                let next = cell + side;
                if !near.contains_key(&next) && has(world, next, true) && !has(world, next, false) {
                    near.insert(next, far + 1);
                    own.push(next);
                    queue.push_back(next);
                }
            }
        }
        let mut cells = Cells::with_capacity_and_hasher(own.len(), Default::default());
        let mut changed = HashSet::new();
        for cell in own {
            // (Stone that comes loose is rubble from then on, not the land: the land is drawn
            // eased into the ground round it and takes its colour, and a lump of it lying
            // in a meadow was drawn half grass.)
            let rubble = |b: Block| if matches!(b, Block::Stone | Block::Slate | Block::Chalk) { Block::Shard } else { b };
            let part = match world.get(cell) {
                Block::Broken => world.bits_at(cell).map(|b| Part::Bits(Box::new(b.map(rubble)))),
                Block::Air => None,
                whole => Some(Part::Whole(rubble(whole))),
            };
            if let Some(part) = part {
                cells.insert(cell, part);
                world.set(cell, Block::Air);
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        // (A crumb of a few cells is gone, not a thing that falls: what a cut or a landing
        // chips off.)
        if cells.len() < 4 {
            return changed;
        }
        // How it stands: its height, and how wide it is at its foot (what bears, lowest).
        let bearing: Vec<IVec3> = cells.iter().filter(|(_, p)| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }).map(|(c, _)| *c).collect();
        let of = if bearing.is_empty() { cells.keys().copied().collect() } else { bearing };
        let (low, high) = of.iter().fold((i32::MAX, i32::MIN), |(l, h), c| (l.min(c.y), h.max(c.y)));
        let foot: Vec<Vec3> = of.iter().filter(|c| c.y <= low + 1).map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        let middle = foot.iter().sum::<Vec3>() / foot.len() as f32;
        let wide = foot.iter().map(|p| (*p - middle).with_y(0.0).length()).fold(0.0, f32::max) * 2.0 + CELL;
        let tall = (high - low + 1) as f32 * CELL;
        let pivot = middle.with_y(low as f32 * CELL);
        // Which way it goes: the way it was pushed, and more the way it leans (where the
        // middle of its wood is from over its foot: a leaning tree goes the way it leans
        // whoever cuts it).
        let heart = of.iter().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / of.len() as f32;
        let lean = (heart - middle).with_y(0.0) / (tall * 0.08).max(0.1);
        let way = (push.with_y(0.0).normalize_or_zero() + lean.clamp_length_max(2.5)).try_normalize().unwrap_or(Vec3::X);
        // (Slowly at first: it hangs on its stump, held by what wood is left, and then goes.
        // Begun leaning three degrees and already turning, it was half way down in a second.)
        // (Hunks one on another are not a pole: they come straight down.)
        let hunk = cells.values().all(|p| matches!(p, Part::Whole(b) if b.hunk()));
        // (But seen to go from the blow that cut it: begun at one degree and all but still,
        // it stood for two seconds as if nothing had happened and then came down in one,
        // "the tree falls at a weird rate", the developer, 2026-11-14.)
        let over = (!hunk && tall > 2.0 && tall > wide * 2.5).then_some((way, 0.03, 0.10));
        // Where it is tried against the world: some hundreds of its cells, spread through it.
        // (Its wood and stone, not its leaves: a crown goes through another crown, and
        // into the ground a little, before the wood under it is stopped.)
        let stride = (of.len() / 300).max(1);
        let mut order: Vec<IVec3> = of.clone();
        order.sort_by_key(|c| (c.y, c.z, c.x));
        let feel: Vec<Vec3> = order.iter().step_by(stride).map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        // Stone and earth break; wood (and what is mostly wood) does not.
        let brittle = !hunk && of.iter().filter(|c| match &cells[*c] { Part::Whole(b) => !b.wood(), Part::Bits(b) => !b.iter().any(|b| b.wood()) }).count() * 2 > of.len();
        self.next += 1;
        self.begun.push((pivot, cells.len(), over.is_some()));
        // (A tree gone over is the thing whose trunk stood here.)
        let thing = over.and_then(|_| self.things.iter().find(|t| t.lies.is_none() && t.foot().is_some_and(|f| f.with_y(0.0).distance(pivot.with_y(0.0)) < 1.5)).map(|t| t.id));
        // (And a tree that lies with its boughs on, come loose again as it lies because what
        // it lay on is gone: still that tree, and drawn as it. Its cells, which nobody sees,
        // came down without it and it was left drawn in the air.)
        let veiled = cells.values().all(|p| match p { Part::Whole(b) => b.whole(), Part::Bits(bits) => bits.iter().all(|b| !b.solid() || b.whole()) });
        let thing = thing.or_else(|| self.things.iter().find(|t| veiled && cells.keys().any(|c| t.lain_trunk((c.as_vec3() + 0.5) * CELL))).map(|t| t.id));
        // A tree comes down onto its trunk and its heavy boughs, not onto its twigs: what of
        // it is thinner than a third of its trunk gives way under it, and is not felt for.
        // (Felt for like the rest, every felled tree lay propped on its own branches, its
        // trunk a metre and a half to two and a half up at its middle and its top three to
        // five: measured on eight islands after the developer saw one lying level in the
        // air over a bush, 2026-11-13. What gives way is crushed where it goes into the
        // ground, as `land` puts back only what finds room.)
        let feel: Vec<Vec3> = match thing.and_then(|id| self.things.iter().find(|t| t.id == id)) {
            Some(tree) => {
                let thick = tree.limbs.iter().map(|l| l.ra.max(l.rb)).fold(0.0, f32::max) * 0.6;
                let heavy: Vec<&crate::plants::Limb> = tree.limbs.iter().filter(|l| l.ra.max(l.rb) >= thick).collect();
                let kept: Vec<Vec3> = feel.iter().copied().filter(|p: &Vec3| heavy.iter().any(|l| from_line(*p, l.a, l.b) < l.ra.max(l.rb) + CELL)).collect();
                if kept.len() >= 8 { kept } else { feel }
            }
            None => feel,
        };
        if std::env::var_os("LOOSE_TRACE").is_some() && cells.len() >= 100 {
            let mut kinds: Vec<(Block, usize)> = Vec::new();
            for part in cells.values() {
                let b = match part { Part::Whole(b) => *b, Part::Bits(_) => Block::Broken };
                match kinds.iter_mut().find(|k| k.0 == b) {
                    Some(k) => k.1 += 1,
                    None => kinds.push((b, 1)),
                }
            }
            trace(format_args!("take: {} cells come loose as piece {}, of thing {thing:?}, going over {}, in {:.0} ms: {kinds:?}", cells.len(), self.next, over.is_some(), began.elapsed().as_secs_f32() * 1000.0));
        }
        self.pieces.push(Piece { id: self.next, mass: mass(&cells), cells, pivot, shift: Vec3::ZERO, turn: Quat::IDENTITY, vel: Vec3::ZERO, over, spin: Vec3::ZERO, brittle, hunk, thing, base: Quat::IDENTITY, settling: false, spare: (pivot, tall * 0.3), tall, feel, age: 0.0, rolls: 0, keeps: false, tips: None, landing: None });
        changed
    }

    /// Breaks a piece that has come down hard: what of it `keeps` says stays one piece (given
    /// back); the rest goes in lumps some three cells across, each thrown out from the
    /// middle and up (`throw` of how fast the whole came down), turning, to land on its own.
    /// What is too small to be a lump goes with the lump beside it; with none beside it, it
    /// is a small stone of its own.
    pub(super) fn shatter(&mut self, mut piece: Piece, speed: f32, throw: f32, keeps: impl Fn(IVec3) -> bool) -> Piece {
        let mut lumps: HashMap<IVec3, Cells<Part>> = HashMap::new();
        let all = std::mem::take(&mut piece.cells);
        for (cell, part) in all {
            if keeps(cell) {
                piece.cells.insert(cell, part);
                continue;
            }
            // (Lumps of uneven size: which lump a cell is of goes by a grid that is itself
            // shifted from place to place.)
            let shove = IVec3::new((cell.y / 2).rem_euclid(2), (cell.z / 3).rem_euclid(2), (cell.x / 2).rem_euclid(2));
            lumps.entry((cell + shove).div_euclid(IVec3::splat(3))).or_default().insert(cell, part);
        }
        // What is too small to be a lump goes with the largest lump beside it. (It was dust,
        // and gone: a boulder is mostly its outside, and a third to a half of one that came
        // down was no longer anywhere, 2026-11-13.) In a fixed order, so the same fall
        // breaks the same way.
        let mut small: Vec<IVec3> = lumps.iter().filter(|(_, c)| c.len() < 4).map(|(k, _)| *k).collect();
        small.sort_by_key(|k| (k.y, k.z, k.x));
        for key in small {
            let beside = round().map(|d| key + d).filter(|k| lumps.get(k).is_some_and(|c| c.len() >= 4)).max_by_key(|k| (lumps[k].len(), k.y, k.z, k.x));
            if let Some(to) = beside {
                let cells = lumps.remove(&key).unwrap_or_default();
                lumps.get_mut(&to).unwrap().extend(cells);
            }
        }
        let heart = piece.pivot + Vec3::Y * piece.tall / 2.0;
        let mut keys: Vec<IVec3> = lumps.keys().copied().collect();
        keys.sort_by_key(|k| (k.y, k.z, k.x));
        for key in keys {
            let cells = lumps.remove(&key).unwrap_or_default();
            // (A lump of leaf alone is not a thing that falls: it is leaves, shed.)
            // (And a scrap of stone with no lump beside it is a small stone, not nothing.)
            if !cells.values().any(|p| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }) {
                continue;
            }
            let middle = cells.keys().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / cells.len() as f32;
            // (Nothing random: how each is thrown goes by where it was.)
            let odd = |n: i32| ((key.x * 73 + key.y * 179 + key.z * 283 + n * 97).rem_euclid(200) as f32 / 100.0) - 1.0;
            let out = (middle - heart).with_y(0.0).try_normalize().unwrap_or(Vec3::new(odd(1), 0.0, odd(2)).normalize_or(Vec3::X));
            let feel = cells.iter().filter(|(_, p)| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }).map(|(c, _)| (c.as_vec3() + 0.5) * CELL).collect();
            self.next += 1;
            self.pieces.push(Piece {
                id: self.next,
                hunk: false,
                // (A lump of stone lies as it tumbled, on what it came down on, and is not
                // put into the grid: "rubble or something could also just be (physics-wise)
                // laying on top", the developer, 2026-11-13. Earth is not: it heaps.)
                tips: None, landing: None,
                keeps: cells.values().all(|p| match p { Part::Whole(b) => b.grown() == Block::Rock, Part::Bits(bits) => bits.iter().all(|b| *b == Block::Air || b.grown() == Block::Rock) }),
                mass: mass(&cells),
                cells,
                // (Turning about its own middle, from where the whole had brought it.)
                pivot: middle,
                shift: piece.pivot + piece.shift + piece.turn * (middle - piece.pivot) - middle,
                turn: piece.turn,
                vel: piece.turn * (out * (0.18 + 0.1 * odd(3)) + Vec3::Y * (0.22 + 0.08 * odd(4))) * speed.min(14.0) * throw,
                over: None,
                spin: Vec3::new(odd(5), odd(6), odd(7)) * 5.0 * throw.min(1.0),
                brittle: false,
                thing: None,
                base: Quat::IDENTITY,
                settling: false,
                spare: (middle, 0.0),
                tall: 3.0 * CELL,
                feel,
                age: 0.0,
                rolls: 0,
            });
        }
        piece
    }

    /// One moment of everything loose: each piece falls or goes further over, and what
    /// meets the world is put back into it. Gives the chunks that changed.
    pub fn step(&mut self, world: &mut World, dt: f32) -> HashSet<IVec3> {
        self.steps += 1;
        let (began, falling) = (std::time::Instant::now(), self.pieces.len());
        let mut changed = HashSet::new();
        let mut down = Vec::new();
        // (Every tree that has been cut into and still stands, for what falls to go through: below.)
        let standing: Vec<(u32, Vec<(Vec3, Vec3, f32, f32)>)> = if self.pieces.is_empty() { Vec::new() } else { self.things.iter().filter(|t| t.lies.is_none() && !t.limbs.is_empty()).map(|t| (t.id, t.limbs.iter().map(|l| (l.a, l.b, l.ra, l.rb)).collect())).collect() };
        // Where each piece that drops is, by the cell, with how fast it drops and how high its
        // middle is: for one that comes down on another in the air (below). Only where there
        // are two.
        let middle = |p: &Piece| p.now(p.feel.iter().sum::<Vec3>() / p.feel.len().max(1) as f32).y;
        let air: HashMap<IVec3, (u32, f32, f32)> = if self.pieces.len() < 2 {
            HashMap::new()
        } else {
            self.pieces.iter().filter(|p| p.over.is_none() && p.tips.is_none()).flat_map(|p| { let at = (p.id, p.vel.y, middle(p)); p.feel.iter().map(move |f| ((p.now(*f) / CELL).floor().as_ivec3(), at)) }).collect()
        };
        // Great pieces part way into the world go on being put, and are not moved.
        let mut putting = Vec::new();
        for piece in std::mem::take(&mut self.pieces) {
            match piece.landing {
                Some(_) => putting.extend(self.put_down(world, piece, 0.0, &mut changed)),
                None => self.pieces.push(piece),
            }
        }
        for (n, piece) in self.pieces.iter_mut().enumerate() {
            piece.age += dt;
            let before = (piece.shift, piece.turn);
            let speed;
            match &mut piece.over {
                Some((way, tipped, rate)) if piece.settling => {
                    // Its top is down: it turns back about it, its butt coming down off the
                    // stump, and slides on a little the way it fell.
                    // (First on, a good stride and more, and only then down: a trunk's foot
                    // spreads into roots a metre out, and let down at once the butt came to
                    // rest on them, still in the air.)
                    if piece.age < 0.4 {
                        piece.shift += *way * 3.5 * dt;
                    } else {
                        *rate -= 5.0 * dt;
                        *tipped += *rate * dt;
                    }
                    piece.turn = Quat::from_axis_angle(Vec3::Y.cross(*way).normalize_or(Vec3::Z), *tipped) * piece.base;
                    speed = rate.abs() * piece.tall * 0.5;
                    if *tipped < -1.2 {
                        piece.age = 99.0;
                    }
                }
                Some((way, tipped, rate)) => {
                    // As a pole on its end goes over: faster the further it has gone.
                    // (As a pole on its end goes over, at the earth's own gravity, and no
                    // faster at its top than the air lets a crown go: `matter`.)
                    *rate = (*rate + 1.5 * crate::matter::TIPPING / piece.tall.max(1.0) * tipped.sin() * dt).min(crate::matter::SWEEP / piece.tall.max(1.0));
                    *tipped += *rate * dt;
                    piece.turn = Quat::from_axis_angle(Vec3::Y.cross(*way).normalize_or(Vec3::Z), *tipped);
                    speed = *rate * piece.tall;
                }
                // Going over about where it touched, the way its weight leans from there,
                // faster as it goes.
                None if piece.tips.is_some() => {
                    let (on, rate, turned, times, lean) = piece.tips.unwrap_or_default();
                    let about = piece.now(on);
                    let rate = (rate + 14.0 * dt).min(7.0);
                    let by = Quat::from_axis_angle(Vec3::Y.cross(lean).normalize_or(Vec3::Z), rate * dt);
                    (piece.shift, piece.turn) = (about + by * (piece.pivot + piece.shift - about) - piece.pivot, (by * piece.turn).normalize());
                    piece.tips = Some((on, rate, turned + rate * dt, times, lean));
                    speed = rate * piece.tall.max(0.5) * 0.5;
                    // (Right over and nothing met: it lies as it is.)
                    if turned > 1.8 {
                        piece.age = 99.0;
                    }
                }
                None => {
                    piece.vel.y -= FALL * dt;
                    piece.shift += piece.vel * dt;
                    piece.turn = (Quat::from_scaled_axis(piece.spin * dt) * piece.turn).normalize();
                    // What comes down on another piece in the air does not go through it: it
                    // drops no faster than the one under it, and is on it when that one lands.
                    // (Pieces met only the world, and each other once one had come to rest: a
                    // hunk let go over a falling log went through it and lay under it. The
                    // developer, of what the physics must do: "how everything collides with
                    // each other".)
                    if !air.is_empty() {
                        let mine = middle(piece);
                        let under = piece.feel.iter().filter_map(|f| air.get(&(piece.now(*f) / CELL).floor().as_ivec3())).filter(|(id, drops, at)| *id != piece.id && *at < mine - BIT && piece.vel.y < *drops).map(|o| o.1).fold(f32::MIN, f32::max);
                        if under > f32::MIN {
                            piece.shift.y += (under - piece.vel.y) * dt;
                            piece.vel.y = under;
                        }
                    }
                    speed = piece.vel.length();
                }
            }
            // What of it is tried: all of it as it drops; as it goes over, what is well
            // above its foot (its foot stands on the stump it was cut from).
            let going_over = piece.over.is_some();
            // (Stopped by the ground and by what has itself been cut or has fallen; not by
            // leaf, and not by a tree still standing whole: in a wood every tree's limbs are
            // in its neighbours', and stopped by those nothing cut ever came down. It goes
            // through them, and where it lies across a standing trunk it is not put.)
            // (Nor by another tree that stands though it has been cut into, which is cells
            // and would bear: a birch felled beside one lay 14 m up in its crown.)
            let (mine, lone) = (piece.thing, piece.hunk);
            let stops = |at: Vec3| {
                let b = world.bit((at / BIT).floor().as_ivec3());
                // (A boulder as it grew stops what falls on it, as one cut into does: until
                // it was cut a tree came down through it.)
                // (And a hunk by any wood: it is no tree, to go through a tree, and one let go
                // over a standing trunk went into it and was lost.)
                if lone && b.wood() {
                    return true;
                }
                (bears(b) || b == Block::Rock) && !(b.wood() && standing.iter().any(|(id, limbs)| Some(*id) != mine && limbs.iter().any(|l| outside(at, l) < 0.3)))
            };
            let tried = |piece: &Piece, p: &Vec3| !(going_over || piece.tips.is_some()) || p.distance(piece.spare.0) > piece.spare.1;
            let meets = |piece: &Piece| piece.feel.iter().filter(|p| tried(piece, p)).any(|p| stops(piece.now(*p)));
            let met = meets(piece);
            // `LOOSE_TRACE=1`: what stopped it, each time it is stopped: where on it (as it
            // stood, from its foot) and by what in the world.
            if met && std::env::var_os("LOOSE_TRACE").is_some() {
                let hits: Vec<String> = piece.feel.iter().filter(|p| tried(piece, p)).filter(|p| stops(piece.now(**p))).take(4).map(|p| { let at = piece.now(*p); format!("{:.1} m up it, {:.1} m out, at {:.1} by {:?}", p.y - piece.pivot.y, (*p - piece.pivot).with_y(0.0).length(), at, world.bit((at / BIT).floor().as_ivec3())) }).collect();
                eprintln!("loose: piece {} of {} cells stopped (going over {going_over}, settling {}): {}", piece.id, piece.cells.len(), piece.settling, hits.join(" · "));
            }
            // (Where it struck, for a tree whose top has just come down: what it will settle about.)
            let struck = (met && going_over && !piece.settling).then(|| piece.feel.iter().filter(|p| tried(piece, p)).find(|p| stops(piece.now(**p))).copied()).flatten();
            if met {
                // Back to where it last was clear, as near to touching as six halvings find:
                // put down where it met the world it would be a step's depth inside it, and
                // what was inside would be lost (half a slab, dropped two metres).
                let after = (piece.shift, piece.turn);
                let (mut clear, mut inside) = (0.0f32, 1.0f32);
                for _ in 0..6 {
                    let half = (clear + inside) / 2.0;
                    (piece.shift, piece.turn) = (before.0.lerp(after.0, half), before.1.slerp(after.1, half));
                    if meets(piece) { inside = half } else { clear = half }
                }
                (piece.shift, piece.turn) = (before.0.lerp(after.0, clear), before.1.slerp(after.1, clear));
            }
            if met || piece.age > 20.0 {
                down.push((n, speed, struck));
            }
        }
        let moved = began.elapsed();
        let mut settling = Vec::new();
        let mut settling_rolls: Vec<Piece> = Vec::new();
        for (n, speed, struck) in down.into_iter().rev() {
            let piece = self.pieces.swap_remove(n);
            let mut piece = piece;
            // A lump that has come down on a heap rolls off it: where the ground beside it is
            // lower than where it lies by more than its own height, it goes that way and
            // falls again, three times at the most. (Left where they met, the lumps of one
            // broken stone stood on one another two metres high, 2026-11-13.) In a fixed
            // order of sides, so the same fall heaps the same way.
            // (Not wood: a thin log rolled from where its tree lay, and the axe, which asks
            // the tree where its logs are, could not find it to split. Seed 2, 2026-11-13.)
            let wooden = || piece.cells.values().any(|p| matches!(p, Part::Whole(b) if b.wood()));
            if piece.thing.is_none() && piece.over.is_none() && !piece.settling && !piece.hunk && piece.cells.len() < 80 && piece.rolls < 3 && !wooden() {
                let (middle, reach) = (piece.now(piece.pivot), piece.tall.max(2.0 * CELL));
                let below = |at: Vec3| world.raycast(at, Vec3::NEG_Y, 12.0);
                let sides = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].map(|d| middle + d * reach);
                let lower = sides.into_iter().filter(|s| !bears(world.bit((*s / BIT).floor().as_ivec3()))).filter_map(|s| below(s).map(|d| (d, s))).filter(|(d, _)| *d > reach * 1.5).max_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, side)) = lower {
                    piece.shift += side - middle;
                    (piece.vel, piece.spin, piece.rolls) = (Vec3::ZERO, piece.spin * 0.5, piece.rolls + 1);
                    settling_rolls.push(piece);
                    continue;
                }
            }
            // A tree's top has struck: it is heard, its outermost twigs snap off, and it
            // settles about where it struck (below, once every piece has been looked at).
            if let (Some(top), Some((way, ..))) = (struck, piece.over) {
                // (All along it, from its foot to where its top struck, and a metre to either
                // side: whoever is under any of its trunk is under it.)
                self.landed.push(Landed { at: piece.now(top), cells: piece.cells.len(), put: 0, lost: 0, speed, settled: false, mass: piece.mass, wood: woody(&piece.cells), along: Some((piece.now(piece.pivot), 1.0)) });
                // (It comes down whole: its outermost twigs broke from it here, each a lump
                // of its own, until a felled tree was to keep the shape it stood in.)
                (piece.shift, piece.pivot) = (piece.pivot + piece.shift + piece.turn * (top - piece.pivot) - top, top);
                (piece.base, piece.over, piece.settling, piece.spare, piece.age) = (piece.turn, Some((way, 0.0, 0.0)), true, (top, piece.tall * 0.3), 0.0);
                settling.push(piece);
                continue;
            }
            if piece.brittle && speed > SHATTERS && piece.cells.len() >= 8 {
                self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put: 0, lost: 0, speed, settled: false, mass: piece.mass, wood: woody(&piece.cells), along: None });
                self.shatter(piece, speed, 1.0, |_| false);
                continue;
            }
            // A lump or a thing dropped straight comes to rest square to the world: turned to
            // the nearest quarter turn each way and moved to the nearest whole cell (up, not
            // down into what it lies on). The world is cells, and what lies turned between
            // them is asked cell by cell what of it is there: a boulder's broken outside,
            // a bit thick, was mostly lost that way, a quarter of its stone (2026-11-13).
            // A tree lies as it fell: it is long, and squared it would jump.
            // What keeps its shape does not stop where it first touches: it goes over about
            // there until its weight is over what it rests on (a touch to either side of its
            // middle, or three with one right under it), three times at the most. (Stopped at its first
            // touch, a lump of rubble stood on a corner.)
            if piece.keeps && piece.age < 20.0 {
                let weight = piece.now(piece.feel.iter().sum::<Vec3>() / piece.feel.len().max(1) as f32);
                let touches: Vec<Vec3> = piece.feel.iter().copied().filter(|p| { let at = piece.now(*p) - Vec3::Y * 0.08; let b = world.bit((at / BIT).floor().as_ivec3()); bears(b) || b == Block::Rock }).collect();
                let off = |p: &Vec3| (piece.now(*p) - weight).with_y(0.0);
                // Held: whichever way one looks from its middle, something it touches is at
                // least a hand that way, or none of it reaches further that way than what
                // it touches (a thin log lies on a line). The way it is least held is the
                // way it goes, about what it touches furthest that way. (A touch to each
                // side of the middle was the test: a lump lay on one edge, at 45 degrees.)
                let ways = (0..16).map(|i| Vec3::new((i as f32 * std::f32::consts::TAU / 16.0).cos(), 0.0, (i as f32 * std::f32::consts::TAU / 16.0).sin()));
                let reach = |n: Vec3, of: &mut dyn Iterator<Item = &Vec3>| of.map(|p| off(p).dot(n)).fold(f32::MIN, f32::max);
                let least = ways.map(|n| (n, reach(n, &mut touches.iter()), reach(n, &mut piece.feel.iter()))).filter(|(_, held, all)| *held < 0.06 && *all > held + 0.1).min_by(|a, b| a.1.total_cmp(&b.1));
                let times = piece.tips.map_or(0, |t| t.3);
                if let Some((lean, _, _)) = least.filter(|_| times < 3 && !touches.is_empty()) {
                    let on = touches.iter().copied().max_by(|a, b| off(a).dot(lean).total_cmp(&off(b).dot(lean))).unwrap_or(piece.pivot);
                    (piece.tips, piece.spare, piece.vel, piece.spin) = (Some((on, 1.0, 0.0, times + 1, lean)), (on, 0.3), Vec3::ZERO, Vec3::ZERO);
                    settling.push(piece);
                    continue;
                }
            }
            // What keeps its shape lies as it came down, and is not put into the world.
            if piece.keeps {
                // Out of what it came down into, first: pieces do not meet each other in
                // the air and a touch is felt at a few places of it, so it stopped a little
                // in the ground or in the log beside it (a twentieth of a log, more of a
                // lump in a heap: `cargo world lying`).
                let mut lifted = 0;
                for _ in 0..10 {
                    if sunk(world, &piece) <= SUNK {
                        break;
                    }
                    piece.shift.y += BIT;
                    lifted += 1;
                }
                let placed = rest(world, &piece);
                trace(format_args!("rest: piece {} of {} cells put up {} cm, in {} cells of the world", piece.id, piece.cells.len(), lifted * 5, placed.len()));
                let (lo, hi) = placed.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
                for cell in &placed {
                    self.owner.insert(*cell, piece.id);
                    changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                }
                self.landed.push(Landed { at: piece.now(piece.pivot), cells: piece.cells.len(), put: placed.len(), lost: 0, speed, settled: false, mass: piece.mass, wood: woody(&piece.cells), along: None });
                self.rested.push(Rested { id: piece.id, cells: piece.cells, pivot: piece.pivot, shift: piece.shift, turn: piece.turn, placed });
                if lo.x <= hi.x {
                    changed.extend(self.loosen(world, lo - 1, hi + 1, Vec3::X));
                }
                continue;
            }
            if piece.thing.is_none() && piece.over.is_none() && !piece.settling {
                square(&mut piece);
            }
            // (A hunk comes down whole or not at all: up, a cell at a time, until every cell
            // of it has room. Put where it met the ground a quarter of a cell in, one lost
            // two of its eight cells and was no hunk: seed 5, 2026-11-13.)
            if piece.hunk {
                for _ in 0..8 {
                    let by = (piece.shift / CELL).round().as_ivec3();
                    // (Leaf is room: it is crushed under what comes down on it, and a hunk
                    // set down on top of it lay on nothing that bears and fell again.)
                    if piece.cells.keys().all(|c| room(world, *c + by)) {
                        // (And what leaf is there is crushed under it.)
                        for c in piece.cells.keys() {
                            if world.get(*c + by) == Block::Broken {
                                world.set(*c + by, Block::Air);
                            }
                        }
                        break;
                    }
                    piece.shift.y += CELL;
                }
            }
            if let Some(piece) = self.put_down(world, piece, speed, &mut changed) {
                settling.push(piece);
            }
        }
        self.pieces.extend(settling);
        self.pieces.extend(settling_rolls);
        self.pieces.extend(putting);
        // `LOOSE_TRACE=1`: a step that took long, and where it went.
        if began.elapsed().as_millis() >= 8 {
            trace(format_args!("step: {:.0} ms, {falling} pieces: {:.0} ms moving and trying them, {:.0} ms landing what came down and looking at what it let go", began.elapsed().as_secs_f32() * 1000.0, moved.as_secs_f32() * 1000.0, (began.elapsed() - moved).as_secs_f32() * 1000.0));
        }
        changed
    }

    /// Puts a piece that has come down into the world, where it is: all of it, or of a great
    /// one ([`SPREAD`]) the next rows of it, in which case it is given back to be gone on
    /// with at the next step. When all of it is in, what it lies as is noted, what it let go
    /// is looked for, and its landing is told.
    fn put_down(&mut self, world: &mut World, mut piece: Piece, speed: f32, changed: &mut HashSet<IVec3>) -> Option<Piece> {
        // What it lies as is noted when it comes down, not when the last of it is in: it is
        // drawn there from then.
        if piece.landing.is_none()
            && let Some(thing) = self.things.iter_mut().find(|t| Some(t.id) == piece.thing)
        {
            // (Come down a second time, from where it lay: the one move after the other.)
            thing.lies = Some(match thing.lies {
                Some((pivot, shift, turn)) => (pivot, piece.pivot + piece.shift + piece.turn * (pivot + shift - piece.pivot) - pivot, piece.turn * turn),
                None => (piece.pivot, piece.shift, piece.turn),
            });
        }
        let (put, lost, lo, hi, speed) = if piece.cells.len() > SPREAD {
            let (row, had, gone, speed) = piece.landing.unwrap_or((i32::MIN, 0, 0, speed));
            // (Its box, which `land` gives whatever rows it is asked for; then the next rows.)
            let (_, _, lo, hi, _) = land(world, &piece, Some((0, 0)));
            let from = row.max(lo.y);
            let many = (ROWS / ((hi.x - lo.x + 1) * (hi.z - lo.z + 1)).max(1)).max(1);
            let (put, touched, _, _, lost) = land(world, &piece, Some((from, many)));
            changed.extend(touched);
            if from + many <= hi.y {
                piece.landing = Some((from + many, had + put, gone + lost, speed));
                return Some(piece);
            }
            (had + put, gone + lost, lo, hi, speed)
        } else {
            let (put, touched, lo, hi, lost) = land(world, &piece, None);
            changed.extend(touched);
            (put, lost, lo, hi, speed)
        };
        // And what of it has come down on nothing (a limb left sticking out over a
        // hollow, a part cut off from the rest by what it fell across) falls in its turn.
        // (Not of a tree that lies as itself: it is one body, drawn whole, and no part of it
        // goes without the rest; and what it came down on has lost nothing by it. Looking
        // through a great one's box was 20 to 35 ms for nothing.)
        if piece.thing.is_none() {
            changed.extend(self.loosen(world, lo - 1, hi + 1, Vec3::X));
        }
        self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put, lost, speed, settled: piece.settling, mass: piece.mass, wood: woody(&piece.cells), along: None });
        None
    }
}
