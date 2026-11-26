# macOS DMG window

What Finder shows when the DMG opens: a 660 × 400 pt background with the app icon and the
`Applications` link side by side. `package.sh` copies these files into the image; nothing here is
generated at build time, so the DMG still builds with `hdiutil makehybrid` (no mounted device, no
Finder scripting on CI).

| File | What |
|---|---|
| `background.svg` | Source of the background, drawn by `tools/dmg-background/draw_background.py` in [nodalia-design](https://github.com/getnodalia/nodalia-design): two Nodalia islands on the window colour, the left one in Orbe's colour with a flat picture of a scene (an orb on its orbit over a floor, with the viewport axes), the right one white under the icons. Its text (Sora) is outlined, so rendering it needs no fonts. |
| `background.tiff` | The background at 1x (660 × 400 px, 72 dpi) and 2x (1320 × 800 px, 144 dpi) in one HiDPI TIFF (16-colour palette, Deflate, sRGB). Goes to `.background/background.tiff`. |
| `dmg-layout.DS_Store` | Finder's view settings for the volume: window size, icon size 128, Orbe.app at (326, 205), `Applications` at (574, 205), and the background. Goes to `.DS_Store` in the image. |
| `generate.py` | Writes `background.tiff` and `dmg-layout.DS_Store` from the SVG and the layout above. |

## The volume name has no version

The mounted volume is called `Orbe`, not `Orbe <version>`. The layout points at the background
through an alias that includes the volume name, and Finder resolves it by that name: with a
versioned name the window keeps its size and icon positions but shows no background. The DMG file
name (`orbe-<version>-macos-<arch>.dmg`) still carries the version. `package.sh` stops the build
if the layout was made for another volume or app name.

## Rules

- **Finder draws the icon labels in black in light and dark mode** when a window has a background,
  so the area under both icons stays light (the white island).
- **Nothing goes inside the icon boxes:** artwork keeps 10 pt clear of each 128 pt icon box and of
  the label strip under it.

## Regenerate

Regenerate `background.svg` with nodalia-design's `tools/dmg-background/draw_background.py orbe`,
then run, on any OS:

```sh
pip install 'pillow>=12' ds_store==1.3.3 mac_alias==2.2.3
python3 packaging/macos/dmg/generate.py   # needs resvg on PATH
```

Both outputs are reproducible and carry nothing from the machine that ran it: the SVG renders
without any fonts, and `dmg-layout.DS_Store` is written from scratch, its background alias
holding only the volume name and `/.background/background.tiff`. The window is 660 × 432 with
Finder's 32 pt title bar; the content area is 660 × 400.
