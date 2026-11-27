// SPDX-License-Identifier: MIT
//! The physics world and its solver.

use crate::collide::{box_vertices, circle_circle, circle_polygon, polygon_polygon, Manifold};
use orbe_core::{Arena, Handle};
use orbe_math::Vec2;
use std::fmt;

/// Most bodies a world holds.
pub const MAX_BODIES: usize = 100_000;
/// Most contact manifolds one step keeps; further pairs are ignored for that step.
pub const MAX_CONTACTS: usize = 1_000_000;
/// Longest time step; longer ones are cut to this.
pub const MAX_STEP: f32 = 0.1;
/// Fastest a body may move or turn; faster ones are clamped.
pub const MAX_SPEED: f32 = 1.0e5;

/// Solver iterations per step.
const ITERATIONS: usize = 10;
/// Overlap that is tolerated so resting contacts stay stable.
const SLOP: f32 = 0.005;
/// Share of the remaining overlap removed per step.
const CORRECTION: f32 = 0.4;
/// Impact speed below which restitution is ignored.
const BOUNCE_THRESHOLD: f32 = 1.0;
/// Largest shape dimension and position accepted.
const MAX_EXTENT: f32 = 1.0e6;

/// A body's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A circle around the body's position.
    Circle {
        /// Radius.
        radius: f32,
    },
    /// A box centred on the body's position, turned by its angle.
    Box {
        /// Half the width and half the height.
        half: Vec2,
    },
}

impl Shape {
    fn area(self) -> f32 {
        match self {
            Shape::Circle { radius } => std::f32::consts::PI * radius * radius,
            Shape::Box { half } => 4.0 * half.x * half.y,
        }
    }

    /// Moment of inertia per unit of mass.
    fn inertia_per_mass(self) -> f32 {
        match self {
            Shape::Circle { radius } => 0.5 * radius * radius,
            Shape::Box { half } => (4.0 * half.x * half.x + 4.0 * half.y * half.y) / 12.0,
        }
    }

    fn valid(self) -> bool {
        let ok = |v: f32| v.is_finite() && v > 0.0 && v <= MAX_EXTENT;
        match self {
            Shape::Circle { radius } => ok(radius),
            Shape::Box { half } => ok(half.x) && ok(half.y),
        }
    }
}

/// Whether a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Never moves; has infinite mass.
    Static,
    /// Moves under gravity and collisions.
    Dynamic,
}

/// How to create a body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyDef {
    /// Static or dynamic.
    pub kind: BodyKind,
    /// The shape.
    pub shape: Shape,
    /// Position of the centre.
    pub position: Vec2,
    /// Rotation in radians.
    pub angle: f32,
    /// Mass per unit of area (dynamic bodies).
    pub density: f32,
    /// Bounciness, from 0 to 1.
    pub restitution: f32,
    /// Friction coefficient, 0 or more.
    pub friction: f32,
    /// Starting linear velocity.
    pub linear_velocity: Vec2,
    /// Starting angular velocity in radians per second.
    pub angular_velocity: f32,
}

impl BodyDef {
    /// A body of this kind and shape at `position` with density 1, friction 0.4 and no bounce.
    pub fn new(kind: BodyKind, shape: Shape, position: Vec2) -> BodyDef {
        BodyDef {
            kind,
            shape,
            position,
            angle: 0.0,
            density: 1.0,
            restitution: 0.0,
            friction: 0.4,
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
        }
    }
}

/// Why a body could not be added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsError {
    /// A number in the definition is not finite or is out of range.
    InvalidBody,
    /// The world already holds [`MAX_BODIES`] bodies.
    TooManyBodies,
}

impl fmt::Display for PhysicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PhysicsError::InvalidBody => "the body has a non-finite or out-of-range value",
            PhysicsError::TooManyBodies => "the world has too many bodies",
        })
    }
}

impl std::error::Error for PhysicsError {}

