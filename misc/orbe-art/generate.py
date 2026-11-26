#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Draws Orbe's placeholder mark and wordmark as SVG, from nothing but a circle and the outlines of
the word "Orbe" in Sora (SIL OFL 1.1, Nodalia's shared font). The mark is original work and flat:
no gradients, no glow. It stands in until Orbe's app icon, a portrait of a historical figure like
every Nodalia app's, is made (see NODALIA_MIGRATION.md).

    python3 misc/orbe-art/generate.py /path/to/Sora[wght].ttf

Writes misc/logo/*.svg and the editor's icons in editor/icons/. PNGs, .ico and the splash are
rendered from these by misc/orbe-art/render.cjs.
"""
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[2]
GOLD, GOLD_DARK, GOLD_LIGHT, INK = "#DDA10A", "#B07F00", "#F5CC5A", "#15171E"


def wordmark(font_path: str, text: str, height: float, weight: int = 650):
    """Outlines of `text` as one SVG path: returns (path_d, width) for a cap height of `height`."""
    font = instantiateVariableFont(TTFont(font_path), {"wght": weight})
    glyphs, cmap = font.getGlyphSet(), font.getBestCmap()
    cap = font["OS/2"].sCapHeight
    scale = height / cap
    x = 0.0
    parts = []
    for ch in text:
        name = cmap[ord(ch)]
        pen = SVGPathPen(glyphs)
        glyphs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, x, height)))
        parts.append(pen.getCommands())
        x += glyphs[name].width * scale
    return "".join(parts), x


def orb(cx: float, cy: float, r: float, ink: bool = True) -> str:
    """The orb: a sphere, its shadowed side and a highlight, all flat shapes."""
    clip = f"o{int(cx)}{int(cy)}{int(r)}"
    return (
        f'<clipPath id="{clip}"><circle cx="{cx}" cy="{cy}" r="{r}"/></clipPath>'
        f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{GOLD}"/>'
        f'<circle cx="{cx + r * 0.42}" cy="{cy + r * 0.42}" r="{r * 0.95}" fill="{GOLD_DARK}" clip-path="url(#{clip})"/>'
        f'<circle cx="{cx - r * 0.12}" cy="{cy - r * 0.12}" r="{r * 0.9}" fill="{GOLD}" clip-path="url(#{clip})"/>'
        f'<ellipse cx="{cx - r * 0.38}" cy="{cy - r * 0.42}" rx="{r * 0.2}" ry="{r * 0.12}" fill="{GOLD_LIGHT}" '
        f'transform="rotate(-35 {cx - r * 0.38} {cy - r * 0.42})"/>'
    )


def write(path: Path, svg: str) -> None:
    path.write_text(svg + "\n", encoding="utf-8")
    print("wrote", path.relative_to(ROOT))


def main() -> None:
    font = sys.argv[1]
    logo = ROOT / "misc" / "logo"
    tile = lambda inner, outline="": (  # noqa: E731
        f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">'
        f'<rect width="128" height="128" rx="28" fill="{INK}"{outline}/>{inner}</svg>'
    )
    mark = orb(64, 64, 38)
    write(logo / "icon.svg", tile(mark))
    write(logo / "icon_outlined.svg", tile(mark, f' stroke="#fff" stroke-width="4"'))

    d, w = wordmark(font, "Orbe", 52)
    gap = 28
    total = 128 + gap + w
    def lockup(colour: str, outline: str = "") -> str:
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{round(total)}" height="128" viewBox="0 0 {round(total)} 128">'
            f'<rect width="128" height="128" rx="28" fill="{INK}"{outline}/>{mark}'
            f'<path fill="{colour}" transform="translate({128 + gap} {64 - 26})" d="{d}"/></svg>'
        )
    write(logo / "logo.svg", lockup("#E4E6EB"))
    write(logo / "logo_outlined.svg", lockup("#E4E6EB", ' stroke="#fff" stroke-width="4"'))
    write(ROOT / "misc" / "dist" / "html" / "logo.svg", lockup("#E4E6EB"))

    icons = ROOT / "editor" / "icons"
    write(icons / "Godot.svg", f'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">{orb(8, 8, 7)}</svg>')
    write(icons / "GodotFile.svg", f'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">{orb(8, 8, 7)}</svg>')
    write(icons / "GodotMonochrome.svg", f'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="7" fill="#fff"/></svg>')
    d2, w2 = wordmark(font, "Orbe", 40)
    write(icons / "Logo.svg", f'<svg xmlns="http://www.w3.org/2000/svg" width="{round(w2)}" height="40"><path fill="#fff" d="{d2}"/></svg>')
    d3, w3 = wordmark(font, "Orbe", 14)
    write(icons / "TitleBarLogo.svg", f'<svg xmlns="http://www.w3.org/2000/svg" width="{round(w3)}" height="14"><path fill="#e0e0e0" d="{d3}"/></svg>')


if __name__ == "__main__":
    main()
