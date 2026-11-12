# Civ III asset retrieval compared with OpenCiv3

## Scope and sources

This is a source-level comparison of the checked-out `openciv3/` tree and CivCraft's current working tree. It maps how each program chooses Civilization III installation files. It does not claim that every OpenCiv3 path is correct or that its behavior is the target: the parity goal is to make CivCraft's installation lookup and precedence match OpenCiv3 where the same asset is used, while retaining documented CivCraft-only tileset choices.

OpenCiv3's shared resolver is `openciv3/C7/Util.cs::Civ3MediaPath`; installation discovery is in `openciv3/QueryCiv3/Civ3Location.cs`; root selection is exposed in `openciv3/C7/UIElements/MainMenu/MainMenu.cs`. CivCraft's source root and renderer lookup live in `app/src/client/tileset.rs`; per-unit art identities and source folders come from `app/data/rulesets/civ3_conquests/units.tsv`.

## Installation root discovery

| Behavior | OpenCiv3 | CivCraft |
| --- | --- | --- |
| Explicit configuration | `C7.ini` setting `[locations] civ3InstallDir`; UI lets the user choose and saves the folder. | `CIVCRAFT_SOURCE_DIR` environment variable. |
| Automatic discovery | `CIV3_HOME`, then Windows registry (`install_path`), then Steam `steamapps/common` scan for a folder containing `civ3id.mb`. | None. |
| Root validation | Discovery returns a candidate; startup checks for recognizable classic media (`buttonsFINAL.pcx`) across Conquests, PTW, and base roots. | Requires exactly cased `Conquests/Art/Terrain` to be a directory. Other expected folders/files fail later when loaded. |
| Case handling | Resolves path components ignoring case on non-Windows platforms, retaining actual filesystem spelling; Windows uses normal file lookup. | Root and most direct paths use exact spelling. Unit animation references and generic theme references have component-wise case-insensitive fallback. |

The CivCraft variable name suggests a source checkout, and fallback-root setup additionally tests that the installation directory's basename is literally `source`. It therefore behaves as expected for this workspace's `source/` layout but not for a conventionally named Steam/GOG installation root. The root should be treated as an arbitrary install directory, not rely on its basename.

## File lookup and precedence

OpenCiv3's `Civ3MediaPath(relativePath)` is a shared resolver for graphics, animation INIs/FLCs, sounds, and text media. Its effective order is:

1. Active scenario/mod path entries (semicolon-separated), with direct, `Conquests/Conquests/`, `Conquests/Scenarios/`, and `civ3PTW/Scenarios/` variants.
2. Packaged OpenCiv3 `Assets/`. In standalone mode this is the only fallback; outside standalone it is checked after mods and before original game media.
3. Installation roots in this order: `Conquests/`, `civ3PTW/`, then the installation root itself.
4. Packaged OpenCiv3 media as a final fallback in normal mode.

Each candidate is joined with the requested relative path. The resolver normalizes separators on non-Windows systems, handles `..`, and retries path components case-insensitively. Missing media throws with the requested relative path.

CivCraft has no shared equivalent resolver today. Its effective behavior is split by asset type:

| Asset family | CivCraft lookup today | Parity gap / observation |
| --- | --- | --- |
| Default terrain and map overlays | `civ3`/`conquests` selects `Conquests/Art`; unoverridden images search that root. If the selected directory is under a directory named exactly `source/Conquests/Art`, image fallback adds `source/civ3PTW/Art` then `source/Art`. Missing optional images may be skipped; required images error. | Does not discover installation root automatically or resolve general image paths case-insensitively. The fallback roots are accidentally coupled to the parent directory name `source`. |
| Snoopy terrain | Selects one of three scenario art folders under `civ3PTW/Scenarios/`, rooted at the configured install directory. | This is a CivCraft-specific explicit tileset selection; it is not OpenCiv3's general scenario-mod precedence. |
| Unit animations and sounds | Unit roles/folders are mapped from the checked-in Conquests catalog. INI selection is driven by each row's explicit folder and root labels (`base`, `ptw`, `conquests`); relative FLC/sound references then search the INI's folder, corresponding folders in fallback roots, and broad configured art roots. FLC references and sounds use case-insensitive component resolution. Missing sound entries are skipped; missing referenced animations fail. | Unit source-root paths use exact spelling and the checked-in table is authoritative. No active scenario override is searched. Precedence is encoded by the TSV's selected root/variant rather than OpenCiv3's per-file mod → Conquests → PTW → base fallback. |
| Cursor | Reads `Art/Animations/Cursor/Cursor.flc` directly from the installation root. | Base-game-only and exact-case; no Conquests/PTW fallback or case-insensitive lookup. |
| Fog atlas | Reads `Art/Terrain/FogOfWar.pcx` directly from the installation root. | Base-game-only and exact-case. This is a deliberate source atlas choice in the current loader, but differs from OpenCiv3's generic resolver policy. |
| Unit movement indicator | First asks the selected theme, then retries from default `Conquests/Art`. | Does not retry PTW or base Art if the Conquests file is absent. |
| City sheets | Theme paths/defaults search selected theme plus the conditional PTW/base fallback roots; required sheets error when absent. | Fallback again depends on directory basename `source`; no generic install-root precedence. |
| Music and UI sound | Campaign music is loaded from `CIVCRAFT_SOURCE_DIR/Sounds` by `client/audio.rs`; it selects MP3 tracks by era/culture and decodes them asynchronously. There is no general configured UI/music/ambience asset loader. | OpenCiv3's shared resolver serves WAV/MP3/OGG requests for configured audio. CivCraft already supports campaign music, but does not yet have the same generic asset-backed audio interface. |
| Civilopedia, PediaIcons, palette/UI and other media | Not currently loaded from the Civ III installation by CivCraft's asset module. | OpenCiv3 uses `Civ3MediaPath` in game/scenario setup, texture loaders, animation, palace config, and audio paths. |