/// A body in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    kind: BodyKind,
    shape: Shape,
    position: Vec2,
    angle: f32,
    linear_velocity: Vec2,
    angular_velocity: f32,
    restitution: f32,
    friction: f32,
    inv_mass: f32,
    inv_inertia: f32,
}

impl Body {
    /// Static or dynamic.
    pub fn kind(&self) -> BodyKind {
        self.kind
    }
    /// The shape.
    pub fn shape(&self) -> Shape {
        self.shape
    }
    /// Position of the centre.
    pub fn position(&self) -> Vec2 {
        self.position
    }
    /// Rotation in radians.
    pub fn angle(&self) -> f32 {
        self.angle
    }
    /// Linear velocity.
    pub fn linear_velocity(&self) -> Vec2 {
        self.linear_velocity
    }
    /// Angular velocity in radians per second.
    pub fn angular_velocity(&self) -> f32 {
        self.angular_velocity
    }

    fn polygon(&self) -> Option<[Vec2; 4]> {
        match self.shape {
            Shape::Box { half } => Some(box_vertices(self.position, self.angle, half)),
            Shape::Circle { .. } => None,
        }
    }

    /// Axis-aligned bounds as `(min, max)`.
    fn bounds(&self) -> (Vec2, Vec2) {
        match (self.shape, self.polygon()) {
            (Shape::Circle { radius }, _) => (
                self.position - Vec2::new(radius, radius),
                self.position + Vec2::new(radius, radius),
            ),
            (_, Some(p)) => {
                let mut lo = Vec2::new(f32::INFINITY, f32::INFINITY);
                let mut hi = Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
                for v in p {
                    lo = Vec2::new(lo.x.min(v.x), lo.y.min(v.y));
                    hi = Vec2::new(hi.x.max(v.x), hi.y.max(v.y));
                }
                (lo, hi)
            }
            _ => (self.position, self.position),
        }
    }
}

/// Identifies a body in a [`World`].
pub type BodyId = Handle<Body>;

/// A set of bodies and the rules they move by.
#[derive(Debug)]
pub struct World {
    bodies: Arena<Body>,
    gravity: Vec2,
    contacts: Vec<(BodyId, BodyId, Manifold)>,
}

fn finite_vec(v: Vec2) -> bool {
    v.is_finite() && v.x.abs() <= MAX_EXTENT && v.y.abs() <= MAX_EXTENT
}

fn clamp_speed(v: Vec2) -> Vec2 {
    let len = v.length();
    if len > MAX_SPEED {
        v * (MAX_SPEED / len)
    } else {
        v
    }
}

/// `w x r` for a scalar angular velocity.
fn cross_sv(w: f32, r: Vec2) -> Vec2 {
    Vec2::new(-w * r.y, w * r.x)
}

struct Constraint {
    a: usize,
    b: usize,
    normal: Vec2,
    ra: Vec2,
    rb: Vec2,
    mass_n: f32,
    mass_t: f32,
    bias: f32,
    friction: f32,
    jn: f32,
    jt: f32,
}

impl World {
    /// An empty world with this gravity (units per second squared).
    pub fn new(gravity: Vec2) -> World {
        let gravity = if gravity.is_finite() {
            gravity
        } else {
            Vec2::ZERO
        };
        World {
            bodies: Arena::new(),
            gravity,
            contacts: Vec::new(),
        }
    }

    /// The gravity vector.
    pub fn gravity(&self) -> Vec2 {
        self.gravity
    }

    /// Sets the gravity; a non-finite vector is ignored.
    pub fn set_gravity(&mut self, gravity: Vec2) {
        if gravity.is_finite() {
            self.gravity = gravity;
        }
    }

