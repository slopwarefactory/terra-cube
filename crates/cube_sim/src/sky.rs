//! The calendar and the sky: what time it is, and where that puts the sun and the moon. Kept
//! in the simulation, not with the drawing, because days and seasons are rules of the world:
//! what sleeps, what hunts, what grows and what a person can see by follow them. The game
//! only draws the sky this describes.
//!
//! Time is one number, `when`: days since the world began, with the hour as its fraction
//! (0.5 is noon of the first day). A day lasts 24 minutes, a minute to the hour. Seven days
//! are a season and four seasons a year, starting with spring; the moon goes through its
//! phases once a season.

use glam::Vec3;
use std::f32::consts::TAU;

/// How long a day lasts, seconds: a minute to the hour.
pub const DAY: f32 = 1440.0;
/// Days in a season, seasons in a year, and days from one new moon to the next.
pub const SEASON: f32 = 7.0;
pub const YEAR: f32 = SEASON * 4.0;
pub const MONTH: f32 = 7.0;
/// When the world starts: the middle of spring (day and night equal), mid afternoon, the
/// moon waxing toward full.
pub const START: f32 = 3.0 + 15.0 / 24.0;
/// How far the sun's path leans from overhead at the equinox, radians: at noon it stands 62
/// degrees up, to the south.
const LEAN: f32 = 0.49;
/// How far the sun swings north and south over the year, radians: summer noons are 81
/// degrees up and the day thirteen and a half hours; winter noons 43 degrees and the day
/// ten and a half.
const SWING: f32 = 0.33;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// How hard the wind blows, from 0 (still air) to 1 (as hard as it gets): the weather, which
/// changes over hours and days (a still morning, a blowing afternoon, a windy week), and on
/// it the gusts, three swells of some seconds that never fall into step. The one wind:
/// what bends the leaves, what is heard and what drifts all take it from here. (Until
/// 2026-11-11 each of the three made its own, and leaves were heard to gust while they
/// were seen to hang still.)
pub fn wind(when: f32) -> f32 {
    use std::f32::consts::TAU;
    let weather = 0.58 + 0.25 * (when * TAU * 0.37 + 0.8).sin() + 0.15 * (when * TAU * 1.9 + 2.0).sin();
    let t = when * DAY;
    let gust = 0.5 + 0.5 * (0.5 * (t * 0.11).sin() + 0.3 * (t * 0.29 + 1.3).sin() + 0.2 * (t * 0.67 + 4.1).sin());
    weather * (0.3 + 0.7 * gust)
}

