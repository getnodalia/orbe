# Orbe: Nodalia migration

What is done and what is left to make Orbe a Nodalia app like the others.

## Done

- Own repository `getnodalia/orbe`, MIT, from upstream 4.7.2 renamed (`docs/upstream.md`).
- Identity: name, binary `orbe`, app id `dev.danielmiguelt.Orbe`, desktop entry, website
  `https://getnodalia.com/orbe`.
- Editor themes: the default dark and light presets are NDL (window frame and island colours, the
  Orbe accent `#DDA10A`, rounder corners).
- `AGENTS.md`, `NOTICE`, release notes format, `ORBE_VERSION`.

## Next

1. **Icon and art.** A placeholder mark, wordmark and splash are in (`misc/orbe-art`, rows in
   `ATTRIBUTION.md`). Orbe's app icon is a portrait of a historical figure like every Nodalia
   app (see `docs/iconography.md` in nodalia-design); the macOS, Android and Apple embedded icons
   are still upstream's and must be replaced before the first release.
2. **CI** (own pull request): rebrand check, a Linux editor build with the unit tests, release
   notes check, weekly upstream sync.
3. **Releases**: `release-auto.yml` with Linux (AppImage, `.deb`, `.rpm`, `.tar.gz`), macOS
   (unsigned DMG with Orbe's own install window) and Windows (MSI and portable) packages, then the
   Launcher catalog entry and the page `getnodalia.com/orbe`.
4. **NDL window.** Round NDL window controls on Linux only (native on macOS and Windows), the
   hamburger menu (Editor settings, Keyboard shortcuts, About), the menu exported over D-Bus to the
   shell's global menu on Linux, NDL islands for the docks, NDL icons for the editor chrome.
5. **About.** The credits keep upstream's authors and the third-party licences, as the licence
   requires; no Contributors or Models tabs.
6. **Names left.** Prose that still says "Godot" in the class reference, the translations and the
   editor's strings (only the engine's full product name is renamed so far) moves over with rules in
   `.github/rebrand.toml`, checked with `scripts/rebrand.py check`. The names kept for
   compatibility are in `AGENTS.md`.
7. **Language.** Spanish is complete upstream; audit Orbe's own strings.