    /// How many bodies there are.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether there are no bodies.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Adds a body.
    pub fn add_body(&mut self, def: BodyDef) -> Result<BodyId, PhysicsError> {
        let ok = def.shape.valid()
            && finite_vec(def.position)
            && def.angle.is_finite()
            && def.density.is_finite()
            && def.density > 0.0
            && def.density <= MAX_EXTENT
            && (0.0..=1.0).contains(&def.restitution)
            && def.friction.is_finite()
            && def.friction >= 0.0
            && def.linear_velocity.is_finite()
            && def.angular_velocity.is_finite();
        if !ok {
            return Err(PhysicsError::InvalidBody);
        }
        if self.bodies.len() >= MAX_BODIES {
            return Err(PhysicsError::TooManyBodies);
        }
        let (inv_mass, inv_inertia, lin, ang) = match def.kind {
            BodyKind::Static => (0.0, 0.0, Vec2::ZERO, 0.0),
            BodyKind::Dynamic => {
                let mass = def.density * def.shape.area();
                let inertia = mass * def.shape.inertia_per_mass();
                if !(mass.is_finite() && mass > 0.0 && inertia.is_finite() && inertia > 0.0) {
                    return Err(PhysicsError::InvalidBody);
                }
                (
                    1.0 / mass,
                    1.0 / inertia,
                    clamp_speed(def.linear_velocity),
                    def.angular_velocity.clamp(-MAX_SPEED, MAX_SPEED),
                )
            }
        };
        self.bodies
            .insert(Body {
                kind: def.kind,
                shape: def.shape,
                position: def.position,
                angle: def.angle,
                linear_velocity: lin,
                angular_velocity: ang,
                restitution: def.restitution,
                friction: def.friction,
                inv_mass,
                inv_inertia,
            })
            .ok_or(PhysicsError::TooManyBodies)
    }

    /// Removes a body. Returns whether it existed.
    pub fn remove_body(&mut self, id: BodyId) -> bool {
        self.bodies.remove(id).is_some()
    }

