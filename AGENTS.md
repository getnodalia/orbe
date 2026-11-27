# Orbe: instructions for agents

Engineering rules for every Nodalia repository: [getnodalia/foundation](https://github.com/getnodalia/foundation)
([engineering](https://github.com/getnodalia/foundation/blob/main/standards/engineering.md),
[CI](https://github.com/getnodalia/foundation/blob/main/standards/ci.md),
[security](https://github.com/getnodalia/foundation/blob/main/standards/security.md),
[dependencies and licences](https://github.com/getnodalia/foundation/blob/main/standards/dependencies.md),
[release](https://github.com/getnodalia/foundation/blob/main/standards/release.md),
[NDL](https://github.com/getnodalia/foundation/blob/main/standards/ndl.md)).
`CLAUDE.md` imports this file; edit this one only.

## What this is

Orbe is Nodalia's open-source game engine and editor: 2D and 3D, scenes and nodes, a scripting
language, physics, rendering and export, with an editor drawn in the Nodalia Design Language
(NDL). It is written in C++ and built with SCons. Licence: **MIT** (`LICENSE.txt`,
`COPYRIGHT.txt`). The product name is **Orbe** in prose and in the interface. Machine names are
lowercase `orbe`: the binary (`orbe.<platform>.<target>.<arch>`, from `short_name` in
`version.py`) and the settings folder. The app id is `dev.danielmiguelt.Orbe`.

Its code base began as an upstream open-source engine; those authors are credited in
`LICENSE.txt`, `COPYRIGHT.txt`, `AUTHORS.md`, `NOTICE` and the About window, which must stay as
they are, with every copyright notice in the source files. Never describe Orbe as a fork or a port
in the README, the website, the interface or the release notes.

## Compatibility with the upstream engine

Projects, scenes, resources, add-ons and extensions made for the upstream engine keep working, so
these keep upstream's names on purpose: the project file `project.godot` and the `.godot/`
cache folder, the scene and resource formats (`.tscn`, `.tres`, `.res`), the GDExtension
interface (`godot_*` symbols, `.gdextension`), GDScript, the `GODOT_*` build macros and the
Android and web export templates' package names. `.github/rebrand.toml` lists what is renamed and
what is kept. A line that has to keep an upstream name for compatibility says `rebrand: keep`.

## Names

`scripts/rebrand.py check` (the **Upstream names** check) fails when a name from the original
engine comes back by hand; `.github/rebrand.toml` lists what is renamed and what is kept. Orbe is
its own project: nothing is merged in from another repository, and there is no sync. Never edit
`thirdparty/` except through its update scripts.

## Build and test

```sh
scons platform=linuxbsd target=editor accesskit=no -j$(nproc)          # bin/orbe.linuxbsd.editor.x86_64
scons platform=linuxbsd target=editor tests=yes accesskit=no -j$(nproc)
bin/orbe.linuxbsd.editor.x86_64 --headless --test                       # the unit tests
bin/orbe.linuxbsd.editor.x86_64 --headless --doctool .                  # class reference check
```

Use `scu_build=yes optimize=none debug_symbols=no` while iterating. Formatting is
`.clang-format` and `pre-commit` as upstream's `CONTRIBUTING.md` describes. Look at UI changes:
run the editor (under `xvfb-run` with `mesa-vulkan-drivers` on a machine with no display) and take a
screenshot in light and dark.

## Rules

- **Never crash on input.** Scenes, resources, imported assets, scripts, network packets and
  command-line arguments are hostile: validate, return errors (`ERR_FAIL_*`), never `abort()`.
  A crash fix lands with a test in `tests/` that failed before it.
- **Tests are the gate.** New behaviour comes with doctest cases in `tests/`; keep the suite green.
- **Licences and assets.** Dependencies stay under permissive licences compatible with MIT and
  inside Foundation's allowlist; each third-party component in `thirdparty/` keeps its notice and its
  row in `COPYRIGHT.txt`. Every asset Orbe adds (icon, logo, splash, sound, font) gets a row in
  `ATTRIBUTION.md` in the same change. Never commit font files from outside `thirdparty/`; fonts are
  Nodalia's shared ones. The upstream engine's logo, icon and splash are not Orbe's: never
  bring them back.
- **Nodalia Design.** The editor's default themes are the NDL ones (`editor/themes`): colours come
  from the `--ndl-*` values in `nodalia-design`, the accent is `#DDA10A`, no decorative gradients or
  glows. NDL's round window controls are drawn on Linux only unless the person picks the desktop's
  own; macOS and Windows keep native controls. The hamburger menu holds Preferences ("Editor
  settings"), Keyboard shortcuts and About; the light and dark choice lives only in the settings.
  What is done and what is left is in `NODALIA_MIGRATION.md`.
- **Language.** The interface is translated: Spanish is complete, and English strings go through
  `TTR`/`TTRC`/`RTR` as upstream does. Nothing user-visible stays untranslated.
- **Own libraries.** Nothing depends on another organisation's repository except `thirdparty/`
  code vendored under its licence.
- **Version.** Orbe's own version is in `ORBE_VERSION`; the engine version in `version.py` follows
  upstream and is never edited by hand (compatibility checks read it).
- Commits: conventional commits (`feat:`, `fix:`, `docs:`, `chore:`). Keep secrets and personal
  details out of the repository, logs and requests.

## CI and releases

- Workflows run on standard GitHub-hosted runners only, actions pinned to a full commit SHA,
  least permissions. Keep CI changes in their own pull request unless a change cannot work without
  them.
- `main` changes only through pull requests. A pull request merges once its required checks pass.
- Releases are automatic. A pull request that sets `ORBE_VERSION` to a release version and adds
  `docs/release-notes/vX.Y.Z.md` (format in that folder's README; the **Release notes are written**
  check runs `scripts/check-release-notes.sh`) releases it when it merges: `release-auto.yml` tags
  the merge through Foundation's `release-tag`, the platform workflows build and attach the
  packages, and Foundation's `release-publish` publishes the release. Nobody pushes a tag by hand.
  Features bump the minor (0.1.0 → 0.2.0); a fix-only release bumps the patch. `main` carries a
  `-dev` version between releases.
