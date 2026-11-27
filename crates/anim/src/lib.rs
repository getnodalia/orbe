// SPDX-License-Identifier: MIT
//! Orbe's animation: keyframe [`Track`]s that drive node properties over time, grouped into an
//! [`Animation`], and a [`Player`] that advances one.
//!
//! A track targets one property of one node, named by its path from the node the animation is
//! applied to. Sampling interpolates numbers, vectors and colours; every other value holds until
//! the next key. Everything that enters an animation is validated, so applying one to any scene
//! cannot panic: tracks whose node or property cannot be found are skipped.

use orbe_core::{Handle, Variant};
use orbe_math::Color;
use orbe_scene::{valid_property, Node, SceneTree};
use std::fmt;

/// Most keys one track may hold.
pub const MAX_KEYS: usize = 100_000;
/// Most tracks one animation may hold.
pub const MAX_TRACKS: usize = 10_000;
/// Longest animation, in seconds.
pub const MAX_LENGTH: f32 = 86_400.0;

/// Why an animation or track was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimError {
    /// The length is not a finite number of seconds in `(0, MAX_LENGTH]`.
    InvalidLength,
    /// The property name is not a valid property name.
    InvalidProperty,
    /// A key time is not finite, outside the animation, or not after the previous key.
    InvalidKeyTime,
    /// A track has no keys or more than [`MAX_KEYS`].
    InvalidKeyCount,
    /// The animation already has [`MAX_TRACKS`] tracks.
    TooManyTracks,
}

impl fmt::Display for AnimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AnimError::InvalidLength => "the animation length is not valid",
            AnimError::InvalidProperty => "the property name is not valid",
            AnimError::InvalidKeyTime => {
                "key times must be finite, inside the animation and increasing"
            }
            AnimError::InvalidKeyCount => {
                "a track needs between one and the maximum number of keys"
            }
            AnimError::TooManyTracks => "the animation has too many tracks",
        })
    }
}

impl std::error::Error for AnimError {}

/// How values are blended between two keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    /// Hold the earlier key's value until the next key.
    Step,
    /// Blend linearly where the values allow it, otherwise hold.
    Linear,
}

/// A value at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// Seconds from the start of the animation.
    pub time: f32,
    /// The value.
    pub value: Variant,
}

/// The keys of one property of one node.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    node: String,
    property: String,
    interpolation: Interpolation,
    keys: Vec<Key>,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Blends two values, or `None` when they cannot be blended.
fn blend(a: &Variant, b: &Variant, t: f32) -> Option<Variant> {
    Some(match (a, b) {
        (Variant::Float(x), Variant::Float(y)) => {
            Variant::Float(f64::from(lerp(*x as f32, *y as f32, t)))
        }
        (Variant::Int(x), Variant::Int(y)) => {
            Variant::Int(lerp(*x as f32, *y as f32, t).round() as i64)
        }
        (Variant::Vec2(x), Variant::Vec2(y)) => Variant::Vec2(x.lerp(*y, t)),
        (Variant::Vec3(x), Variant::Vec3(y)) => Variant::Vec3(x.lerp(*y, t)),
        (Variant::Color(x), Variant::Color(y)) => Variant::Color(Color::new(
            lerp(x.r, y.r, t),
            lerp(x.g, y.g, t),
            lerp(x.b, y.b, t),
            lerp(x.a, y.a, t),
        )),
        _ => return None,
    })
}

impl Track {
    /// A track for `property` of the node at `node` (a path from the animated root, `.` for the
    /// root itself). Keys must be non-empty, with finite, strictly increasing times.
    pub fn new(
        node: &str,
        property: &str,
        interpolation: Interpolation,
        keys: Vec<Key>,
    ) -> Result<Track, AnimError> {
        if !valid_property(property) {
            return Err(AnimError::InvalidProperty);
        }
        if keys.is_empty() || keys.len() > MAX_KEYS {
            return Err(AnimError::InvalidKeyCount);
        }
        let mut prev = f32::NEG_INFINITY;
        for k in &keys {
            if !k.time.is_finite() || k.time < 0.0 || k.time <= prev {
                return Err(AnimError::InvalidKeyTime);
            }
            prev = k.time;
        }
        Ok(Track {
            node: node.to_string(),
            property: property.to_string(),
            interpolation,
            keys,
        })
    }