/// How much of the sky is cloud, 0 (clear) to 1 (overcast): the same weather that drives the
/// wind, so a blowing day is a clouded one, on a slower swell of its own, so there are clear
/// spells of days and dull ones. It changes over hours, never in a moment.
pub fn cloud(when: f32) -> f32 {
    use std::f32::consts::TAU;
    let front = 0.5 + 0.25 * (when * TAU * 0.37 + 0.8).sin() + 0.2 * (when * TAU * 0.13 + 1.7).sin() + 0.15 * (when * TAU * 1.9 + 2.0).sin();
    let t = ((front - 0.4) / 0.5).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How hard it rains, 0 (dry) to 1 (a downpour): only under heavy cloud, coming on and
/// passing over some minutes, in showers within a wet spell. In the cold it is snow that
/// falls ([`snows`]); how hard is the same.
pub fn rain(when: f32) -> f32 {
    use std::f32::consts::TAU;
    // (Clear three fifths of the time, overcast a fifth, raining an eighth, on two days in
    // five: tuned by those numbers, which the test below holds.)
    let wet = ((cloud(when) - 0.9) / 0.1).clamp(0.0, 1.0);
    let shower = 0.65 + 0.35 * (when * TAU * 7.3 + 0.6).sin();
    (wet * shower).clamp(0.0, 1.0)
}

/// Whether what falls is snow: in the cold of the year.
pub fn snows(when: f32) -> bool {
    warmth(when) < -0.35
}

/// `when` after `seconds` more.
pub fn later(when: f32, seconds: f32) -> f32 {
    when + seconds / DAY
}

/// The hour of the day, 0 to 24.
pub fn hour(when: f32) -> f32 {
    when.rem_euclid(1.0) * 24.0
}

/// `when` at an hour of a day.
pub fn at(day: u32, hour: f32) -> f32 {
    day as f32 + hour / 24.0
}

pub fn season(when: f32) -> Season {
    [Season::Spring, Season::Summer, Season::Autumn, Season::Winter][(when.rem_euclid(YEAR) / SEASON) as usize % 4]
}

/// How far through the year, 0 to 1, from the first day of spring.
pub fn year(when: f32) -> f32 {
    when.rem_euclid(YEAR) / YEAR
}

/// How warm the season is, -1 in the middle of winter to 1 in the middle of summer.
pub fn warmth(when: f32) -> f32 {
    ((year(when) - 0.125) * TAU).sin()
}

/// Which way a body is that crosses the sky as the sun does, at an hour angle (0 at its
/// highest) and so far north (up) of the sun's equinox path.
fn across(turn: f32, north: f32) -> Vec3 {
    let path = Vec3::new(-turn.sin(), turn.cos() * LEAN.cos(), turn.cos() * LEAN.sin());
    let pole = Vec3::new(0.0, LEAN.sin(), -LEAN.cos());
    (path * north.cos() + pole * north.sin()).normalize()
}

/// Which way the sun is, from the ground: up in the east (+x), highest at noon to the south,
/// down in the west, under the world at night; higher and longer up in summer.
pub fn sun(when: f32) -> Vec3 {
    across((hour(when) - 12.0) / 24.0 * TAU, SWING * warmth(when))
}

/// How much of day it is, 0 in full night to 1 in full day, passing through twilight while
/// the sun is within six degrees of the horizon.
pub fn daylight(when: f32) -> f32 {
    let t = ((sun(when).y + 0.105) / 0.21).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far the moon is through its phases, 0 to 1: new at 0, full at a half.
pub fn month(when: f32) -> f32 {
    when.rem_euclid(MONTH) / MONTH
}

/// Which way the moon is. It trails the sun across the sky by as much as it is through its
/// phases: beside the sun when new, opposite when full (so a full moon rises at sunset), and
/// it rides high when the sun rides low.
pub fn moon(when: f32) -> Vec3 {
    across((hour(when) - 12.0) / 24.0 * TAU - month(when) * TAU, -SWING * 0.6 * warmth(when))
}

/// How much of the moon's face is lit, 0 new to 1 full.
pub fn moon_lit(when: f32) -> f32 {
    (1.0 - (month(when) * TAU).cos()) / 2.0
}

/// How much light the moon gives the land, 0 to 1: by how full it is, and nothing when it is
/// down. A full moon is far more than twice a half one, as in life.
pub fn moonlight(when: f32) -> f32 {
    let up = ((moon(when).y + 0.02) / 0.12).clamp(0.0, 1.0);
    moon_lit(when).powf(1.6) * up
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_rains_now_and_then_and_not_for_ever() {
        // Over a year, by the minute: it is clear a good part of the time and overcast some;
        // it rains a tenth of the time or so, on a fair number of days, in spells of an hour
        // and more, and neither cloud nor rain ever jumps.
        let all: Vec<(f32, f32)> = (0..(super::YEAR * 24.0 * 60.0) as u32).map(|m| { let when = 3.0 + m as f32 / (24.0 * 60.0); (super::cloud(when), super::rain(when)) }).collect();
        let n = all.len() as f32;
        let (clear, dull, wet) = (all.iter().filter(|w| w.0 < 0.25).count() as f32 / n, all.iter().filter(|w| w.0 > 0.75).count() as f32 / n, all.iter().filter(|w| w.1 > 0.1).count() as f32 / n);
        let days = all.chunks(24 * 60).filter(|day| day.iter().any(|w| w.1 > 0.1)).count() as f32 / super::YEAR;
        let jump = all.windows(2).map(|w| (w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs())).fold(0.0, f32::max);
        assert!(clear > 0.2 && dull > 0.1 && (0.04..0.2).contains(&wet) && (0.15..0.6).contains(&days) && jump < 0.05, "clear {clear:.2} of the time, overcast {dull:.2}, raining {wet:.2}, on {days:.2} of days; the most it changes in a minute {jump:.3}");
    }

    #[test]
    fn the_wind_is_never_still_for_long_nor_always_blowing() {
        // Over a season, by the second: it stays in its range, there are lulls and blows,
        // and it never jumps (what sways by it would jump too).
        let all: Vec<f32> = (0..(super::SEASON * super::DAY) as u32).map(|s| super::wind(3.0 + s as f32 / super::DAY)).collect();
        let (low, high) = (all.iter().copied().fold(1.0, f32::min), all.iter().copied().fold(0.0, f32::max));
        let (lull, blow) = (all.iter().filter(|w| **w < 0.2).count() as f32 / all.len() as f32, all.iter().filter(|w| **w > 0.6).count() as f32 / all.len() as f32);
        let jump = all.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(low >= 0.0 && high <= 1.0 && lull > 0.05 && blow > 0.05 && jump < 0.12, "wind {low:.2} to {high:.2}, lulls {lull:.2}, blows {blow:.2}, jump {jump:.3}");
    }

    use super::*;

    #[test]
    fn the_sun_is_up_by_day_and_down_by_night() {
        let d = 3.0; // mid spring: the equinox
        assert!(sun(at(3, 12.0)).y > 0.85 && sun(at(3, 0.0)).y < -0.85);
        assert!(sun(d + 0.25).y.abs() < 0.02 && sun(d + 0.25).x > 0.99 && sun(d + 0.75).x < -0.99);
        assert_eq!((daylight(at(3, 12.0)), daylight(at(3, 0.0))), (1.0, 0.0));
        // Morning only gets lighter, evening only darker.
        assert!((0..60).all(|i| daylight(at(3, 4.0 + i as f32 * 0.1)) <= daylight(at(3, 4.1 + i as f32 * 0.1))));
        assert!((0..60).all(|i| daylight(at(3, 15.0 + i as f32 * 0.1)) >= daylight(at(3, 15.1 + i as f32 * 0.1))));
    }

    #[test]
    fn the_year_has_four_seasons_of_seven_days() {
        assert_eq!([0.0, 6.9, 7.0, 14.0, 21.0, 27.9, 28.0].map(season), [Season::Spring, Season::Spring, Season::Summer, Season::Autumn, Season::Winter, Season::Winter, Season::Spring]);
        // Summer days are long and the noon sun high; winter days short and the sun low.
        let lit = |day: u32| (0..240).filter(|i| sun(at(day, *i as f32 * 0.1)).y > 0.0).count() as f32 * 0.1;
        assert!(lit(10) > 13.0 && lit(24) < 11.0 && (lit(3) - 12.0).abs() < 0.6, "{} {} {}", lit(10), lit(24), lit(3));
        assert!(sun(at(10, 12.0)).y > sun(at(3, 12.0)).y && sun(at(3, 12.0)).y > sun(at(24, 12.0)).y);
        assert!(warmth(10.5) > 0.99 && warmth(24.5) < -0.99);
    }

    #[test]
    fn the_moon_goes_through_its_phases_once_a_season() {
        // New: beside the sun and dark. Full: opposite the sun, up at midnight, and bright.
        assert!(moon_lit(0.0) < 0.01 && moon(0.0).dot(sun(0.0)) > 0.9);
        let full = 3.5;
        assert!(moon_lit(full) > 0.99 && moon(full).dot(sun(full)) < -0.8);
        assert!(moon(at(4, 0.0)).y > 0.5 && moonlight(at(4, 0.0)) > 0.8);
        // Waxing: each night brighter than the last, to the full.
        assert!(moon_lit(1.0) < moon_lit(2.0) && moon_lit(2.0) < moon_lit(3.0) && moon_lit(5.0) > moon_lit(6.0));
        // No moon in the sky, no moonlight, however full.
        assert_eq!(moonlight(at(3, 12.0) + 0.02), 0.0);
        assert!((month(7.0 + 1.0) - month(1.0)).abs() < 1e-5);
    }

    #[test]
    fn time_goes_on() {
        assert!((later(START, DAY) - START - 1.0).abs() < 1e-4);
        assert!((hour(START) - 15.0).abs() < 1e-3);
    }
}
