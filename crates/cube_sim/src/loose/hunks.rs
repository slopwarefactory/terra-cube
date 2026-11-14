//! Hunks: the pieces that are carried. Found in the grid by where they lie, strewn where a
//! log or a stone is broken, taken up and put down.

use super::*;

/// Whether a hunk's cell may go where this is: into air, water, what grows low, and leaf,
/// which gives way under it.
pub(super) fn room(world: &World, cell: IVec3) -> bool {
    match world.get(cell) {
        // (A cell part filled with leaf and nothing else is leaf: a hunk could neither go
        // into one, which was taken, nor lie on one, which bears nothing, and was lifted
        // over it and let fall into it for as long as anyone watched. The developer saw
        // one "infinitely falling on itself", 2026-11-13.)
        Block::Broken => world.bits_at(cell).is_some_and(|bits| bits.iter().all(|b| !b.solid() || b.foliage())),
        there => !there.solid() || there.foliage(),
    }
}

/// The cells of a hunk, from its lowest corner.
pub(super) fn hunk() -> impl Iterator<Item = IVec3> {
    (0..HUNK).flat_map(|y| (0..HUNK).flat_map(move |z| (0..HUNK).map(move |x| IVec3::new(x, y, z))))
}

/// The hunk a cell is part of: its lowest corner, and what it is of. Nothing is kept of
/// where hunks are: a hunk is the cells it is. It lies on the squares two cells a side that
/// start at even cells, and at whatever height it came to rest, counted in hunks from the
/// lowest of its kind under it (one put on another is one hunk higher).
pub fn hunk_at(world: &World, cell: IVec3) -> Option<(IVec3, Block)> {
    let block = world.get(cell);
    if !block.hunk() {
        return None;
    }
    let (x, z) = (cell.x.div_euclid(HUNK) * HUNK, cell.z.div_euclid(HUNK) * HUNK);
    let low = (1..64).map(|d| cell.y - d).find(|y| world.get(IVec3::new(x, *y, z)) != block).map_or(cell.y, |y| y + 1);
    let corner = IVec3::new(x, low + (cell.y - low).div_euclid(HUNK) * HUNK, z);
    hunk().all(|d| world.get(corner + d) == block).then_some((corner, block))
}

/// Every cell joined to one, by faces, edges or corners, that `holds` says is of the same
/// stuff: the piece a blow has struck. No further than 6,000 cells.
pub(super) fn joined(world: &World, first: IVec3, holds: impl Fn(&World, IVec3) -> bool) -> Vec<IVec3> {
    let (mut body, mut queue, mut seen) = (vec![first], VecDeque::from([first]), HashSet::from([first]));
    while let Some(cell) = queue.pop_front() {
        for side in round() {
            let next = cell + side;
            if body.len() < 6_000 && holds(world, next) && seen.insert(next) {
                body.push(next);
                queue.push_back(next);
            }
        }
    }
    body
}

/// Puts hunks down: as many as asked, at the first of the places given (each a lowest
/// corner, with what it is ordered by) that have room. Says how many found room.
///
/// `apart`: none of them beside another put with it, while there are places for that. The
/// hunks of a split log lay end to end as the log had, and looked like the log still: "they
/// should be spaced out a bit vs continuous (like they've been chopped into pieces)", the
/// developer, 2026-11-14. Hunks lie on squares of their own size, so apart is a square between.
pub(super) fn strew(world: &mut World, mut places: Vec<(f32, IVec3)>, many: usize, block: Block, apart: bool, changed: &mut HashSet<IVec3>) -> usize {
    places.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))));
    let mut put: Vec<IVec3> = Vec::new();
    for spaced in if apart { [true, false].iter() } else { [false].iter() } {
        for (_, corner) in &places {
            if put.len() == many {
                break;
            }
            let beside = |c: &IVec3| (*c - *corner).abs().cmple(IVec3::new(HUNK, HUNK - 1, HUNK)).all();
            if *spaced && put.iter().any(beside) {
                continue;
            }
            if hunk().all(|d| room(world, *corner + d)) {
                for d in hunk() {
                    world.set(*corner + d, block);
                    changed.insert((*corner + d).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                }
                put.push(*corner);
            }
        }
    }
    put.len()
}

/// Sets one hunk down at a place: on the square it is over, at that height or the least
/// above it that has room, as high as three more hunks. Gives its corner, if it found room.
pub(super) fn set_down(world: &mut World, at: Vec3, block: Block, changed: &mut HashSet<IVec3>) -> Option<IVec3> {
    let cell = (at / CELL).floor().as_ivec3();
    let (x, z) = (cell.x.div_euclid(HUNK) * HUNK, cell.z.div_euclid(HUNK) * HUNK);
    let corner = (0..=3 * HUNK).map(|up| IVec3::new(x, cell.y + up, z)).find(|c| hunk().all(|d| room(world, *c + d)))?;
    strew(world, vec![(0.0, corner)], 1, block, false, changed);
    Some(corner)
}

impl Loose {
    /// Takes up the hunk that is at a place, if one is: it is gone from the world, and what
    /// lay on it comes down. Gives what it was of, and the chunks changed.
    pub fn lift(&mut self, world: &mut World, at: Vec3) -> Option<(Block, HashSet<IVec3>)> {
        let (corner, block) = hunk_at(world, (at / CELL).floor().as_ivec3())?;
        self.journal.push((self.steps, Edit::Lift { at }));
        self.restless.clear();
        let mut changed = HashSet::new();
        for d in hunk() {
            world.set(corner + d, Block::Air);
            changed.insert((corner + d).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
        changed.extend(self.loosen(world, corner - 1, corner + HUNK, Vec3::X));
        Some((block, changed))
    }

    /// Puts a hunk down at a place: on the square it is over, at that height or the least
    /// above it that has room (as high as three more hunks: what a person lifts to), and
    /// from there it comes down onto what is under it. Nothing, if there is no room; else
    /// the corner it was put at, and the chunks changed.
    pub fn lay(&mut self, world: &mut World, at: Vec3, block: Block) -> Option<(IVec3, HashSet<IVec3>)> {
        let mut changed = HashSet::new();
        let corner = set_down(world, at, block, &mut changed)?;
        self.journal.push((self.steps, Edit::Lay { at, block }));
        changed.extend(self.loosen(world, corner - 1, corner + HUNK, Vec3::X));
        Some((corner, changed))
    }
}