    /// Looks a body up.
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id)
    }

    /// Sets a dynamic body's linear velocity. Returns whether it was applied.
    pub fn set_linear_velocity(&mut self, id: BodyId, v: Vec2) -> bool {
        match self.bodies.get_mut(id) {
            Some(b) if b.kind == BodyKind::Dynamic && v.is_finite() => {
                b.linear_velocity = clamp_speed(v);
                true
            }
            _ => false,
        }
    }

    /// Applies an impulse (mass times velocity change) to a dynamic body's centre.
    pub fn apply_impulse(&mut self, id: BodyId, impulse: Vec2) -> bool {
        match self.bodies.get_mut(id) {
            Some(b) if b.kind == BodyKind::Dynamic && impulse.is_finite() => {
                b.linear_velocity = clamp_speed(b.linear_velocity + impulse * b.inv_mass);
                true
            }
            _ => false,
        }
    }

    /// The contacts found by the last step: the two bodies and their manifold, with the normal
    /// pointing from the first to the second.
    pub fn contacts(&self) -> &[(BodyId, BodyId, Manifold)] {
        &self.contacts
    }

    /// Advances the simulation by `dt` seconds. A non-finite or non-positive `dt` does nothing;
    /// one longer than [`MAX_STEP`] is cut to it.
    pub fn step(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(MAX_STEP);
        let handles: Vec<BodyId> = self.bodies.iter().map(|(h, _)| h).collect();
        let mut work: Vec<Body> = handles
            .iter()
            .filter_map(|&h| self.bodies.get(h).cloned())
            .collect();
        if work.len() != handles.len() {
            return;
        }

        for b in work.iter_mut().filter(|b| b.kind == BodyKind::Dynamic) {
            b.linear_velocity = clamp_speed(b.linear_velocity + self.gravity * dt);
        }

        let pairs = broadphase(&work);
        self.contacts.clear();
        let mut constraints: Vec<Constraint> = Vec::new();
        for (i, j) in pairs {
            let (Some(a), Some(b)) = (work.get(i), work.get(j)) else {
                continue;
            };
            let Some(m) = collide(a, b) else { continue };
            if self.contacts.len() >= MAX_CONTACTS {
                break;
            }
            if let (Some(&ha), Some(&hb)) = (handles.get(i), handles.get(j)) {
                self.contacts.push((ha, hb, m));
            }
            for c in m.contacts() {
                let ra = c.point - a.position;
                let rb = c.point - b.position;
                let n = m.normal;
                let t = Vec2::new(-n.y, n.x);
                let k = |dir: Vec2| {
                    let (rna, rnb) = (ra.cross(dir), rb.cross(dir));
                    a.inv_mass + b.inv_mass + a.inv_inertia * rna * rna + b.inv_inertia * rnb * rnb
                };
                let (kn, kt) = (k(n), k(t));
                if !(kn > 0.0 && kt > 0.0) {
                    continue;
                }
                let vrel = (b.linear_velocity + cross_sv(b.angular_velocity, rb))
                    - (a.linear_velocity + cross_sv(a.angular_velocity, ra));
                let vn = vrel.dot(n);
                let e = a.restitution.max(b.restitution);
                constraints.push(Constraint {
                    a: i,
                    b: j,
                    normal: n,
                    ra,
                    rb,
                    mass_n: 1.0 / kn,
                    mass_t: 1.0 / kt,
                    bias: if vn < -BOUNCE_THRESHOLD { -e * vn } else { 0.0 },
                    friction: (a.friction * b.friction).sqrt(),
                    jn: 0.0,
                    jt: 0.0,
                });
            }
        }

        for _ in 0..ITERATIONS {
            for c in constraints.iter_mut() {
                solve(&mut work, c);
            }
        }

        for b in work.iter_mut().filter(|b| b.kind == BodyKind::Dynamic) {
            b.linear_velocity = clamp_speed(b.linear_velocity);
            b.angular_velocity = b.angular_velocity.clamp(-MAX_SPEED, MAX_SPEED);
            b.position = b.position + b.linear_velocity * dt;
            b.angle += b.angular_velocity * dt;
            if !finite_vec(b.position) || !b.angle.is_finite() {
                // Out of range: freeze rather than propagate garbage.
                b.position = Vec2::ZERO;
                b.angle = 0.0;
                b.linear_velocity = Vec2::ZERO;
                b.angular_velocity = 0.0;
            }
        }

        // Push overlapping bodies apart so resting contacts do not sink.
        for (i, j, m) in self.contacts_by_index(&handles) {
            for c in m.contacts() {
                let (Some(a), Some(b)) = (work.get(i), work.get(j)) else {
                    continue;
                };
                let total = a.inv_mass + b.inv_mass;
                if total <= 0.0 {
                    continue;
                }
                let push = m.normal * ((c.depth - SLOP).max(0.0) * CORRECTION / total);
                let (ima, imb) = (a.inv_mass, b.inv_mass);
                if let Some(a) = work.get_mut(i) {
                    a.position = a.position - push * ima;
                }
                if let Some(b) = work.get_mut(j) {
                    b.position = b.position + push * imb;
                }
            }
        }

        for (&h, b) in handles.iter().zip(work) {
            if let Some(slot) = self.bodies.get_mut(h) {
                *slot = b;
            }
        }
    }

    /// The last step's contacts with body indices instead of handles.
    fn contacts_by_index(&self, handles: &[BodyId]) -> Vec<(usize, usize, Manifold)> {
        self.contacts
            .iter()
            .filter_map(|(ha, hb, m)| {
                let i = handles.iter().position(|h| h == ha)?;
                let j = handles.iter().position(|h| h == hb)?;
                Some((i, j, *m))
            })
            .collect()
    }
}