OpenCiv3 additionally supports scenario media overrides when a scenario's mod path has been set. CivCraft's Snoopy tilesets are explicit terrain roots, but its unit, cursor, fog, and other asset loaders do not apply the active scenario's mod path.

## Reconciliation map

To make retrieval match for shared assets, consolidate CivCraft's installation path logic behind one resolver with these semantics:

1. Resolve and retain one install root. Accept `CIVCRAFT_SOURCE_DIR` as an explicit override; add OpenCiv3-compatible `CIV3_HOME`/platform discovery or clearly report that explicit configuration is still required. Do not make behavior depend on the root folder's basename.
2. Probe candidates in a single declared precedence order: active mod/scenario roots when CivCraft has that concept; `Conquests`, `civ3PTW`, base install root; then any CivCraft packaged fallback. Preserve the current explicit tileset as an intentional override where requested.
3. Resolve every path component case-insensitively on case-sensitive filesystems, normalize Windows separators, and return the actual canonical path. Use the resolver for terrain, cities, unit INIs/FLCs/sounds, cursor, fog, and movement indicators rather than mixing it with direct joins.
4. Keep asset-family policies visible: required files should fail with the full candidate list; optional files may remain optional; source-specific choices such as the fog atlas should be documented as explicit exceptions if OpenCiv3 resolves the same asset differently.
5. Compare unit folder/root choices in `units.tsv` against OpenCiv3's INI-relative fallback for every campaign identity. Where OpenCiv3 finds a shared animation in another edition's corresponding unit folder, CivCraft should resolve the same file before broader roots. The current `resolve_unit_file` already implements much of this cross-edition per-unit fallback, but only for the roots it can derive and after the selected INI has been found.

## Port design: shared media resolver and generic audio loader

The port should introduce one installation-aware media resolver and make both textures/animations and audio consume its resolved paths. It should replace duplicated asset-specific root selection while keeping content-specific interpretation (tileset roles, unit INIs, music era selection, and `.amb` cue parsing) in their existing layers.

### 1. Resolver contract

Add a client-side `Civ3MediaResolver` (for example in `client/media.rs`) constructed once from an `AssetRoots` configuration. It should expose a narrow interface:

```rust,ignore
enum MediaClass { Game, Scenario, Packaged }
struct ResolvedMedia { path: PathBuf, class: MediaClass }

fn resolve(&self, relative: &str) -> Result<ResolvedMedia, MediaError>;
fn resolve_optional(&self, relative: &str) -> Option<ResolvedMedia>;
```

`resolve` accepts a relative Civ III path, normalizes slash styles, supports scenario-style `..` references within the installation root, looks up each path component case-insensitively on case-sensitive systems, and returns the actual on-disk path. Reject absolute paths and traversal that escapes every configured root; for INI references that intentionally use `..`, resolve them relative to the owning file first and then apply the established roots. Errors should include the requested name and roots tried. `resolve_optional` should be the explicit missing-media path for genuinely optional content rather than broadly swallowing read/decode errors.

Configure precedence once and document it. To mirror OpenCiv3's effective normal-mode order, use active scenario/mod candidates first, then packaged CivCraft media, then `Conquests`, `civ3PTW`, and base installation roots. OpenCiv3 checks the same packaged path again after installation lookup, but the earlier check already makes it win whenever present, so CivCraft need not reproduce that redundant check. Standalone mode should use packaged media only. Keep explicit tileset selection as an override for terrain assets; do not let a generic campaign lookup silently replace the selected Snoopy theme.

The resolver configuration should accept the existing `CIVCRAFT_SOURCE_DIR` override, stop relying on the directory basename being `source`, and own one canonical root for every asset subsystem. Discovery from `CIV3_HOME`/Steam can be added behind the same configuration layer; it is separate from lookup correctness and should not be required to land the resolver. Scenario roots should be supplied as ordered paths by the scenario loader when CivCraft gains or already has a selected scenario; do not infer them from arbitrary filenames.

### 2. Generic audio catalog and loader

