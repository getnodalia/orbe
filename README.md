<div align="center">
  <a href="https://getnodalia.com">
    <img src="https://raw.githubusercontent.com/getnodalia/nodalia-web/main/assets/brand/nodalia-wordmark-badge.svg" width="150" alt="Nodalia">
  </a>
  <br><br>
  <img src="misc/logo/hicolor/512x512/apps/dev.danielmiguelt.Orbe.png" width="192" alt="Orbe">
  <h1>Orbe</h1>
  <p><strong>The game engine and editor of the Nodalia suite.</strong></p>

  <p>
    <img src="https://img.shields.io/badge/status-early%20stage-orange" alt="Early stage">
    <img src="https://img.shields.io/badge/C%2B%2B-native-00599c?logo=cplusplus&logoColor=white" alt="Native C++">
    <img src="https://img.shields.io/badge/license-MIT-2ea44f" alt="MIT">
  </p>

  <p>
    <a href="#overview">Overview</a> ·
    <a href="#screenshots">Screenshots</a> ·
    <a href="#what-it-does">What it does</a> ·
    <a href="#download">Download</a> ·
    <a href="#build-and-run">Build</a> ·
    <a href="#project-status">Status</a> ·
    <a href="#the-nodalia-suite">Suite</a> ·
    <a href="#license-and-credits">License</a>
  </p>
</div>

<p align="center">
  <img src="https://raw.githubusercontent.com/getnodalia/nodalia-web/main/assets/screens/orbe/orbe-editor-dark.jpg" width="100%" alt="Orbe's editor in the dark theme with a 3D landscape scene open: the scene tree and file system on the left, the viewport in the middle and the inspector on the right">
</p>

---

## Overview

**Orbe** is the open-source game engine and editor of [Nodalia](https://getnodalia.com). You build
2D and 3D games from scenes and nodes, script them, run them on the desktop and export them, from
one editor drawn in the [Nodalia Design Language](https://github.com/getnodalia/nodalia-design):
rounded islands, pill tabs, the suite's bold icons with Orbe's amber accent, and the suite's own
window controls on Linux. It has no account and no cloud.

> [!IMPORTANT]
> **Orbe is at an early stage.** Expect rough edges, and keep your projects under version control.
> The installers are not signed.

## Screenshots

<table>
<tr>
<td width="50%" valign="top"><img src="https://raw.githubusercontent.com/getnodalia/nodalia-web/main/assets/screens/orbe/orbe-editor-light.jpg" alt="The editor in the light theme" width="100%"><p align="center"><sub><b>Light and dark</b>, following the system or your choice in the editor settings.</sub></p></td>
<td width="50%" valign="top"><img src="https://raw.githubusercontent.com/getnodalia/nodalia-web/main/assets/screens/orbe/orbe-scene.jpg" alt="The landscape scene running as a game: mountains over a calm lake at dusk" width="100%"><p align="center"><sub><b>The scene</b>, as the game shows it.</sub></p></td>
</tr>
</table>

## What it does

- **A scene editor.** A tree of nodes, an inspector, 2D and 3D viewports, an animation editor, a
  script editor and a debugger.
- **A rendering engine** with a modern Vulkan renderer and a compatibility OpenGL one, plus a
  physics engine, audio, navigation and a scripting language.
- **Run and export.** Run your game from the editor and export it, with a Home screen that lists your projects.
- **Open projects.** Existing scenes, resources (`.tscn`, `.tres`, `.res`) and GDExtension add-ons
  keep working; [`AGENTS.md`](AGENTS.md) lists the names kept for that.
- **Nodalia Design.** Light and dark themes, a hamburger menu with Editor Settings, Keyboard
  Shortcuts and About, round window controls on Linux (native ones on macOS and Windows) and the
  menus exported over D-Bus to the top bar on Linux.
- **Spanish and English.** The editor speaks both, with the Spanish in sentence case.

## Download

Packages for Linux, macOS and Windows are on the
[releases page](https://github.com/getnodalia/orbe/releases/latest) and at
[getnodalia.com/orbe](https://getnodalia.com/orbe).

| Platform | Files |
|---|---|
| Linux (x86_64) | `.AppImage`, `.deb`, `.rpm` and `.tar.gz` |
| macOS (universal) | `.dmg`, unsigned: open it with right click, Open |
| Windows (x64, x86 and arm64) | a per-user `.msi` (Vulkan and OpenGL) and a `-portable.zip` for each, unsigned |

Checksums are in `SHA256SUMS`, `SHA256SUMS-macos` and `SHA256SUMS-windows-<arch>`.

## Build and run

You need Python 3, SCons 4 and the development packages of X11 or Wayland, ALSA or PulseAudio, udev
and OpenGL.

```sh
git clone https://github.com/getnodalia/orbe && cd orbe
scons platform=linuxbsd target=editor accesskit=no -j"$(nproc)"
bin/orbe.linuxbsd.editor.x86_64
```

[`AGENTS.md`](AGENTS.md) lists the test commands.

## Project status

| Area | State |
|---|---|
| Engine and editor | Working on Linux, macOS and Windows; Windows verified by CI alone |
| App icon | Done: portrait of Johannes Kepler |
| Nodalia interface | Done: NDL themes, islands, pill tabs, bold icons, hamburger menu, Nodalia window controls on Linux |
| Global menu | Done on Linux (D-Bus), in the top bar of the shell |
| Installable packages | Linux, macOS (DMG) and Windows, built by CI for every release |
| Remaining icons and Spanish for the class reference | In progress, tracked in [`NODALIA_MIGRATION.md`](NODALIA_MIGRATION.md) |

## App icon

Nodalia app icons are black-and-white portraits of historical figures over the app colour.
Orbe's is Johannes Kepler, who described the motion of the planets. Provenance and licence are in
[`ATTRIBUTION.md`](ATTRIBUTION.md).

## The Nodalia suite

Orbe is one of the applications of [Nodalia](https://getnodalia.com), an independent software
project developed in public by [Daniel Miguel Tejedor](https://danielmigueltejedor.com). It belongs
to the Creator suite, with Lienzo, Pluma, Toma and the other apps for making things.

## License and credits

Orbe is free software under the MIT licence ([`LICENSE.txt`](LICENSE.txt)). The notices of everyone
whose code is in Orbe, and of the third-party libraries in `thirdparty/`, are kept in
[`NOTICE`](NOTICE), [`COPYRIGHT.txt`](COPYRIGHT.txt) and [`AUTHORS.md`](AUTHORS.md), and in the
editor's About window.