/// Candidate pairs by sweeping along the x axis; two static bodies never pair.
fn broadphase(bodies: &[Body]) -> Vec<(usize, usize)> {
    let mut order: Vec<(usize, Vec2, Vec2)> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let (lo, hi) = b.bounds();
            (i, lo, hi)
        })
        .collect();
    order.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.0.cmp(&b.0)));
    let mut pairs = Vec::new();
    for (n, &(i, lo_i, hi_i)) in order.iter().enumerate() {
        for &(j, lo_j, hi_j) in order.iter().skip(n + 1) {
            if lo_j.x > hi_i.x {
                break;
            }
            if lo_j.y > hi_i.y || hi_j.y < lo_i.y {
                continue;
            }
            let (a, b) = if i < j { (i, j) } else { (j, i) };
            let both_static = matches!(
                (bodies.get(a), bodies.get(b)),
                (Some(x), Some(y)) if x.kind == BodyKind::Static && y.kind == BodyKind::Static
            );
            if !both_static {
                pairs.push((a, b));
            }
        }
    }
    pairs.sort_unstable();
    pairs
}

/// The manifold between two bodies, normal from `a` to `b`.
fn collide(a: &Body, b: &Body) -> Option<Manifold> {
    match (a.shape, b.shape) {
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_circle(a.position, ra, b.position, rb)
        }
        (Shape::Circle { radius }, Shape::Box { .. }) => {
            circle_polygon(a.position, radius, &b.polygon()?)
        }
        (Shape::Box { .. }, Shape::Circle { radius }) => {
            circle_polygon(b.position, radius, &a.polygon()?).map(flip)
        }
        (Shape::Box { .. }, Shape::Box { .. }) => polygon_polygon(&a.polygon()?, &b.polygon()?),
    }
}

fn flip(m: Manifold) -> Manifold {
    Manifold {
        normal: -m.normal,
        ..m
    }
}