Add `client/audio_assets.rs` (or extend `audio.rs`) with a data-driven catalog mapping stable keys to media names, playback category, and looping policy. Example keys are `menu.main_menu_1`, `buttons.button_1`, and `ambience.oriole`, matching the useful OpenCiv3 calling pattern. Keep this catalog in a small checked-in TOML file initially; allow scenario/mod overrides to replace entries by key, while the resolver handles the actual candidate path order. Do not put absolute installation paths in the catalog.

Suggested API:

```rust,ignore
enum AudioBus { Music, Sfx, Ui, Ambience }
struct AudioCue { key: String, path: String, bus: AudioBus, looped: bool }

struct AudioAssetLoader { resolver: Civ3MediaResolver, decoded: HashMap<PathBuf, Sound> }
async fn load(&mut self, key: &str) -> Result<Option<Sound>, AudioLoadError>;
async fn load_path(&mut self, path: &str, looped: bool) -> Result<Option<Sound>, AudioLoadError>;
```

Use Macroquad `Sound` for playback and cache by canonical resolved path plus decode options where options affect the result. Support WAV, MP3, OGG, and OGA, as OpenCiv3 does. CivCraft already has Symphonia with MP3 decoding in `audio.rs`; retain that decoding path and add/use the appropriate OGG codec feature if Macroquad's byte loader does not decode OGG. WAV can use Macroquad's WAV loader or a small validated RIFF decoder. Decoder failure should return a contextual error; missing optional cues may return `None`. Apply `looped` from cue metadata (music normally loops; UI/SFX generally do not) instead of hardcoding looping in format decoders.

The loader should not own volume or channel policy. Route decoded sounds to existing Macroquad playback channels with per-bus volume/mute state. CivCraft already has music mute/volume and SFX mute/volume UI controls, plus campaign cross-fading in `MusicPlayer`; wire the generic loader into those controls rather than replacing those interactions. Add UI and ambience bus settings only when those categories are used.

### 3. Integration points

- `Assets` should retain the resolver (or a cloneable handle) so `tileset.rs` can request source files through it. Keep image decode, palette handling, and atlas validation in `tileset.rs`.
- Unit animation INI lookup should use the resolver for INI discovery and its owning-file-relative variant for FLC/sound references. Preserve the current same-unit cross-edition fallback before global roots. `.amb` remains a Civ III sound-cue container: load/parse it as today, then resolve each listed WAV through the same media resolver, retaining same-folder and same-unit preference.
- `MusicPlayer` should ask the catalog for a key/path and use the shared resolver rather than constructing `root/Sounds`. Keep its era/culture track selection, asynchronous MP3 decoding, intro-clip concatenation, and cross-fade state machine. This makes existing music participate in scenario/packaged precedence without changing when the game changes tracks.
- UI clicks, ambience, and future configured sounds should use the same key-based loader; `PlaySfxAudioByPath`'s unit event use can become `load_path` after its INI reference is resolved.
- Cursor, fog, and movement-indicator loading should also call the resolver, with explicit policy where the selected source asset must stay base-game-specific.

### 4. Incremental delivery and acceptance conditions

1. Introduce and locally exercise the resolver against the current `source/` tree and at least one differently named synthetic install root; verify exact-case and mixed-case path components, missing-file diagnostics, Conquests/PTW/base precedence, and safe `..` resolution.
2. Move existing terrain, cursor, fog, city, and unit source lookups to the resolver without changing selected art or unit role mapping.
3. Move current music and unit sounds to resolver-returned paths; confirm the same campaign tracks and unit cues are selected before adding new generic cue entries.
4. Add key-based audio configuration and support WAV/MP3/OGG/OGA, cache decoded data, and keep sound category loop policies explicit. Missing optional sounds should not stop game startup; malformed required media should report the key, file, and decode error.
5. Add scenario and packaged overrides only with explicit roots/configuration. Verify that an override wins over campaign content and that standalone mode does not fall through to installed Civ III media.

These are implementation acceptance conditions, not claims that the feature is already present. They call for focused resolver/audio verification when implementation is requested; they do not imply replacing CivCraft's current campaign music selection or sound timing.

## Prioritized differences

| Priority | Difference | Why it matters |
| --- | --- | --- |
| P1 | No automatic install discovery and a different configuration surface. | Users must configure CivCraft even where OpenCiv3 discovers the same Steam or Windows install. |
| P1 | Exact-case direct paths and the `source` basename condition. | Common packaged installations may fail on Linux/macOS or skip PTW/base fallback despite containing the assets. |
| P1 | No general mod/scenario asset precedence. | Scenario-specific media can be shadowed by campaign art or fail to load. |
| P2 | Cursor/fog and movement-indicator paths bypass the common fallback behavior. | Files available in one of the other supported edition roots are treated as missing. |
| P2 | Music lookup bypasses the shared candidate order; generic key-based UI/ambience audio is absent. | Campaign tracks currently resolve only below the configured root's `Sounds/` folder, so scenario/package overrides and the other install roots cannot participate. |

This comparison is source-based. It was not runtime-validated against multiple Steam/GOG/Windows installations; OpenCiv3's checked-out revision and the current CivCraft working tree can also change independently.
