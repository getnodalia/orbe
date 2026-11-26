# Attribution

Assets Orbe adds. Third-party code is in `thirdparty/` and `COPYRIGHT.txt`; the upstream engine's
authors are credited in `NOTICE`.

| Asset | Author | Source | Licence | Use |
|---|---|---|---|---|
| `misc/logo/{logo,logo_outlined,icon_outlined}.*`, `editor/icons/{Godot,GodotFile,GodotMonochrome,Logo,TitleBarLogo}.svg`, `misc/dist/html/logo.svg`, `main/splash.png` | Orbe contributors | original work, drawn by `misc/orbe-art/generate.py` | MIT | Orbe's placeholder mark, wordmark and splash |
| `editor/icons/{TripleBar,2D,3D,Add,Back,Close,DistractionFree,Duplicate,Edit,ExternalLink,Favorites,File,Folder,Forward,Game,Groups,Help,Load,Lock,MenuBar,MoveDown,MoveUp,Pause,Play,PlayCustom,PlayScene,Redo,Reload,Remove,Rename,Save,Script,Search,Stop,ToolMove,ToolRotate,ToolScale,ToolSelect,Tools,Window,Zoom}.svg` | Nodalia contributors | NDL's bold icons from nodalia-design `packages/icons` (`src/icons.mjs`, `src/bold.mjs`), drawn on a 24 px grid and scaled to 16 px | MIT | The editor's header, toolbars and docks |
| `misc/logo/icon-portrait-1024.png`, `misc/logo/icon.svg`, `misc/logo/icon.png`, `misc/logo/icon-1024.png` | App icon artwork: Johannes Kepler (1571–1630), cut out in black and white on Orbe's gold | Nodalia, from a 17th-century portrait painting (artist unknown) | [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Portrait_of_Johannes_Kepler.jpg); made with nodalia-design `tools/app-portrait` | Portrait: public domain. Artwork: MIT |
| `misc/logo/hicolor/*`, `platform/windows/godot.ico`, `platform/windows/godot_console.ico`, `packaging/macos/Orbe.icns`, `misc/dist/macos_template.app/Contents/Resources/icon.icns`, `misc/dist/macos_tools.app/Contents/Resources/GodotLG.icns`, `platform/android/java/lib/src/main/res/mipmap*/icon*.webp` | App icon at each size: the Colloid application grid for Linux, a full-bleed tile for Windows, Apple's icon grid for macOS, Android launcher icons (legacy, and adaptive with the figure in the background layer and a silhouette as the themed layer) | Orbe contributors | made from the artwork above with nodalia-design `tools/app-portrait/icon-set.py` | MIT |
| `packaging/macos/dmg/background.{svg,tiff}` | Orbe contributors | original work, drawn by nodalia-design's `tools/dmg-background/draw_background.py` | MIT | the window of the macOS DMG |
| The word "Orbe" in those files, as outlines | Sora Project Authors | [Sora](https://github.com/sora-xor/sora-font) (`Sora[wght].ttf` from nodalia-design) | SIL OFL 1.1 | the wordmark |
| `examples/lago-sereno/` | Orbe contributors | original work: a procedural valley, lake and dusk sky built from noise and two shaders, no imported art | MIT | the example project that ships with Orbe |

Still upstream's and to be replaced before the first release: the macOS document icons in
`misc/dist/macos*` (the packaging in `packaging/macos` does not use them), the Android and Apple embedded splash images and icons.