fn solve(work: &mut [Body], c: &mut Constraint) {
    let (Some(a), Some(b)) = (work.get(c.a), work.get(c.b)) else {
        return;
    };
    let (mut va, mut wa, mut vb, mut wb) = (
        a.linear_velocity,
        a.angular_velocity,
        b.linear_velocity,
        b.angular_velocity,
    );
    let (ima, iia, imb, iib) = (a.inv_mass, a.inv_inertia, b.inv_mass, b.inv_inertia);
    let n = c.normal;
    let t = Vec2::new(-n.y, n.x);

    let apply = |p: Vec2, va: &mut Vec2, wa: &mut f32, vb: &mut Vec2, wb: &mut f32| {
        *va = *va - p * ima;
        *wa -= iia * c.ra.cross(p);
        *vb = *vb + p * imb;
        *wb += iib * c.rb.cross(p);
    };
    let relative = |va: Vec2, wa: f32, vb: Vec2, wb: f32| {
        (vb + cross_sv(wb, c.rb)) - (va + cross_sv(wa, c.ra))
    };

    let vn = relative(va, wa, vb, wb).dot(n);
    let new_jn = (c.jn + c.mass_n * (-vn + c.bias)).max(0.0);
    let dj = new_jn - c.jn;
    c.jn = new_jn;
    apply(n * dj, &mut va, &mut wa, &mut vb, &mut wb);

    let vt = relative(va, wa, vb, wb).dot(t);
    let max_t = c.friction * c.jn;
    let new_jt = (c.jt + c.mass_t * -vt).clamp(-max_t, max_t);
    let dt = new_jt - c.jt;
    c.jt = new_jt;
    apply(t * dt, &mut va, &mut wa, &mut vb, &mut wb);

    if let Some(a) = work.get_mut(c.a) {
        a.linear_velocity = va;
        a.angular_velocity = wa;
    }
    if let Some(b) = work.get_mut(c.b) {
        b.linear_velocity = vb;
        b.angular_velocity = wb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    fn floor(world: &mut World) -> BodyId {
        world
            .add_body(BodyDef::new(
                BodyKind::Static,
                Shape::Box { half: v(50.0, 1.0) },
                v(0.0, 11.0),
            ))
            .unwrap()
    }

    fn dynamic(world: &mut World, shape: Shape, at: Vec2) -> BodyId {
        world
            .add_body(BodyDef::new(BodyKind::Dynamic, shape, at))
            .unwrap()
    }

    #[test]
    fn free_fall_matches_the_formula() {
        let mut w = World::new(v(0.0, 10.0));
        let b = dynamic(&mut w, Shape::Circle { radius: 0.5 }, v(0.0, 0.0));
        for _ in 0..60 {
            w.step(1.0 / 60.0);
        }
        let body = w.body(b).unwrap();
        // Semi-implicit Euler after one second: y = g * dt^2 * n(n+1)/2.
        assert!((body.position().y - 10.0 * (1.0 / 3600.0) * 1830.0).abs() < 1e-2);
        assert!((body.linear_velocity().y - 10.0).abs() < 1e-3);
    }

    #[test]
    fn a_ball_comes_to_rest_on_the_floor() {
        let mut w = World::new(v(0.0, 10.0));
        floor(&mut w);
        let b = dynamic(&mut w, Shape::Circle { radius: 0.5 }, v(0.0, 2.0));
        for _ in 0..300 {
            w.step(1.0 / 60.0);
        }
        let body = w.body(b).unwrap();
        // The floor's top is at y = 10; the ball rests on it.
        assert!(
            (body.position().y - 9.5).abs() < 0.03,
            "y = {}",
            body.position().y
        );
        assert!(body.linear_velocity().length() < 0.1);
    }

    #[test]
    fn restitution_makes_a_ball_bounce() {
        let mut w = World::new(v(0.0, 10.0));
        floor(&mut w);
        let mut def = BodyDef::new(
            BodyKind::Dynamic,
            Shape::Circle { radius: 0.5 },
            v(0.0, 4.0),
        );
        def.restitution = 1.0;
        let b = w.add_body(def).unwrap();
        let mut highest_after_bounce = f32::INFINITY;
        let mut bounced = false;
        for _ in 0..240 {
            w.step(1.0 / 120.0);
            let body = w.body(b).unwrap();
            if body.linear_velocity().y < -0.5 && body.position().y > 8.0 {
                bounced = true; // moving up near the floor after a hit
            }
            if bounced {
                highest_after_bounce = highest_after_bounce.min(body.position().y);
            }
        }
        assert!(bounced);
        assert!(
            highest_after_bounce < 6.0,
            "rebound only reached {highest_after_bounce}"
        );
    }

    #[test]
    fn a_stack_of_boxes_stays_up() {
        let mut w = World::new(v(0.0, 10.0));
        floor(&mut w);
        let boxes: Vec<BodyId> = (0..4)
            .map(|i| {
                dynamic(
                    &mut w,
                    Shape::Box { half: v(0.5, 0.5) },
                    v(0.0, 9.5 - i as f32 * 1.0),
                )
            })
            .collect();
        for _ in 0..240 {
            w.step(1.0 / 60.0);
        }
        for (i, id) in boxes.iter().enumerate() {
            let p = w.body(*id).unwrap().position();
            assert!(p.x.abs() < 0.1, "box {i} drifted sideways to {}", p.x);
            assert!(
                (p.y - (9.5 - i as f32)).abs() < 0.1,
                "box {i} at y = {}",
                p.y
            );
        }
    }

    #[test]
    fn friction_holds_a_box_on_a_slope() {
        let mut w = World::new(v(0.0, 10.0));
        let mut ground = BodyDef::new(
            BodyKind::Static,
            Shape::Box { half: v(50.0, 1.0) },
            v(0.0, 11.0),
        );
        ground.angle = 0.3;
        ground.friction = 1.0;
        w.add_body(ground).unwrap();
        let mut def = BodyDef::new(
            BodyKind::Dynamic,
            Shape::Box { half: v(0.5, 0.5) },
            v(0.0, 9.3),
        );
        def.angle = 0.3;
        def.friction = 1.0;
        let b = w.add_body(def).unwrap();
        for _ in 0..300 {
            w.step(1.0 / 60.0);
        }
        assert!(w.body(b).unwrap().linear_velocity().length() < 0.3);
    }

    #[test]
    fn fast_bodies_do_not_pass_a_thick_floor() {
        let mut w = World::new(v(0.0, 10.0));
        floor(&mut w);
        let b = dynamic(&mut w, Shape::Box { half: v(0.5, 0.5) }, v(0.0, 2.0));
        assert!(w.set_linear_velocity(b, v(0.0, 40.0)));
        for _ in 0..120 {
            w.step(1.0 / 60.0);
        }
        assert!(w.body(b).unwrap().position().y < 11.0);
    }

    #[test]
    fn stepping_is_deterministic() {
        let build = || {
            let mut w = World::new(v(0.0, 10.0));
            floor(&mut w);
            for i in 0..6 {
                let x = (i % 3) as f32 * 0.8;
                let shape = if i % 2 == 0 {
                    Shape::Box { half: v(0.4, 0.4) }
                } else {
                    Shape::Circle { radius: 0.4 }
                };
                dynamic(&mut w, shape, v(x, i as f32 * -1.1));
            }
            w
        };
        let (mut a, mut b) = (build(), build());
        for _ in 0..300 {
            a.step(1.0 / 60.0);
            b.step(1.0 / 60.0);
        }
        let state = |w: &World| {
            w.bodies
                .iter()
                .map(|(_, b)| (b.position, b.angle))
                .collect::<Vec<_>>()
        };
        assert_eq!(state(&a), state(&b));
    }

    #[test]
    fn hostile_input_is_refused_or_contained() {
        let mut w = World::new(v(f32::NAN, 0.0));
        assert_eq!(w.gravity(), Vec2::ZERO);
        let bad = |f: &dyn Fn(&mut BodyDef)| {
            let mut d = BodyDef::new(
                BodyKind::Dynamic,
                Shape::Circle { radius: 1.0 },
                v(0.0, 0.0),
            );
            f(&mut d);
            d
        };
        for d in [
            bad(&|d| d.position = v(f32::NAN, 0.0)),
            bad(&|d| d.position = v(1e30, 0.0)),
            bad(&|d| d.shape = Shape::Circle { radius: 0.0 }),
            bad(&|d| {
                d.shape = Shape::Box {
                    half: v(1.0, f32::INFINITY),
                }
            }),
            bad(&|d| d.density = -1.0),
            bad(&|d| d.restitution = 2.0),
            bad(&|d| d.friction = f32::NAN),
            bad(&|d| d.linear_velocity = v(f32::INFINITY, 0.0)),
        ] {
            assert_eq!(w.add_body(d), Err(PhysicsError::InvalidBody));
        }
        let id = dynamic(&mut w, Shape::Circle { radius: 1.0 }, v(0.0, 0.0));
        assert!(!w.apply_impulse(id, v(f32::NAN, 0.0)));
        assert!(w.apply_impulse(id, v(1e30, 0.0)));
        w.step(f32::NAN);
        w.step(-1.0);
        w.step(1e9);
        let p = w.body(id).unwrap().position();
        assert!(p.is_finite());
        assert!(w.remove_body(id));
        assert!(!w.remove_body(id));
        assert!(w.body(id).is_none());
        assert!(!w.set_linear_velocity(id, v(0.0, 0.0)));
    }

    #[test]
    fn many_overlapping_bodies_stay_finite() {
        let mut w = World::new(v(0.0, 10.0));
        floor(&mut w);
        for i in 0..200 {
            dynamic(
                &mut w,
                Shape::Box { half: v(0.5, 0.5) },
                v((i % 5) as f32 * 0.01, 9.0),
            );
        }
        for _ in 0..60 {
            w.step(1.0 / 60.0);
        }
        assert!(w
            .bodies
            .iter()
            .all(|(_, b)| b.position.is_finite() && b.angle.is_finite()));
    }
}
