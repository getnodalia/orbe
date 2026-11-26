# Orbe

Orbe is Nodalia's open-source game engine and editor. Build 2D and 3D games from scenes and
nodes, script them, run them on the desktop and export them, from one editor drawn in the
[Nodalia Design Language](https://github.com/getnodalia/nodalia-design).

Orbe is part of [Nodalia](https://getnodalia.com), the suite of open-source apps made to feel like
one. It is free software under the MIT licence.

> **Status: early.** This is the first version of Orbe's own repository: the engine and the
> editor build and run on Linux, and the editor opens in the NDL light and dark themes with
> Orbe's accent. What is not done yet (Orbe's own icon, NDL window controls and hamburger menu,
> macOS and Windows packages) is listed in [`NODALIA_MIGRATION.md`](NODALIA_MIGRATION.md).

## What it is

- A scene editor: a tree of nodes, an inspector, a 2D and a 3D viewport, an animation editor, a
  script editor and a debugger.
- A rendering engine with a modern Vulkan renderer and a compatibility OpenGL one, a physics
  engine, audio, navigation and a scripting language.
- Projects made for the upstream engine this code base comes from keep opening: Orbe reads the same
  project, scene and resource files. See [`AGENTS.md`](AGENTS.md) for what is kept for that.

## Build

```sh
git clone https://github.com/getnodalia/orbe && cd orbe
scons platform=linuxbsd target=editor accesskit=no -j"$(nproc)"
bin/orbe.linuxbsd.editor.x86_64
```

You need Python 3, SCons 4 and the development packages of X11 or Wayland, ALSA or PulseAudio,
udev and OpenGL. `AGENTS.md` lists the test commands.

## Credits and licence

Orbe is MIT licensed (`LICENSE.txt`). Its code base began as an upstream open-source engine; those
authors are credited in `NOTICE`, `LICENSE.txt`, `COPYRIGHT.txt`, `AUTHORS.md` and the About
window, together with the third-party libraries in `thirdparty/`.
