// SPDX-License-Identifier: MIT
//! Orbe's 2D rigid body physics.
//!
//! A [`World`] holds circles and boxes that are either static or dynamic. [`World::step`] moves
//! them with semi-implicit Euler integration, finds collisions (circle and convex polygon
//! tests with a separating-axis search and edge clipping) and resolves them with a sequential
//! impulse solver with friction and restitution. Stepping is deterministic: the same inputs give
//! the same bodies, bit for bit, on the same platform.
//!
//! The world has no unit system and no preferred "up": pick units and a gravity vector that fit
//! the scene. Every number that enters the world is checked; bodies with non-finite or absurd
//! values are refused, and the simulation never panics.

mod collide;
mod world;

pub use collide::{Contact, Manifold};
pub use world::{
    Body, BodyDef, BodyId, BodyKind, PhysicsError, Shape, World, MAX_BODIES, MAX_CONTACTS,
    MAX_SPEED, MAX_STEP,
};
