// SPDX-License-Identifier: MIT
//! The GPU backend must agree with the CPU rasteriser. The tests skip when the machine has no
//! graphics adapter.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use orbe_core::Variant;
use orbe_gpu::{Gpu, GpuError};
use orbe_math::{Color, Vec2};
use orbe_render::{build_draw_list, render, DrawList, Image};
use orbe_scene::SceneTree;

fn gpu() -> Option<Gpu> {
    match Gpu::new() {
        Ok(g) => {
            eprintln!("using adapter: {}", g.adapter_name());
            Some(g)
        }
        Err(GpuError::NoAdapter | GpuError::Device(_)) => {
            eprintln!("skipped: no usable graphics adapter");
            None
        }
        Err(e) => panic!("unexpected error: {e}"),
    }
}

fn scene() -> SceneTree {
    let mut t = SceneTree::new("Root", "Node2D").unwrap();
    let root = t.root();
    let a = t.add_child(root, "Red", "ColorRect").unwrap();
    t.set_property(a, "position", Variant::Vec2(Vec2::new(4.0, 4.0)))
        .unwrap();
    t.set_property(a, "size", Variant::Vec2(Vec2::new(16.0, 12.0)))
        .unwrap();
    t.set_property(a, "color", Variant::Color(Color::new(1.0, 0.0, 0.0, 1.0)))
        .unwrap();
    let b = t.add_child(root, "GlassGreen", "ColorRect").unwrap();
    t.set_property(b, "position", Variant::Vec2(Vec2::new(12.0, 8.0)))
        .unwrap();
    t.set_property(b, "size", Variant::Vec2(Vec2::new(16.0, 16.0)))
        .unwrap();
    t.set_property(b, "color", Variant::Color(Color::new(0.0, 1.0, 0.0, 0.5)))
        .unwrap();
    let p = t.add_child(root, "Tri", "Polygon2D").unwrap();
    let flat: Vec<Variant> = [2.0, 28.0, 14.0, 28.0, 2.0, 16.0]
        .iter()
        .map(|f| Variant::Float(*f))
        .collect();
    t.set_property(p, "polygon", Variant::Array(flat)).unwrap();
    t.set_property(p, "color", Variant::Color(Color::new(0.2, 0.4, 1.0, 1.0)))
        .unwrap();
    t
}

/// Largest per-channel difference over pixels whose neighbourhood is flat (edges differ by the
/// anti-aliasing pattern, which is not specified to match).
fn max_interior_difference(a: &Image, b: &Image) -> u8 {
    let (w, h) = (a.width(), a.height());
    let mut worst = 0;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let flat = |img: &Image| {
                let c = img.pixel(x, y).unwrap();
                [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .all(|(dx, dy)| {
                        img.pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32) == Some(c)
                    })
            };
            if !flat(a) || !flat(b) {
                continue;
            }
            let (pa, pb) = (a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap());
            for i in 0..4 {
                worst = worst.max(pa[i].abs_diff(pb[i]));
            }
        }
    }
    worst
}

#[test]
fn matches_the_cpu_rasteriser() {
    let Some(gpu) = gpu() else { return };
    let list = build_draw_list(&scene());
    let clear = Color::new(0.05, 0.05, 0.08, 1.0);
    let cpu = render(&list, 32, 32, clear).unwrap();
    let on_gpu = gpu.render(&list, 32, 32, clear).unwrap();
    assert_eq!((on_gpu.width(), on_gpu.height()), (32, 32));
    // Sanity: the picture is not blank.
    assert_eq!(on_gpu.pixel(6, 6), cpu.pixel(6, 6));
    assert_eq!(on_gpu.pixel(0, 0), cpu.pixel(0, 0));
    let worst = max_interior_difference(&cpu, &on_gpu);
    assert!(worst <= 2, "interior pixels differ by {worst}");
    // Edges: a handful of pixels may differ, but not many.
    let differing = cpu
        .pixels()
        .chunks(4)
        .zip(on_gpu.pixels().chunks(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 24))
        .count();
    assert!(differing < 80, "{differing} pixels differ noticeably");
}

#[test]
fn empty_lists_and_transparent_backgrounds() {
    let Some(gpu) = gpu() else { return };
    let empty = DrawList::default();
    let img = gpu
        .render(&empty, 4, 4, Color::new(0.0, 0.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(img.pixel(1, 1), Some([0, 0, 0, 0]));
    let img = gpu
        .render(&empty, 3, 5, Color::new(1.0, 1.0, 1.0, 1.0))
        .unwrap();
    assert_eq!(img.pixel(2, 4), Some([255, 255, 255, 255]));
    // A width that is not a multiple of the copy alignment still reads back correctly.
    assert_eq!(img.pixels().len(), 3 * 5 * 4);
}

#[test]
fn refuses_bad_sizes() {
    let Some(gpu) = gpu() else { return };
    let l = DrawList::default();
    assert!(gpu.render(&l, 0, 4, Color::default()).is_err());
    assert!(gpu.render(&l, 100_000, 4, Color::default()).is_err());
}
