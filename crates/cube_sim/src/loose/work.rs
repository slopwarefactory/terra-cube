//! What a tool does: a spade's cut, an axe's staged work on a tree (fell, limb, buck, split),
//! a pick's on a boulder and on rubble, and a thing put down.

use super::*;

/// What part of a trunk's wood must be left across a cut for it to go on standing: half.
/// (Chops are balls: three in a line through a trunk a metre across leave two fifths of it,
/// in two strips, and it must not stand on those.)
pub(super) const HOLDS: f32 = 0.5;

/// Wood cut most of the way through gives way: if fewer than [`HOLDS`] of the trunk's fibres
/// run unbroken through the cut, what is left at the level with least wood goes too. (A hole chopped straight through a trunk left a sliver each side of
/// it, and the tree stood on them with daylight through it; every chop after went through
/// the hole.) Says how many bits went.
pub(super) fn snap(world: &mut World, middle: Vec3, radius: f32) -> u32 {
    let reach = radius + 0.8;
    let (lo, hi) = (((middle - Vec3::new(reach, radius, reach)) / BIT).floor().as_ivec3(), ((middle + Vec3::new(reach, radius, reach)) / BIT).floor().as_ivec3());
    let level = |world: &World, y: i32| -> Vec<IVec3> { (lo.z..=hi.z).flat_map(|z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z))).filter(|b| world.bit(*b).wood()).collect() };
    // What holds is wood that runs unbroken from over the cut to under it: of the trunk's
    // fibres just above the cut, those with wood at every level down through it. (Judged
    // by the least wood at any one level, a tunnel chopped aslant through a trunk never
    // felled it: no level had lost half, and nine fibres in ten were cut.)
    let over = level(world, hi.y + 2);
    let whole = over.len();
    let unbroken = over.iter().filter(|b| (lo.y..=hi.y).all(|y| world.bit(b.with_y(y)).wood())).count();
    let Some(thin) = (lo.y..=hi.y).map(|y| level(world, y)).min_by_key(|l| l.len()) else { return 0 };
    if whole == 0 || thin.is_empty() || unbroken as f32 >= whole as f32 * HOLDS {
        return 0;
    }
    // (All of the cell that level is in, a hand's breadth of wood: what is joined to what is
    // told by the cell, and a slit one bit high through a cell parts nothing.)
    let floor = thin[0].y.div_euclid(SPLIT) * SPLIT;
    let mut gone = 0;
    for y in floor..floor + SPLIT {
        for bit in level(world, y) {
            world.set_bit(bit, Block::Air);
            gone += 1;
        }
    }
    gone
}

/// Takes out of the world what of a ball is `stuff` (or all of it), to the bit. Says how many
/// bits went, of earth, wood and stone.
pub(super) fn carve(world: &mut World, middle: Vec3, radius: f32, stuff: Option<Stuff>) -> [u32; 3] {
    let mut taken = [0; 3];
    let goes = |b: Block| b != Block::Air && b != Block::Water && stuff.is_none_or(|s| Stuff::of(b) == Some(s));
    let tally = |taken: &mut [u32; 3], b: Block, bits: u32| {
        if let Some(of) = Stuff::of(b).filter(|_| !b.foliage()) {
            taken[of as usize] += bits;
        }
    };
    let inside = |p: Vec3| p.distance_squared(middle) < radius * radius;
    let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3(), ((middle + radius) / CELL).floor().as_ivec3());
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                let was = world.get(cell);
                if was == Block::Air {
                    continue;
                }
                let all = (0..8).all(|i| inside((cell + IVec3::new(i & 1, i >> 1 & 1, i >> 2 & 1)).as_vec3() * CELL));
                if all && was != Block::Broken {
                    if goes(was) {
                        world.set(cell, Block::Air);
                        tally(&mut taken, was, (SPLIT * SPLIT * SPLIT) as u32);
                    }
                    continue;
                }
                for i in 0..SPLIT * SPLIT * SPLIT {
                    let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                    if inside((bit.as_vec3() + 0.5) * BIT) {
                        let b = world.bit(bit);
                        if goes(b) {
                            world.set_bit(bit, Block::Air);
                            tally(&mut taken, b, 1);
                        }
                    }
                }
            }
        }
    }
    taken
}

/// Makes wrought every tree and boulder of `forms` that a ball reaches: all of each tree, its
/// every limb and its leaf, as `plants` grew it. Says which, and the chunks changed.
/// `whole`: a tree is made cells that are still drawn as the tree ([`Block::felled`]), for an
/// axe; else, cells drawn as cells.
pub(super) fn work(world: &mut World, middle: Vec3, radius: f32, forms: &[&Form], stuff: Option<Stuff>, known: &[Vec3], whole: bool) -> Cut {
    let mut cut = Cut::default();
    // (Each cell made wrought as it is come to, not gathered first: making the same cell
    // wrought twice is nothing, and a great oak is a quarter of a million cells, which took
    // 50 ms to gather into a set and then change.)
    let turn = |world: &mut World, cut: &mut Cut, cell: IVec3| {
        let was = world.get(cell);
        if was == Block::Broken || was.shaped() {
            // (A boulder's stone is drawn as cells either way.)
            world.map(cell, if whole { |b: Block| if b == Block::Rock { b.wrought() } else { b.felled() } } else { Block::wrought });
            cut.worked += 1;
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
    };
    let (trees, stones) = (stuff.is_none_or(|s| s == Stuff::Wood), stuff.is_none_or(|s| s == Stuff::Stone));
    for (n, form) in forms.iter().enumerate() {
        // (A rooted trunk swells to twice its thickness and more at its foot.)
        let reach = |l: &crate::plants::Limb| radius + l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL;
        let mut trees: Vec<u32> = form.limbs.iter().filter(|l| trees && from_line(middle, l.a, l.b) < reach(l)).map(|l| l.tree).collect();
        trees.sort_unstable();
        trees.dedup();
        trees.retain(|t| form.limbs.iter().find(|l| l.tree == *t).is_none_or(|l| !known.contains(&l.a)));
        for tree in trees {
            // (Low down a tree's wood is written to the bit, in the cubes it is drawn in
            // near the eye (`plants::wood`), and those reach cells that its shape at the
            // size of a cell does not: a root's flare, the edge of a leaning trunk. Left as
            // they grew they were wood nobody draws and nothing rests on: a birch stood with
            // a band of air in its trunk, and its top was held by nothing, 2026-11-13. So
            // near its foot the cells beside each cell are looked at too.)
            let low = form.limbs.iter().filter(|l| l.tree == tree && l.roots > 0).map(|l| l.a.y).fold(f32::MAX, f32::min) + 4.0;
            for limb in form.limbs.iter().filter(|l| l.tree == tree) {
                limb.fill(CELL, |c| {
                    turn(world, &mut cut, c);
                    if (c.y as f32) * CELL < low {
                        // (Only what has this limb's own wood in it and is no further from
                        // it than its foot swells: every broken cell beside it, a sapling's
                        // and a bush's among them, was 13,000 cells more on one tree, and
                        // the tree then came down on the sapling.)
                        let swell = limb.ra.max(limb.rb) * if limb.roots > 0 { 2.6 } else { 1.0 } + CELL;
                        for side in round() {
                            let next = c + side;
                            if world.get(next) == Block::Broken && from_line((next.as_vec3() + 0.5) * CELL, limb.a, limb.b) < swell && world.bits_at(next).is_some_and(|bits| bits.contains(&limb.bark)) {
                                turn(world, &mut cut, next);
                            }
                        }
                    }
                });
            }
            for clump in form.clumps.iter().filter(|c| c.tree == tree) {
                let (lo, hi) = (((clump.middle - clump.radii) / CELL).floor().as_ivec3(), ((clump.middle + clump.radii) / CELL).floor().as_ivec3());
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        for x in lo.x..=hi.x {
                            let c = IVec3::new(x, y, z);
                            if world.get(c).foliage() {
                                turn(world, &mut cut, c);
                            }
                        }
                    }
                }
            }
            cut.trees.push((n, tree));
        }
        for (k, stone) in form.stones.iter().enumerate().filter(|(_, s)| stones && ((middle - s.middle) / (s.radii + radius + CELL)).length_squared() < 1.0) {
            let reach = Vec3::splat(stone.radii.max_element());
            let (lo, hi) = (((stone.middle - reach) / CELL).floor().as_ivec3(), ((stone.middle + reach) / CELL).floor().as_ivec3());
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        turn(world, &mut cut, IVec3::new(x, y, z));
                    }
                }
            }
            cut.stones.push((n, k));
        }
    }
    cut
}