    /// Path of the animated node.
    pub fn node(&self) -> &str {
        &self.node
    }

    /// The animated property.
    pub fn property(&self) -> &str {
        &self.property
    }

    /// The keys.
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    /// The value at time `t`: the first key's before it, the last key's after the last.
    pub fn sample(&self, t: f32) -> Option<Variant> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if !t.is_finite() || t <= first.time {
            return Some(first.value.clone());
        }
        if t >= last.time {
            return Some(last.value.clone());
        }
        // The first key strictly after `t`; there is always one before it.
        let after = self.keys.partition_point(|k| k.time <= t);
        let (a, b) = (self.keys.get(after.checked_sub(1)?)?, self.keys.get(after)?);
        if self.interpolation == Interpolation::Linear {
            let span = b.time - a.time;
            if span > 0.0 {
                if let Some(v) = blend(&a.value, &b.value, (t - a.time) / span) {
                    return Some(v);
                }
            }
        }
        Some(a.value.clone())
    }
}

/// A named set of tracks of a given length.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    name: String,
    length: f32,
    looping: bool,
    tracks: Vec<Track>,
}

impl Animation {
    /// An empty animation of `length` seconds.
    pub fn new(name: &str, length: f32, looping: bool) -> Result<Animation, AnimError> {
        if !length.is_finite() || length <= 0.0 || length > MAX_LENGTH {
            return Err(AnimError::InvalidLength);
        }
        Ok(Animation {
            name: name.to_string(),
            length,
            looping,
            tracks: Vec::new(),
        })
    }

    /// The name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Length in seconds.
    pub fn length(&self) -> f32 {
        self.length
    }

    /// Whether playback wraps around at the end.
    pub fn looping(&self) -> bool {
        self.looping
    }

    /// The tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Adds a track. Its keys must fall inside the animation.
    pub fn add_track(&mut self, track: Track) -> Result<(), AnimError> {
        if self.tracks.len() >= MAX_TRACKS {
            return Err(AnimError::TooManyTracks);
        }
        if track.keys.last().is_some_and(|k| k.time > self.length) {
            return Err(AnimError::InvalidKeyTime);
        }
        self.tracks.push(track);
        Ok(())
    }

    /// Maps a playback time to a time inside the animation: wrapped when looping, clamped
    /// otherwise.
    pub fn wrap(&self, t: f32) -> f32 {
        if !t.is_finite() {
            return 0.0;
        }
        if self.looping {
            t.rem_euclid(self.length)
        } else {
            t.clamp(0.0, self.length)
        }
    }

    /// Sets every animated property of the tree, below `root`, to its value at time `t`. Returns
    /// how many properties were set; tracks whose node cannot be found are skipped.
    pub fn apply(&self, tree: &mut SceneTree, root: Handle<Node>, t: f32) -> usize {
        let t = self.wrap(t);
        let mut applied = 0;
        for track in &self.tracks {
            let Some(node) = tree.find(root, &track.node) else {
                continue;
            };
            let Some(value) = track.sample(t) else {
                continue;
            };
            if tree.set_property(node, &track.property, value).is_ok() {
                applied += 1;
            }
        }
        applied
    }
}

/// Plays one animation: holds the time and advances it.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    animation: Animation,
    time: f32,
    speed: f32,
    playing: bool,
    finished: bool,
}

impl Player {
    /// A stopped player at the start of `animation`.
    pub fn new(animation: Animation) -> Player {
        Player {
            animation,
            time: 0.0,
            speed: 1.0,
            playing: false,
            finished: false,
        }
    }

    /// The animation.
    pub fn animation(&self) -> &Animation {
        &self.animation
    }

    /// The current time inside the animation.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Whether it is advancing.
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Sets the playback speed (negative plays backwards). A non-finite value is ignored.
    pub fn set_speed(&mut self, speed: f32) {
        if speed.is_finite() {
            self.speed = speed.clamp(-1000.0, 1000.0);
        }
    }

