//! What things weigh and how they fall: the numbers of the world's physics, in one place.
//!
//! There are three kinds of thing that move, and one world they move in.
//!
//! - **The world is cells** (`cube_voxel`), and the only thing anything is stopped by. A
//!   tree, a wall, a hunk put down: each is cells, and so each is ground to whatever comes
//!   after it, with nothing more to be said about it.
//! - **A body** (`body`): a creature or the player, an upright box held out of solid cells.
//!   It walks up whatever is no higher than its step (`Tuning::step`, 0.55 m: a hunk, a
//!   thin log, a block of the ground), must jump what is higher, and falls at [`GRAVITY`].
//!   Bodies push each other apart over the ground and nothing else.
//! - **A piece** (`loose`): cells cut loose, moving as one rigid thing until it comes to
//!   rest and is cells again. It falls at [`GRAVITY`] as a body does; a tall one that
//!   stands on its end goes over about its foot ([`TIPPING`]). In the air it is stopped by
//!   the world alone: it does not meet bodies or other pieces until it has landed, when
//!   it is the world and whoever was under it is struck ([`harm`]).
//!
//! What a thing weighs changes none of how fast it falls. It changes what it does: how hard
//! it strikes ([`harm`]), how loud it lands, how fast it is carried ([`burden`]) and, as
//! more is built on this, whether it can be pushed. So everything has a mass, from what it is made
//! of ([`density`]) and how much of it there is, and nothing has a weight of its own
//! written anywhere else.

use cube_voxel::Block;

/// Metres a second a second that a body or a loose thing falls at. More than twice the
/// earth's, as in most games a person jumps in: at 9.8 a jump hangs. (`body::Tuning`'s own
/// default is this.)
pub const GRAVITY: f32 = 22.0;

/// The gravity a tall thing goes over at, about its foot: the earth's. How long a tree takes
/// to come down is the one thing that says how big it is, and at [`GRAVITY`] a birch was on
/// the ground 1.8 s after it was cut through and a willow 1.3, where a tree takes four or
/// five ("when it falls its not really like a tree falling", the developer, 2026-11-13).
pub const TIPPING: f32 = 9.81;

/// The fastest the top of a thing going over moves, metres a second: its crown is held by
/// the air, and it does not come down faster and faster to the end.
pub const SWEEP: f32 = 13.0;

/// Kilograms to a cubic metre of what a block is: green wood, stone, earth, leaf. Nothing
/// for air and what grows low.
pub fn density(block: Block) -> f32 {
    use Block::*;
    match block {
        b if b.wood() => 650.0,
        Billet => 650.0,
        b if b.foliage() => 40.0,
        Stone | Slate | Chalk | Rock | Shard | Cobble | Scree => 2600.0,
        Gravel => 1800.0,
        Sand => 1600.0,
        Grass | Dirt | Turf | Moss => 1500.0,
        Litter => 400.0,
        Snow => 300.0,
        Water => 1000.0,
        _ => 0.0,
    }
}

/// The heaviest thing lying loose that a person shoves along the ground by leaning on it,
/// kilograms: a lump of rubble or a thin log, not a boulder's half or a great log (half a
/// tonne of birch). What is heavier is broken up and carried.
pub const SHOVED: f32 = 200.0;

/// How fast a thing must come down to hurt anybody, metres a second: a drop of 0.8 m. What
/// is set down, or settles as it is made (a log cut to hunks at the feet of whoever cut it),
/// comes slower, and took health from them half the times they did either.
pub const GENTLE: f32 = 4.0;

/// Health a thing that lands on a body takes from it: by what it weighs and how much faster
/// than [`GENTLE`] it came, 400 at the most. A clod of 90 kg dropped a metre and a half is a
/// bruise (5); a tonne and a half of trunk at ten metres a second kills.
pub fn harm(mass: f32, speed: f32) -> f32 {
    (mass * (speed - GENTLE).max(0.0) / 25.0).min(400.0)
}

/// How much of their walking pace someone keeps with this in their arms, of 1: all of it up
/// to 120 kg, and less as it is heavier, six tenths at the least. A hunk of wood is 80 kg
/// and is walked with; one of stone is 325, and is hauled.
pub fn burden(mass: f32) -> f32 {
    (120.0 / mass.max(1.0)).clamp(0.6, 1.0)
}

/// What a hunk of this weighs, kilograms (`loose::HUNK` cells a side).
pub fn hunk(block: Block) -> f32 {
    density(block) * (crate::loose::HUNK as f32 * cube_voxel::CELL).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_heavy_strikes_hard_and_what_is_light_does_not() {
        let cubic = |b: Block| density(b);
        assert!(cubic(Block::Stone) > cubic(Block::Wood) * 3.0 && cubic(Block::Wood) > cubic(Block::Leaves) * 10.0);
        // A cubic metre of leaf that comes down is nothing; of wood, a hard blow; of stone, death.
        let (leaf, wood, stone) = (harm(cubic(Block::Leaves), 8.0), harm(cubic(Block::Wood), 8.0), harm(cubic(Block::Stone), 8.0));
        assert!(leaf < 10.0 && (60.0..200.0).contains(&wood) && stone > 300.0, "{leaf} {wood} {stone}");
        // What is set down or settles where it was made hurts nobody, whatever it weighs.
        assert_eq!(harm(hunk(Block::Cobble), 2.5), 0.0);
        // A hunk of wood is carried at a walk; one of stone slower.
        assert_eq!(burden(hunk(Block::Billet)), 1.0);
        assert!((0.6..0.7).contains(&burden(hunk(Block::Cobble))), "{}", burden(hunk(Block::Cobble)));
    }
}