impl Loose {
    /// Notes each tree and boulder a cut has made cells of, once, as a [`Thing`].
    pub(super) fn note(&mut self, cut: &Cut, forms: &[&Form], cells: bool) {
        for (n, tree) in &cut.trees {
            let limbs: Vec<crate::plants::Limb> = forms[*n].limbs.iter().filter(|l| l.tree == *tree).copied().collect();
            // (Once: whoever keeps the shapes is to drop it from them, and may not have yet.)
            if self.things.iter().any(|t| t.limbs.first().map(|l| l.a) == limbs.first().map(|l| l.a)) {
                continue;
            }
            self.next += 1;
            self.things.push(Thing { id: self.next, limbs, clumps: forms[*n].clumps.iter().filter(|c| c.tree == *tree).copied().collect(), stone: None, cuts: Vec::new(), lies: None, blows: 0, limbed: false, felled: false, cells, notch: None, stump: false });
        }
        for (n, stone) in &cut.stones {
            if self.things.iter().any(|t| t.stone.as_ref().map(|s| s.middle) == forms[*n].stones.get(*stone).map(|s| s.middle)) {
                continue;
            }
            self.next += 1;
            self.things.push(Thing { id: self.next, limbs: Vec::new(), clumps: Vec::new(), stone: forms[*n].stones.get(*stone).cloned(), cuts: Vec::new(), lies: None, blows: 0, limbed: false, felled: false, cells, notch: None, stump: false });
        }
    }