    /// Starts or resumes playback from the current time.
    pub fn play(&mut self) {
        self.playing = true;
        self.finished = false;
    }

    /// Stops and rewinds.
    pub fn stop(&mut self) {
        self.playing = false;
        self.time = 0.0;
        self.finished = false;
    }

    /// Jumps to a time inside the animation.
    pub fn seek(&mut self, t: f32) {
        self.time = self.animation.wrap(t);
    }

    /// Advances by `dt` seconds. Returns `true` on the step a non-looping animation reaches its end.
    pub fn advance(&mut self, dt: f32) -> bool {
        if !self.playing || !dt.is_finite() || dt < 0.0 {
            return false;
        }
        let next = self.time + dt * self.speed;
        if self.animation.looping() {
            self.time = self.animation.wrap(next);
            return false;
        }
        self.time = self.animation.wrap(next);
        let at_end = (self.speed >= 0.0 && next >= self.animation.length())
            || (self.speed < 0.0 && next <= 0.0);
        if at_end && !self.finished {
            self.finished = true;
            self.playing = false;
            return true;
        }
        false
    }

    /// Applies the current time to the scene; see [`Animation::apply`].
    pub fn apply(&self, tree: &mut SceneTree, root: Handle<Node>) -> usize {
        self.animation.apply(tree, root, self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbe_math::Vec2;

    fn key(time: f32, value: Variant) -> Key {
        Key { time, value }
    }

    fn floats(points: &[(f32, f64)]) -> Vec<Key> {
        points
            .iter()
            .map(|&(t, v)| key(t, Variant::Float(v)))
            .collect()
    }

    #[test]
    fn linear_and_step_sampling() {
        let lin = Track::new(
            ".",
            "x",
            Interpolation::Linear,
            floats(&[(0.0, 0.0), (2.0, 10.0), (4.0, 0.0)]),
        )
        .unwrap();
        assert_eq!(lin.sample(-1.0), Some(Variant::Float(0.0)));
        assert_eq!(lin.sample(1.0), Some(Variant::Float(5.0)));
        assert_eq!(lin.sample(3.0), Some(Variant::Float(5.0)));
        assert_eq!(lin.sample(9.0), Some(Variant::Float(0.0)));
        let step = Track::new(
            ".",
            "x",
            Interpolation::Step,
            floats(&[(0.0, 1.0), (1.0, 2.0)]),
        )
        .unwrap();
        assert_eq!(step.sample(0.99), Some(Variant::Float(1.0)));
        assert_eq!(step.sample(1.0), Some(Variant::Float(2.0)));
        assert_eq!(step.sample(f32::NAN), Some(Variant::Float(1.0)));
    }

    #[test]
    fn vectors_colours_ints_and_holds() {
        let v = Track::new(
            ".",
            "position",
            Interpolation::Linear,
            vec![
                key(0.0, Variant::Vec2(Vec2::new(0.0, 0.0))),
                key(1.0, Variant::Vec2(Vec2::new(10.0, -10.0))),
            ],
        )
        .unwrap();
        assert_eq!(v.sample(0.5), Some(Variant::Vec2(Vec2::new(5.0, -5.0))));
        let c = Track::new(
            ".",
            "color",
            Interpolation::Linear,
            vec![
                key(0.0, Variant::Color(Color::new(0.0, 0.0, 0.0, 1.0))),
                key(1.0, Variant::Color(Color::new(1.0, 1.0, 1.0, 1.0))),
            ],
        )
        .unwrap();
        assert_eq!(
            c.sample(0.5),
            Some(Variant::Color(Color::new(0.5, 0.5, 0.5, 1.0)))
        );
        let i = Track::new(
            ".",
            "n",
            Interpolation::Linear,
            vec![key(0.0, Variant::Int(0)), key(1.0, Variant::Int(10))],
        )
        .unwrap();
        assert_eq!(i.sample(0.5), Some(Variant::Int(5)));
        // Strings cannot be blended: they hold.
        let s = Track::new(
            ".",
            "text",
            Interpolation::Linear,
            vec![key(0.0, "a".into()), key(1.0, "b".into())],
        )
        .unwrap();
        assert_eq!(s.sample(0.5), Some(Variant::from("a")));
    }

    #[test]
    fn tracks_and_animations_are_validated() {
        assert_eq!(
            Track::new(".", "bad name", Interpolation::Step, floats(&[(0.0, 0.0)])),
            Err(AnimError::InvalidProperty)
        );
        assert_eq!(
            Track::new(".", "x", Interpolation::Step, vec![]),
            Err(AnimError::InvalidKeyCount)
        );
        assert_eq!(
            Track::new(
                ".",
                "x",
                Interpolation::Step,
                floats(&[(1.0, 0.0), (1.0, 1.0)])
            ),
            Err(AnimError::InvalidKeyTime)
        );
        assert_eq!(
            Track::new(".", "x", Interpolation::Step, floats(&[(f32::NAN, 0.0)])),
            Err(AnimError::InvalidKeyTime)
        );
        assert_eq!(
            Track::new(".", "x", Interpolation::Step, floats(&[(-1.0, 0.0)])),
            Err(AnimError::InvalidKeyTime)
        );
        assert_eq!(
            Animation::new("a", 0.0, false),
            Err(AnimError::InvalidLength)
        );
        assert_eq!(
            Animation::new("a", f32::INFINITY, false),
            Err(AnimError::InvalidLength)
        );
        let mut a = Animation::new("a", 1.0, false).unwrap();
        let late = Track::new(".", "x", Interpolation::Step, floats(&[(2.0, 0.0)])).unwrap();
        assert_eq!(a.add_track(late), Err(AnimError::InvalidKeyTime));
    }

    #[test]
    fn applying_to_a_scene() {
        let mut tree = SceneTree::new("Root", "Node2D").unwrap();
        let root = tree.root();
        let child = tree.add_child(root, "Child", "Node2D").unwrap();
        let mut a = Animation::new("slide", 2.0, true).unwrap();
        a.add_track(
            Track::new(
                "Child",
                "x",
                Interpolation::Linear,
                floats(&[(0.0, 0.0), (2.0, 100.0)]),
            )
            .unwrap(),
        )
        .unwrap();
        a.add_track(
            Track::new("Missing", "x", Interpolation::Step, floats(&[(0.0, 1.0)])).unwrap(),
        )
        .unwrap();
        assert_eq!(a.apply(&mut tree, root, 1.0), 1);
        assert_eq!(tree.property(child, "x"), Some(&Variant::Float(50.0)));
        // Looping wraps: 3.0 is 1.0 into the second run.
        a.apply(&mut tree, root, 3.0);
        assert_eq!(tree.property(child, "x"), Some(&Variant::Float(50.0)));
        assert_eq!(a.wrap(f32::NAN), 0.0);
    }

    #[test]
    fn player_advances_loops_and_finishes() {
        let mut a = Animation::new("a", 1.0, false).unwrap();
        a.add_track(
            Track::new(
                ".",
                "x",
                Interpolation::Linear,
                floats(&[(0.0, 0.0), (1.0, 1.0)]),
            )
            .unwrap(),
        )
        .unwrap();
        let mut p = Player::new(a.clone());
        assert!(!p.advance(0.5), "a stopped player does not move");
        assert_eq!(p.time(), 0.0);
        p.play();
        assert!(!p.advance(0.4));
        assert!((p.time() - 0.4).abs() < 1e-6);
        assert!(p.advance(0.8), "reaching the end is reported once");
        assert_eq!(p.time(), 1.0);
        assert!(!p.playing());
        assert!(!p.advance(0.1));
        p.set_speed(-1.0);
        p.play();
        assert!(p.advance(2.0));
        assert_eq!(p.time(), 0.0);

        let mut looped = Player::new(Animation::new("l", 1.0, true).unwrap());
        looped.play();
        assert!(!looped.advance(2.25));
        assert!((looped.time() - 0.25).abs() < 1e-5);
        looped.seek(f32::NAN);
        assert_eq!(looped.time(), 0.0);
        looped.set_speed(f32::NAN);
        assert!(!looped.advance(f32::NAN) && !looped.advance(-1.0));
        looped.stop();
        assert!(!looped.playing());
    }
}
