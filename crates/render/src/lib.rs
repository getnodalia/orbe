// SPDX-License-Identifier: MIT
//! Orbe's 2D renderer.
//!
//! Rendering has two steps, so that every backend draws exactly the same thing:
//!
//! 1. [`build_draw_list`] walks a [`orbe_scene::SceneTree`] and produces a [`DrawList`]: shapes in
//!    painting order with their world transform, colour and camera already resolved.
//! 2. A backend draws the list. [`render`] is the CPU reference rasteriser; it is the oracle the
//!    GPU backend is tested against and it works without a window or a graphics card.
//!
//! Nothing here panics on hostile scenes: non-finite numbers, huge sizes and malformed property
//! values are skipped or refused with a [`RenderError`].
//!
//! Colours are linear; [`Image`] holds 8-bit sRGB-encoded RGBA, as a display wants it.
//!
//! Scene properties read: `position`, `rotation` (radians), `scale`, `z_index` (relative to the
//! parent), `visible` and `modulate` on every node (they inherit down the tree); `size` and
//! `color` on `ColorRect`; `polygon` and `color` on `Polygon2D`; `position` and `zoom` on
//! `Camera2D`. Textures are not drawn yet.

mod draw;
mod raster;

pub use draw::{build_draw_list, Camera, DrawItem, DrawList, Shape, MAX_POLYGON_POINTS};
pub use raster::{render, Image, RenderError, MAX_DIMENSION, MAX_PIXELS};