    /// Digs a ball out of the world and looses what that leaves unsupported, pushed the way
    /// `push` points (the way whoever cut it was facing). Gives the chunks that changed.
    ///
    /// `forms` are the shapes of what grows round the cut, as `plants` noted them: a tree or
    /// a boulder among them that the ball reaches is made wrought first.
    ///
    /// Only `stuff` is cut, where one is given (what a tool cuts: an axe takes the wood out
    /// of the ball and leaves the earth); with none, everything in the ball goes.
    pub fn cut(&mut self, world: &mut World, middle: Vec3, radius: f32, push: Vec3, forms: &[&Form], stuff: Option<Stuff>) -> Cut {
        self.journal.push((self.steps, Edit::Cut { middle, radius, push, stuff }));
        let (noted, falling) = (self.journal.len() - 1, self.pieces.len());
        let began = std::time::Instant::now();
        let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3() - 1, ((middle + radius) / CELL).floor().as_ivec3() + 1);
        // (A tree is made wrought only by what cuts wood, a boulder by what cuts stone.)
        // (A tree already cut into is cells, whether or not whoever keeps the shapes has
        // dropped it from them yet.)
        let known: Vec<Vec3> = self.things.iter().filter_map(|t| t.limbs.first().map(|l| l.a)).collect();
        let mut cut = work(world, middle, radius, forms, stuff, &known, false);
        // Each thing made wrought is noted, and every bite out of a thing noted on it.
        self.note(&cut, forms, true);
        for thing in self.things.iter_mut().filter(|t| t.lies.is_none() && t.reached(middle, radius)) {
            thing.cuts.push((middle, radius, stuff));
        }
        cut.taken = carve(world, middle, radius, stuff);
        // A trunk cut most of the way through snaps.
        if cut.taken[Stuff::Wood as usize] > 0 {
            cut.taken[Stuff::Wood as usize] += snap(world, middle, radius);
        }
        if cut.taken.iter().any(|t| *t > 0) {
            self.cuts.push((middle, cut.taken));
        }
        let mut changed = std::mem::take(&mut cut.changed);
        // Stone dug from the land with a pick comes away as a hunk: a bite that took half
        // a hunk's worth or more leaves one, on the side the blow came from, to be carried
        // off. (By the bite alone, with nothing kept from one to the next: a region's
        // edits are done again on their own, `Replay`.) Not from a boulder, which is
        // chipped ([`Loose::chip`]).
        let boulder = self.things.iter().any(|t| t.stone.is_some() && t.reached(middle, radius));
        if stuff == Some(Stuff::Stone) && !boulder && cut.taken[Stuff::Stone as usize] * 2 >= (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as u32 {
            let back = middle - push.with_y(0.0).normalize_or_zero() * (radius + CELL);
            if let Some(corner) = set_down(world, back, Block::Cobble, &mut changed) {
                changed.extend(self.loosen(world, corner - 1, corner + HUNK, push));
            }
        }
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                changed.insert(IVec3::new(x, lo.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                changed.insert(IVec3::new(x, hi.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        changed.extend(self.loosen(world, lo, hi, push));
        changed.extend(flow(world, lo, hi));
        // `LOOSE_TRACE=1`: each cut in a line (where, what it took, what is loose after it).
        if std::env::var_os("LOOSE_TRACE").is_some() {
            let around = |y: f32| { let c = ((middle.with_y(y)) / CELL).floor().as_ivec3(); (-3..=3).flat_map(|z| (-3..=3).map(move |x| c + IVec3::new(x, 0, z))).map(|c| format!("{:?}", world.get(c)).chars().next().unwrap_or('?')).collect::<String>() };
            eprintln!("loose: cut at {middle:.2?} r {radius:.2} of {stuff:?} · took {:?} · made wrought {} in {:.1} ms · {} loose · cells at the cut {} · over it {}", cut.taken, cut.worked, began.elapsed().as_secs_f32() * 1000.0, self.pieces.len(), around(middle.y), around(middle.y + radius + 0.3));
        }
        let _ = began;
        // A cut that took nothing, made nothing its cells and loosed nothing is not an edit:
        // the world is its seed and its journal, and every swing at the air was kept in it
        // (sixty blows of a pick at a stone already broken were sixty edits, 2026-11-13).
        if cut.taken == [0, 0, 0] && cut.worked == 0 && self.pieces.len() == falling {
            self.journal.remove(noted);
        }
        Cut { changed, ..cut }
    }

    /// A blow of an axe on a tree. An axe does not dig wood as a shovel digs earth: it does
    /// to the tree it strikes what is next for that tree (the developer, 2026-11-13).
    ///
    /// 1. Standing, the blow goes into one notch a metre up its trunk, on the side it came
    ///    from, and the tree stands until it has taken as many as its girth asks ([`BLOWS`]):
    ///    the last cuts it through, and it falls.
    /// 2. Fallen, it is still a tree: a blow knocks off all its boughs and leaf, which are
    ///    noted in `shed` for whoever shows them.
    /// 3. Bare, it is timber ([`Loose::timber`]): a blow breaks what it strikes through at
    ///    the nearest of the places it breaks, one to each [`LOG`] of its length.
    /// 4. And a blow at a log splits it into hunks ([`HUNK`]): pieces of wood all of one
    ///    size, to be carried and built with.
    ///
    /// Says what changed as a cut does; a tree it made cells of is in `trees`.
    pub fn chop(&mut self, world: &mut World, at: Vec3, push: Vec3, forms: &[&Form]) -> Cut {
        self.journal.push((self.steps, Edit::Chop { at, push }));
        // (A blow that did nothing is not an edit, as a cut that did nothing is not: by
        // whether anything it could do was done.)
        let state = |l: &Loose| (l.things.len(), l.things.iter().map(|t| t.blows + t.limbed as u32 + t.felled as u32 + t.cells as u32).sum::<u32>(), l.rested.len(), l.pieces.len(), l.cuts.len(), l.shed.len());
        let (noted, before) = (self.journal.len() - 1, state(self));
        let cut = self.chopped(world, at, push, forms);
        if cut.changed.is_empty() && state(self) == before {
            self.journal.remove(noted);
            trace(format_args!("chop: a blow at {at:.1?} did nothing: {} trees known, {} lying, {} limbed, {} at rest", self.things.iter().filter(|t| !t.limbs.is_empty()).count(), self.things.iter().filter(|t| t.lies.is_some()).count(), self.things.iter().filter(|t| t.limbed).count(), self.rested.len()));
        }
        cut
    }

    fn chopped(&mut self, world: &mut World, at: Vec3, push: Vec3, forms: &[&Form]) -> Cut {
        // The tree struck: one already cut into, as it stands or as it lies; or one still as
        // it grew, which is made cells and noted first.
        let reached = |t: &Thing| {
            let p = t.lies.map_or(at, |(pivot, shift, turn)| pivot + turn.inverse() * (at - pivot - shift));
            t.limbs.iter().map(|l| from_line(p, l.a, l.b) - l.ra.max(l.rb)).fold(f32::MAX, f32::min)
        };
        // (Not a stump or a log lying where it grew, which an axe does nothing to: one
        // beside a great tree was nearer the blow than the tree's own trunk, took every
        // blow, and the tree stood. Something with three metres of height to it.)
        let tree = |t: &Thing| t.lies.is_some() || t.limbs.iter().map(|l| l.a.y.max(l.b.y)).fold(f32::MIN, f32::max) - t.limbs.iter().map(|l| l.a.y.min(l.b.y)).fold(f32::MAX, f32::min) >= 3.0;
        let near = |things: &[Thing]| things.iter().enumerate().filter(|(_, t)| !t.limbs.is_empty() && tree(t) && !(t.lies.is_some() && t.limbed)).map(|(n, t)| (reached(t), n)).filter(|(d, _)| *d < 0.7).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, n)| n);
        let mut cut = Cut::default();
        let mut which = near(&self.things);
        // (No tree that stands or lies in its boughs: then it is at timber, if any is there.)
        // (A blow among the boughs of a tree that lies with its boughs still on is at that
        // tree, though no limb of it is within a stride: its crown is leaf and twigs metres
        // from any limb, and a blow there was taken for one at timber, which bucked the
        // trunk it was joined to with its crown still on. A great tree at -32,-40.)
        if which.is_none() {
            which = self.things.iter().enumerate().filter(|(_, t)| t.lies.is_some() && !t.limbed && !t.limbs.is_empty()).map(|(n, t)| (reached(t), n)).filter(|(d, _)| *d < 4.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, n)| n);
        }
        if which.is_none() && let Some(cut) = self.timber(world, at, push) {
            return cut;
        }
        if which.is_none() {
            let known: Vec<Vec3> = self.things.iter().filter_map(|t| t.limbs.first().map(|l| l.a)).collect();
            cut = work(world, at, 0.5, forms, Some(Stuff::Wood), &known, true);
            self.note(&cut, forms, false);
            // (It is still among the shapes drawn, until it is cut through: below.)
            cut.trees.clear();
            which = near(&self.things);
        }
        let Some(n) = which else { return cut };
        let thing = self.things[n].clone();
        let wood = |b: Block| b.wood() || b == Block::Broken;
        let mut gone = 0u32;
        let clear = |world: &mut World, cell: IVec3, cut: &mut Cut| {
            world.set(cell, Block::Air);
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        };
        match thing.lies {
            None => {
                let Some(trunk) = thing.limbs.iter().find(|l| l.roots > 0).or(thing.limbs.first()).copied() else { return cut };
                let up = ((crate::plants::Limb::UNDER + 0.9) / (trunk.b.y - trunk.a.y).max(0.1)).min(0.8);
                let (middle, thick) = (trunk.a.lerp(trunk.b, up), trunk.ra + (trunk.rb - trunk.ra) * up);
                let needs = BLOWS(thick);
                // (Through already, and on its way down: a blow at where it stood is at nothing.)
                if thing.blows >= needs {
                    return cut;
                }
                let blows = thing.blows + 1;
                self.things[n].blows = blows;
                trace(format_args!("chop: standing, blow {blows} of {needs}"));
                let into = push.with_y(0.0).try_normalize().unwrap_or(Vec3::X);
                if blows < needs {
                    // The notch, a blow deeper: a wedge on the side the blows come from.
                    let deep = thick * 2.0 * blows as f32 / needs as f32 * 0.6;
                    self.things[n].notch = Some((middle, into, thick, deep));
                    let taken = carve(world, middle - into * (thick * 1.5 - deep) , thick * 0.75, Some(Stuff::Wood));
                    cut.taken[Stuff::Wood as usize] += taken[Stuff::Wood as usize];
                    let r = (thick * 3.0 / CELL).ceil() as i32 + 1;
                    let c = (middle / CELL).floor().as_ivec3();
                    // (Should a notch ever part it, it falls as if cut through.)
                    cut.changed.extend(self.loosen(world, c - r, c + r, push));
                } else {
                    // Through: a cell's height of its trunk, all the way across.
                    let (c, r) = ((middle / CELL).floor().as_ivec3(), ((thick * 2.4 + 0.2) / CELL).ceil() as i32);
                    for z in -r..=r {
                        for x in -r..=r {
                            for y in 0..=1 {
                                let cell = c + IVec3::new(x, y, z);
                                if x * x + z * z <= r * r && wood(world.get(cell)) {
                                    clear(world, cell, &mut cut);
                                    gone += 1;
                                }
                            }
                        }
                    }
                    // (A great tree is more cells than a body may be and still be looked
                    // through, and was taken for the land: cut clean through, it stood in
                    // the air. The developer saw one, 2026-11-13; of 35 trees tried then, the
                    // great ones did not come loose. Here it is known to be a tree.)
                    self.whole = true;
                    cut.changed.extend(self.loosen(world, c - IVec3::new(r, 1, r), c + IVec3::new(r, 2, r), push));
                    self.whole = false;
                    // It is no longer among the shapes that stand: whoever keeps those drops
                    // it, and draws it as it goes over and lies (`felled`), and its stump
                    // where it stands (`Thing::stump`).
                    self.things[n].felled = true;
                    cut.trees.extend(forms.iter().enumerate().filter_map(|(k, form)| form.limbs.iter().find(|l| l.a == trunk.a).map(|l| (k, l.tree))));
                    self.things[n].stump = true;
// What of its leaf did not go with it is shed where it hung: a piece takes the
                    // leaf near its wood, and a weeping tree's long strands were left hanging in
                    // the air where it had stood (seen in the game, 2026-11-13).
                    for clump in &thing.clumps {
                        let (bl, bh) = (((clump.middle - clump.radii) / CELL).floor().as_ivec3(), ((clump.middle + clump.radii) / CELL).floor().as_ivec3());
                        for y in bl.y..=bh.y {
                            for z in bl.z..=bh.z {
                                for x in bl.x..=bh.x {
                                    let cell = IVec3::new(x, y, z);
                                    let b = world.get(cell);
                                    // (Its own: within the mass it grew as, not all of the box round it.)
                                    if b.foliage() && !b.shaped() && (((cell.as_vec3() + 0.5) * CELL - clump.middle) / (clump.radii + CELL)).length_squared() < 1.0 {
                                        clear(world, cell, &mut cut);
                                        if (x + y * 3 + z * 7).rem_euclid(23) == 0 {
                                            self.shed.push(((cell.as_vec3() + 0.5) * CELL, b));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some((pivot, shift, turn)) => {
                let (now, was) = (|p: Vec3| pivot + shift + turn * (p - pivot), |p: Vec3| pivot + turn.inverse() * (p - pivot - shift));
                let stout = thing.limbs.iter().map(|l| l.ra.max(l.rb)).fold(0.0, f32::max) * 0.34;
                let heavy: Vec<crate::plants::Limb> = thing.limbs.iter().filter(|l| l.ra.max(l.rb) >= stout).copied().collect();
                let ends = thing.limbs.iter().flat_map(|l| [(l.a, l.ra.max(l.rb)), (l.b, l.ra.max(l.rb))]).chain(thing.clumps.iter().map(|c| (c.middle, c.radii.max_element())));
                let (mut from, mut to) = (Vec3::MAX, Vec3::MIN);
                for (p, r) in ends {
                    (from, to) = (from.min(now(p) - r - 0.5), to.max(now(p) + r + 0.5));
                }
                let (bl, bh) = ((from / CELL).floor().as_ivec3(), (to / CELL).floor().as_ivec3());
                let (lo, hi);
                {
                    // Its boughs and leaf: whatever of this tree is not its trunk or a heavy limb.
                    for y in bl.y..=bh.y {
                        for z in bl.z..=bh.z {
                            for x in bl.x..=bh.x {
                                let cell = IVec3::new(x, y, z);
                                let b = world.get(cell);
                                // (Of a cell in bits, what of this tree is in it: its twigs
                                // near the ground are bits, and what of it came down into a
                                // cell part filled. Passed over, a hundred such cells at a
                                // time were left in the air by the blow, came loose and lay
                                // where nobody could see them.)
                                if b == Block::Broken && !self.owner.contains_key(&cell) {
                                    let p = was((cell.as_vec3() + 0.5) * CELL);
                                    let its = thing.limbs.iter().filter(|l| l.ra.max(l.rb) < stout).any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 2.0) || thing.clumps.iter().any(|c| ((p - c.middle) / (c.radii + CELL * 1.5)).length_squared() < 1.0);
                                    if its && !heavy.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 1.5) {
                                        let mut any = false;
                                        for i in 0..SPLIT * SPLIT * SPLIT {
                                            let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                                            if world.bit(bit).whole() {
                                                world.set_bit(bit, Block::Air);
                                                any = true;
                                            }
                                        }
                                        if any {
                                            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                                        }
                                    }
                                    continue;
                                }
                                if !(b.wood() || b.foliage()) || b.shaped() || self.owner.contains_key(&cell) {
                                    continue;
                                }
                                let here = (cell.as_vec3() + 0.5) * CELL;
                                let p = was(here);
                                // (Its wood there: leaf against its trunk goes with the rest.)
                                if b.wood() && heavy.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 1.5) {
                                    continue;
                                }
                                let its = thing.limbs.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 2.0) || thing.clumps.iter().any(|c| ((p - c.middle) / (c.radii + CELL * 2.0)).length_squared() < 1.0);
                                if !its {
                                    continue;
                                }
                                clear(world, cell, &mut cut);
                                gone += b.wood() as u32;
                                if (x + y * 3 + z * 7).rem_euclid(9) == 0 {
                                    self.shed.push((here, b));
                                }
                            }
                        }
                    }
                    // (All of them at the one blow, wherever on it the blow lands: "the first
                    // chop probably should cut away the branches", the developer, having
                    // felled one, 2026-11-13. By how far a blow reached it took two, and
                    // before that a willow was never bare.)
                    self.things[n].limbed = true;
                    trace(format_args!("chop: fallen, its boughs and leaf knocked off"));
                    (lo, hi) = (bl, bh);
                }
                // (What a blow leaves on nothing comes down in its turn: a bough's end, its trunk.)
                let before = self.next;
                cut.changed.extend(self.loosen(world, lo - 1, hi + 1, push));
                // Its trunk, let down by the boughs it lay propped on, is still the tree, and
                // the tree lies where that comes to rest: whoever draws it draws it there.
                // (The piece its trunk's middle is in, not the biggest: a bough's end that
                // came down was taken for the tree.)
                let stem = thing.limbs.iter().find(|l| l.roots > 0).or(thing.limbs.first()).map(|l| [0.35, 0.5, 0.65].map(|along| (now(l.a.lerp(l.b, along)) / CELL).floor().as_ivec3()));
                if let Some(trunk) = stem.and_then(|stem| self.pieces.iter_mut().filter(|p| p.id > before).find(|p| stem.iter().filter(|c| p.cells.contains_key(*c)).count() >= 2)) {
                    trunk.thing = Some(thing.id);
                }
            }
        }
        // (One of timber to carry for each two cells of wood a blow takes, by a cut's own count.)
        cut.taken[Stuff::Wood as usize] += gone * (SPLIT * SPLIT * SPLIT) as u32 / 2;
        self.cuts.push((at, cut.taken));
        cut
    }

    /// A blow of an axe at timber: wood that lies cut, whatever tree it was and wherever it
    /// has come to rest since. The piece struck is all the cut wood joined to what the blow
    /// landed on. A trunk, it is bucked: all of it becomes logs at the one blow, one to each
    /// [`LOG`] of its length, each a straight length lying where that part of it lay. A
    /// log, it is split into hunks, which lie where it lay. Nothing, if no cut wood is
    /// there.
    ///
    /// (By what is there, not by where a tree's logs ought to be: asked of the tree, a log
    /// that had dropped a metre when its trunk was broken was not found by any blow, and
    /// half the wood of a thin tree could not be split. `cargo world hunks`, 2026-11-13.)
    pub(super) fn timber(&mut self, world: &mut World, at: Vec3, push: Vec3) -> Option<Cut> {
        let holds = |world: &World, cell: IVec3| match world.get(cell) {
            Block::Broken => world.bits_at(cell).is_some_and(|b| b.iter().any(|b| b.wood() && !b.shaped())),
            b => b.wood() && !b.shaped(),
        };
        // (Not what stands: a tree cut into and not yet down is for the blows that fell it.)
        let stands = |p: Vec3| self.things.iter().any(|t| t.lies.is_none() && t.limbs.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL * 2.0));
        let c = (at / CELL).floor().as_ivec3();
        let first = (-2..=2).flat_map(|y| (-2..=2).flat_map(move |z| (-2..=2).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| holds(world, *cell)).map(|cell| (((cell.as_vec3() + 0.5) * CELL).distance(at), cell)).filter(|(d, _)| *d < 0.45).min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))))?.1;
        if stands((first.as_vec3() + 0.5) * CELL) {
            return None;
        }
        // A log: split into hunks, which lie where it lay, nearest its middle line first.
        if let Some(n) = self.owner.get(&first).and_then(|id| self.rested.iter().position(|r| r.id == *id)) {
            let log = self.rested.remove(n);
            let mut cut = Cut::default();
            self.unrest(world, &log, &mut cut.changed);
            let (low, high) = log.cells.keys().fold((IVec3::MAX, IVec3::MIN), |(l, h), c| (l.min(*c), h.max(*c)));
            let middle = (low + high + 1).as_vec3() * CELL / 2.0;
            let (a, b) = (log.now(middle.with_x(low.x as f32 * CELL)), log.now(middle.with_x((high.x + 1) as f32 * CELL)));
            // (And the squares beside those, so that there are places apart from each other.)
            let mut places: Vec<(f32, IVec3)> = log.placed.iter().flat_map(|c| [0, -1].map(|down| IVec3::new(c.x.div_euclid(HUNK) * HUNK, c.y + down, c.z.div_euclid(HUNK) * HUNK))).flat_map(|c| [IVec3::ZERO, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].map(|d| c + d * HUNK)).collect::<HashSet<_>>().into_iter().map(|corner| (from_line((corner.as_vec3() + HUNK as f32 * 0.5) * CELL, a, b), corner)).collect();
            places.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| (x.1.y, x.1.z, x.1.x).cmp(&(y.1.y, y.1.z, y.1.x))));
            // (Two from a small log and three from a large, `SPLITS`: large is three cells
            // thick, a great tree's, which is more than sixty cells of it.)
            let many = SPLITS[(log.cells.len() > 60) as usize];
            let put = strew(world, places, many, Block::Billet, true, &mut cut.changed);
            for k in 0..6 {
                self.shed.push((a.lerp(b, (k as f32 + 0.5) / 6.0) + Vec3::Y * 0.3, Block::Timber));
            }
            trace(format_args!("chop: a log {:.1} m long split into {put} hunks of {many}", a.distance(b)));
            let (lo, hi) = log.placed.iter().fold((IVec3::MAX, IVec3::MIN), |(l, h), c| (l.min(*c), h.max(*c)));
            if lo.x <= hi.x {
                cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
            }
            cut.taken[Stuff::Wood as usize] = (log.cells.len() * 125) as u32;
            self.cuts.push((at, cut.taken));
            return Some(cut);
        }
        // (A piece of timber is not the logs that lie against it: each of those is its own.)
        let logs: HashSet<IVec3> = self.owner.keys().copied().collect();
        let holds = |world: &World, cell: IVec3| !logs.contains(&cell) && holds(world, cell);
        let (mut cut, tree) = self.worked(world, first, at, push, &holds);
        // A trunk is bucked whole at the one blow, though it lies in more than one piece:
        // every other piece of that tree's wood that is still as it fell is bucked too. (A
        // thick trunk lay in two or three, and took as many blows.)
        if let Some(thing) = tree.and_then(|id| self.things.iter().find(|t| t.id == id)).cloned() {
            let (pivot, shift, turn) = thing.lies.unwrap_or((Vec3::ZERO, Vec3::ZERO, Quat::IDENTITY));
            for _ in 0..12 {
                let left = thing.limbs.iter().flat_map(|l| (0..=(l.a.distance(l.b) / CELL) as i32).map(move |k| l.a.lerp(l.b, k as f32 * CELL / l.a.distance(l.b).max(CELL)))).map(|p| ((pivot + shift + turn * (p - pivot)) / CELL).floor().as_ivec3()).flat_map(|c| std::iter::once(c).chain(round().map(move |d| c + d))).find(|c| { let b = world.get(*c); b.whole() && b.wood() && !logs.contains(c) });
                let Some(next) = left else { break };
                let (more, _) = self.worked(world, next, (next.as_vec3() + 0.5) * CELL, push, &holds);
                cut.changed.extend(more.changed);
                cut.taken[Stuff::Wood as usize] += more.taken[Stuff::Wood as usize];
            }
        }
        Some(cut)
    }

    /// What a blow at timber does to the piece that has this cell in it ([`Loose::timber`]).
    /// Says what changed, and which tree it was the trunk of, if it bucked one.
    pub(super) fn worked(&mut self, world: &mut World, first: IVec3, at: Vec3, push: Vec3, holds: &dyn Fn(&World, IVec3) -> bool) -> (Cut, Option<u32>) {
        // The piece: every cell of cut wood joined to that one.
        let body = joined(world, first, holds);
        // From here it is cells, drawn as cells: the tree it was is no longer drawn as one.
        let mut made = HashSet::new();
        for cell in &body {
            let there = world.get(*cell);
            if there.whole() || there == Block::Broken {
                world.map(*cell, |b| if b.whole() { b.wrought() } else { b });
                made.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        // (The tree whose trunk is in this piece, by the middle of its trunk as it lies:
        // not one a scrap of which was struck, whose trunk then lay there unseen.)
        let own: HashSet<IVec3> = body.iter().copied().collect();
        let (mut tree, mut bucked) = (None, false);
        // (And a stump this piece has the middle of is cells from now, as it is.)
        for thing in self.things.iter_mut().filter(|t| t.stump) {
            let Some(trunk) = thing.limbs.iter().find(|l| l.roots > 0).or(thing.limbs.first()) else { continue };
            let foot = trunk.a + Vec3::Y * crate::plants::Limb::UNDER;
            if [0.2, 0.45].iter().any(|up| own.contains(&((foot + Vec3::Y * *up) / CELL).floor().as_ivec3())) {
                thing.stump = false;
            }
        }
        for thing in self.things.iter_mut().filter(|t| t.felled && !t.cells) {
            let (Some((pivot, shift, turn)), Some(trunk)) = (thing.lies, thing.limbs.iter().find(|l| l.roots > 0).or(thing.limbs.first())) else { continue };
            let stem = [0.4, 0.55, 0.7].map(|along| ((pivot + shift + turn * (trunk.a.lerp(trunk.b, along) - pivot)) / CELL).floor().as_ivec3());
            if stem.iter().filter(|c| round().any(|d| own.contains(&(**c + d))) || own.contains(*c)).count() >= 2 {
                thing.cells = true;
                tree = Some(thing.id);
            }
        }
        // Its length: from the cell furthest from the blow to the one furthest from that.
        let middle = |c: &IVec3| (c.as_vec3() + 0.5) * CELL;
        let furthest = |from: Vec3| body.iter().map(middle).max_by(|a, b| a.distance(from).total_cmp(&b.distance(from)).then_with(|| (a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap_or(std::cmp::Ordering::Equal))).unwrap_or(from);
        let a = furthest(at);
        let b = furthest(a);
        let (long, along) = (a.distance(b), (b - a).normalize_or(Vec3::X));
        let mut cut = Cut { changed: made, ..Cut::default() };
        let (mut lo, mut hi) = (IVec3::MAX, IVec3::MIN);
        let mut wood = 0u32;
        // (Its own wood out of a cell: all of a whole one, the bits of a broken one.)
        let mut empty = |world: &mut World, cell: IVec3, cut: &mut Cut| {
            if world.get(cell) == Block::Broken {
                for i in 0..SPLIT * SPLIT * SPLIT {
                    let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / SPLIT % SPLIT, i / (SPLIT * SPLIT));
                    if world.bit(bit).wood() {
                        world.set_bit(bit, Block::Air);
                        wood += 1;
                    }
                }
            } else {
                world.set(cell, Block::Air);
                wood += (SPLIT * SPLIT * SPLIT) as u32;
            }
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            (lo, hi) = (lo.min(cell), hi.max(cell));
        };
        let birch = body.iter().filter(|c| world.get(**c).grown() == Block::Birch).count() * 2 > body.len();
        if long < 0.75 {
            // A scrap: a bough's stub, a splinter. Chips.
            for cell in &body {
                empty(world, *cell, &mut cut);
            }
            self.shed.push((a.lerp(b, 0.5) + Vec3::Y * 0.2, Block::Timber));
            trace(format_args!("chop: a scrap {long:.1} m long, gone in chips"));
        } else {
            // Bucked: all of it at the one blow into logs, each a straight length of one
            // thickness lying along the grid where that part of it lay, bark on its sides
            // and cut wood at its ends. As many as it is long, as thick as it has wood for.
            // (One break a blow, a cell's width out of what was already cells, took eight
            // blows a tree and could hardly be seen: "when it breaks it should kind of be
            // more obvious, and the detail of it ... getting closer to the wood chunks
            // should make it more refined", the developer, 2026-11-13.)
            for cell in &body {
                empty(world, *cell, &mut cut);
            }
            let volume = wood as f32 * BIT * BIT * BIT;
            let many = ((long / LOG).round() as usize).max(1);
            let wide = (((volume / long.max(LOG * 0.5)).sqrt() / CELL).round() as i32).clamp(1, 3);
            // Each a piece of its own that keeps its shape ([`Rested`]): a straight beam,
            // its length along its own x, turned to lie along the line the trunk lay on,
            // and let go a little over where it lay to come down onto what is under it.
            let block = if birch { Block::BalkBirch } else { Block::Balk };
            // (A cell short of its share of the length: a gap between it and the next.)
            let each = long / many as f32;
            let length = ((each / CELL).floor() as i32 - 1).clamp(3, (LOG / CELL) as i32 - 1);
            let turn = Quat::from_rotation_arc(Vec3::X, along);
            for k in 0..many {
                let middle = a + along * each * (k as f32 + 0.5);
                let from = (middle / CELL).floor().as_ivec3() - IVec3::new(length / 2, wide / 2, wide / 2);
                let cells: Cells<Part> = (0..length).flat_map(|i| (0..wide).flat_map(move |j| (0..wide).map(move |y| from + IVec3::new(i, y, j)))).map(|c| (c, Part::Whole(block))).collect();
                let mut log = self.made(cells, middle, turn);
                // (Up out of what it lay sunk in, a cell at a time.)
                for _ in 0..8 {
                    if !log.feel.iter().any(|p| bears(world.bit((log.now(*p) / BIT).floor().as_ivec3()))) {
                        break;
                    }
                    log.shift.y += CELL;
                }
                self.pieces.push(log);
                for end in [-0.5f32, 0.5] {
                    self.shed.push((middle + along * each * end + Vec3::Y * 0.2, Block::Timber));
                }
            }
            bucked = true;
            trace(format_args!("chop: a trunk {long:.1} m long bucked into {many} logs, each {wide} cells thick"));
        }
        if lo.x <= hi.x {
            cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
        }
        cut.taken[Stuff::Wood as usize] = wood;
        self.cuts.push((at, cut.taken));
        (cut, tree.filter(|_| bucked))
    }

    /// A blow of a pick at loose stone: rubble, what a boulder or a cliff became when it came
    /// down, or what is left where a boulder broke. As timber to an axe: the piece struck
    /// is all the loose stone joined to what the blow landed on, and it breaks into hunks,
    /// as many as it has stone for; a scrap of under half a hunk is gone in chips. Nothing,
    /// if no loose stone is there, or it is more than a blow breaks (some 12 m3).
    pub(super) fn rubble(&mut self, world: &mut World, at: Vec3, push: Vec3) -> Option<Cut> {
        let holds = |world: &World, cell: IVec3| match world.get(cell) {
            Block::Broken => world.bits_at(cell).is_some_and(|b| b.contains(&Block::Shard)),
            b => b == Block::Shard,
        };
        let c = (at / CELL).floor().as_ivec3();
        // (This was among what an axe does to timber, and a pick did nothing to a lump:
        // found when a script first struck one in the real game, 2026-11-14.)
        // A lump lying at rest as itself: broken into hunks, as many as it has stone for,
        // which lie where it lay.
        let lump = (-2..=2).flat_map(|y| (-2..=2).flat_map(move |z| (-2..=2).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| self.owner.contains_key(cell) && (world.get(*cell) == Block::Scree || world.get(*cell) == Block::Broken && world.bits_at(*cell).is_some_and(|b| b.contains(&Block::Scree))) && ((cell.as_vec3() + 0.5) * CELL).distance(at) < 0.45).min_by(|a, b| ((a.as_vec3() + 0.5) * CELL).distance(at).total_cmp(&((b.as_vec3() + 0.5) * CELL).distance(at)).then_with(|| (a.y, a.z, a.x).cmp(&(b.y, b.z, b.x))));
        if let Some(n) = lump.and_then(|cell| self.owner.get(&cell)).and_then(|id| self.rested.iter().position(|r| r.id == *id)) {
            let lump = self.rested.remove(n);
            self.journal.push((self.steps, Edit::Chip { at, push }));
            let mut cut = Cut::default();
            self.unrest(world, &lump, &mut cut.changed);
            let stone = lump.cells.values().map(|p| match p { Part::Whole(_) => (SPLIT * SPLIT * SPLIT) as u32, Part::Bits(bits) => bits.iter().filter(|b| b.solid()).count() as u32 }).sum::<u32>();
            let many = (stone as f32 / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize;
            let middle = lump.now(lump.pivot);
            let places: Vec<(f32, IVec3)> = lump.placed.iter().map(|c| IVec3::new(c.x.div_euclid(HUNK) * HUNK, c.y, c.z.div_euclid(HUNK) * HUNK)).collect::<HashSet<_>>().into_iter().map(|corner| (corner.y as f32 * 8.0 + ((corner.as_vec3() + HUNK as f32 * 0.5) * CELL - middle).with_y(0.0).length(), corner)).collect();
            let put = strew(world, places, many, Block::Cobble, false, &mut cut.changed);
            for k in 0..4 {
                self.shed.push((middle + Vec3::new(k as f32 * 0.15 - 0.2, 0.2, 0.0), Block::Shard));
            }
            trace(format_args!("chip: a lump of {} cells broke into {put} hunks of {many}", lump.cells.len()));
            let (lo, hi) = lump.placed.iter().fold((IVec3::MAX, IVec3::MIN), |(l, h), c| (l.min(*c), h.max(*c)));
            if lo.x <= hi.x {
                cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
            }
            cut.taken[Stuff::Stone as usize] = stone;
            self.cuts.push((at, cut.taken));
            return Some(cut);
        }
        let first = (-2..=2).flat_map(|y| (-2..=2).flat_map(move |z| (-2..=2).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| holds(world, *cell)).map(|cell| (((cell.as_vec3() + 0.5) * CELL).distance(at), cell)).filter(|(d, _)| *d < 0.45).min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))))?.1;
        let body = joined(world, first, holds);
        if body.len() > 800 {
            return None;
        }
        self.journal.push((self.steps, Edit::Chip { at, push }));
        let mut cut = Cut::default();
        let (lo, hi) = body.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let middle = body.iter().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / body.len() as f32;
        // (From the ground up and from its middle out, on the squares its cells are over.)
        let places: Vec<(f32, IVec3)> = body.iter().map(|c| IVec3::new(c.x.div_euclid(HUNK) * HUNK, c.y, c.z.div_euclid(HUNK) * HUNK)).collect::<HashSet<_>>().into_iter().map(|corner| (corner.y as f32 * 8.0 + ((corner.as_vec3() + HUNK as f32 * 0.5) * CELL - middle).with_y(0.0).length(), corner)).collect();
        let own: HashSet<IVec3> = body.iter().copied().collect();
        let stone = emptied(world, lo, hi, |b| b == Block::Shard, |p| own.contains(&(p / CELL).floor().as_ivec3()), &mut cut.changed);
        let many = (stone as f32 / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize;
        let put = strew(world, places, many, Block::Cobble, false, &mut cut.changed);
        for k in 0..4 {
            self.shed.push((middle + Vec3::new(k as f32 * 0.15 - 0.2, 0.2, 0.0), Block::Shard));
        }
        trace(format_args!("chip: rubble of {} cells broke into {put} hunks of {many}", body.len()));
        cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
        cut.taken[Stuff::Stone as usize] = stone;
        self.cuts.push((at, cut.taken));
        Some(cut)
    }

    /// A blow of a pick on a boulder. As an axe to a tree, so a pick to a boulder: it is
    /// chipped, a little off it at each blow, until it has taken as many as its size asks
    /// ([`CHIPS`]), and at the last it breaks into hunks, as many as it has stone for, lying
    /// where it lay. Nothing, if what is struck is no boulder (the land's own stone is dug).
    pub fn chip(&mut self, world: &mut World, at: Vec3, push: Vec3, forms: &[&Form]) -> Option<Cut> {
        // (By where the boulder lies, not by what is at the blow: a blow into the hollow an
        // earlier one left is at air, and was taken for a blow at no boulder.)
        // Every boulder the blow reaches is cells from now and noted, and the blow is at the
        // one it is deepest in that is not yet broken. (The first found that it reached: of
        // two that lay against each other the one already noted took every blow at the
        // other, which never broke: seed 5, 2026-11-13.)
        let cut = work(world, at, 0.3, forms, Some(Stuff::Stone), &[], false);
        self.note(&cut, forms, true);
        let deep = |t: &Thing| t.stone.as_ref().filter(|s| t.blows < CHIPS(s.radii.max_element())).map(|s| ((at - s.middle) / (s.radii + 0.35)).length_squared()).filter(|d| *d < 1.0);
        let Some(n) = self.things.iter().enumerate().filter_map(|(n, t)| deep(t).map(|d| (d, n))).min_by(|a, b| a.0.total_cmp(&b.0)).map(|b| b.1) else {
            return self.rubble(world, at, push);
        };
        let mut cut = cut;
        let Some(stone) = self.things[n].stone.clone() else { return Some(cut) };
        let needs = CHIPS(stone.radii.max_element());
        self.journal.push((self.steps, Edit::Chip { at, push }));
        self.things[n].blows += 1;
        let (lo, hi);
        if self.things[n].blows < needs {
            cut.taken = carve(world, at, 0.22, Some(Stuff::Stone));
            (lo, hi) = (((at - 0.5) / CELL).floor().as_ivec3(), ((at + 0.5) / CELL).floor().as_ivec3());
            for k in 0..3 {
                self.shed.push((at + Vec3::new(k as f32 * 0.1 - 0.1, 0.1, 0.0), Block::Shard));
            }
        } else {
            let reach = stone.radii.max_element() + 0.3;
            (lo, hi) = (((stone.middle - reach) / CELL).floor().as_ivec3(), ((stone.middle + reach) / CELL).floor().as_ivec3());
            let its = |here: Vec3| here.distance(stone.middle) < reach;
            let held = emptied(world, lo, hi, |b| b == Block::Shard, its, &mut cut.changed);
            let many = ((held as f32 / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize).max(1);
            // A rock is often two or three stones that lie in each other: the stone of any
            // whose middle is within this one went with it, and it is broken too. (Left
            // unbroken it took three more blows and broke into nothing, with no stone in
            // it: in the game at -69,23, 2026-11-13.)
            for other in self.things.iter_mut().filter(|t| t.stone.as_ref().is_some_and(|s| its(s.middle))) {
                other.blows = other.blows.max(CHIPS(other.stone.as_ref().map_or(0.0, |s| s.radii.max_element())));
            }
            // From the ground up and from its middle out: a heap where it lay.
            let mut places = Vec::new();
            for y in lo.y..=hi.y {
                for z in (lo.z.div_euclid(HUNK) * HUNK..=hi.z).step_by(HUNK as usize) {
                    for x in (lo.x.div_euclid(HUNK) * HUNK..=hi.x).step_by(HUNK as usize) {
                        let corner = IVec3::new(x, y, z);
                        let middle = (corner.as_vec3() + HUNK as f32 * 0.5) * CELL;
                        if its(middle) {
                            places.push((y as f32 * 8.0 + (middle - stone.middle).with_y(0.0).length(), corner));
                        }
                    }
                }
            }
            let mut put = if held > 0 { strew(world, places, many, Block::Cobble, true, &mut cut.changed) } else { 0 };
            // (A small stone lies half in the ground and no hunk's place within it is all
            // free: what found no place among them is set down where it lay, on what is
            // there. In the game a stone of 30 to 70 cm broke into nothing, 2026-11-13.)
            while held > 0 && put < many && set_down(world, stone.middle, Block::Cobble, &mut cut.changed).is_some() {
                put += 1;
            }
            trace(format_args!("chip: a boulder {:.1} m across broke at blow {needs} into {put} hunks of {many}, {held} bits of stone in it", stone.radii.max_element() * 2.0));
            for k in 0..8 {
                let turn = k as f32 * 0.8;
                self.shed.push((stone.middle + Vec3::new(turn.cos(), 0.4, turn.sin()) * stone.radii * 0.7, Block::Shard));
            }
            cut.taken[Stuff::Stone as usize] = held;
        }
        cut.changed.extend(self.loosen(world, lo - 1, hi + 1, push));
        self.cuts.push((at, cut.taken));
        Some(cut)
    }

    /// (Earth does not stay where it is let go: it falls to what is under it and runs down
    /// off a heap, [`heaped`]. Stone and timber stay, which is building.)
    /// Puts a ball of something down, into air only (what is put down lies on what is there,
    /// not through it). Gives the chunks changed.
    pub fn put(&mut self, world: &mut World, middle: Vec3, radius: f32, block: Block) -> HashSet<IVec3> {
        self.journal.push((self.steps, Edit::Put { middle, radius, block }));
        let middle = if block == Block::Dirt { heaped(world, middle, radius) } else { middle };
        // A piece of stone or timber in hand (a lump's size) is put down as the whole cell
        // it is in: what is built is square and its faces meet, a wall and not a heap of
        // balls (seven pieces put one on another were one brown lump, 2026-11-13). Earth
        // is a lump, and anything larger is the ball asked for.
        if block != Block::Dirt && radius < CELL {
            let cell = (middle / CELL).floor().as_ivec3();
            if !world.get(cell).solid() || world.get(cell) == Block::Broken {
                world.set(cell, block);
            }
        } else {
            world.ellipsoid(middle, Vec3::splat(radius), block, true);
        }
        // (And the cells beside it, whose faces toward it are drawn or not by what it is.)
        let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3() - 1, ((middle + radius) / CELL).floor().as_ivec3() + 1);
        let mut changed: HashSet<IVec3> = (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK))))).collect();
        // (What is put where nothing holds it comes down like anything else cut loose: a
        // stone let go in the air hung there until 2026-11-13.)
        if block != Block::Dirt {
            changed.extend(self.loosen(world, lo, hi, Vec3::X));
        }
        changed
    }
}
