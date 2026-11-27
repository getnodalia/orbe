# Orbe: Nodalia migration

What is done and what is left to make Orbe a Nodalia app like the others.

## Done

- Own repository `getnodalia/orbe`, MIT, with its code base renamed to Orbe's names. The notices
  of the original authors stay where the licence requires them.
- Identity: name, binary `orbe`, app id `dev.danielmiguelt.Orbe`, desktop entry, website
  `https://getnodalia.com/orbe`.
- Editor themes: the default dark and light presets are NDL (window frame and island colours, the
  Orbe accent `#DDA10A`, rounder corners).
- `AGENTS.md`, `NOTICE`, release notes format, `ORBE_VERSION`.

## Next

1. **Icon and art.** Orbe's app icon is a portrait of John von Neumann, made with nodalia-design's
   `tools/app-portrait` (see `docs/iconography.md` there): `misc/logo/icon*` and the hicolor set,
   the Windows `.ico`, `Orbe.icns` and the macOS export templates' icons, and the Android launcher
   icons. iOS and visionOS exports take the project's own icon, so the engine ships none. Still
   to do: a Liquid Glass icon for macOS 26 (the editor bundle's `Assets.car` still holds
   upstream's and is not referenced: `CFBundleIconName` is gone, so the `.icns` shows), the
   macOS document icons in `misc/dist/macos*`, a placeholder mark, wordmark and splash
   (`misc/orbe-art`, rows in `ATTRIBUTION.md`), and the Apple embedded splash images.
2. **CI**: done (rebrand check, a Linux editor build with the unit tests, release notes check).
3. **Releases**: `release-auto.yml` has Linux (AppImage, `.deb`, `.rpm`, `.tar.gz`) and macOS
   (unsigned DMG with Orbe's own install window; Metal on Apple silicon, the Compatibility
   renderer on Intel) and Windows (a per-user MSI and a portable zip for x64, x86 and arm64, built
   with MSVC and no Direct3D 12 SDK or ANGLE: Vulkan and OpenGL only; verified by CI alone, no
   Windows machine here). Orbe is in Nodalia Launcher's catalog and has its page at
   `getnodalia.com/orbe`.
4. **NDL window.** Done: the hamburger menu (Editor Settings, Keyboard Shortcuts, About) and, on
   Linux only, the frameless window with NDL's round minimize, maximize and close controls and
   resizing edges (`editor/gui/editor_window_controls.*`; the setting "Use System Window Controls"
   brings the desktop's own back; macOS and Windows always keep native controls). The editor's menus are exported over D-Bus
   (`com.canonical.dbusmenu`, registered with the AppMenu registrar of Nodalix's shell, on Wayland;
   `platform/linuxbsd/freedesktop_global_menu.*`) and the in-window menu bar hides when the top bar
   shows them. Also done in 0.2.0: pill-shaped workspace tabs, a Home button and a command search button in the header, NDL's bold icons for the header, toolbars and docks (`editor/icons`, 40 of them), Orbe's own version in the editor and translated inspector options. Also done in 0.3.0: NDL islands with pill tabs for the docks (`theme_modern.cpp`), 158 node and resource type icons in NDL's bold style with their category colours, Spanish in sentence case and the editor catalog's 255 untranslated texts. Still to do: the remaining icons (cursors, a few in the file tree) and Spanish for the class reference. The Help menu links point to Orbe's repository and page; the
   engine documentation and the asset library still point to upstream's, since Orbe opens the same
   projects.
5. **About.** The credits keep upstream's authors and the third-party licences, as the licence
   requires; no Contributors or Models tabs.
6. **Names.** Done: every mention of upstream's name in the interface, the class reference, the
   translations and the code comments is Orbe's (the rule `Godot` to `Orbe` in
   `.github/rebrand.toml`, checked by `scripts/rebrand.py check`). What keeps upstream's name on
   purpose, for compatibility or because the licence requires it: the copyright lines and
   notices (`LICENSE.txt`, `COPYRIGHT.txt`, `AUTHORS.md`, `DONORS.md`, `NOTICE`, the About window),
   `project.godot`, `.godot/`, `.tscn`/`.tres`/`.res`, the GDExtension interface, the C# API
   namespace (`modules/mono`), the Android and web export templates, class and file names inside
   the code, upstream's version history ("Godot 3 to 4" project converter) and the editor icon
   file names until the art is replaced.
7. **Language.** Spanish is complete; audit Orbe's own strings.
