# Civ III media resolver and audio loader

## Status and provenance

Implemented on 2026-11-17. This document was recovered from `800be5b^`, where
it described an OpenCiv3-inspired resolver and audio-loader proposal. It now
describes the implementation. The historical comparison can still be retrieved
from that revision. Source inspection originally referenced OpenCiv3's
`C7/Util.cs::Civ3MediaPath`, `QueryCiv3/Civ3Location.cs`, texture loaders and
animation manager. OpenCiv3 is a design reference for media lookup, not gameplay
authority; Freeciv remains the gameplay authority.

## Module boundaries

Desktop startup constructs one `AssetContext`. It reads root configuration and
trace settings once, retains an `Arc<Civ3MediaResolver>`, and, when audio is
enabled, constructs one shared `Arc<AudioLibrary>`. Tileset selection, terrain
fallbacks, fog, music and unit players receive this context or its resolver;
they do not reconstruct configuration from the environment. Catalog overrides
are a startup snapshot; changing files requires a new context or restart.

`media.rs` supplies generic filesystem resolution and root filtering. Civ III
unit catalog edition selection, owning-INI searches and the broad `.amb` WAV
fallback live in `unit_media.rs`. `tileset.rs` interprets graphics and animation
metadata. `audio_codecs.rs` contains pure format decoding and WAV assembly with
no dependency on playback players, catalogs or filesystem lookup.
`audio_assets.rs` supplies catalog lookup and sound caching; `audio.rs` and
`unit_sound.rs` own playback policy and timing. Caches remain per loader; shared
configuration does not imply a global sound cache.

Asset entry points consistently return `AssetResult<T>` with `AssetError` from
`asset_error.rs`. Missing requests carry searched roots; I/O failures preserve
the underlying error and path; configuration and decode errors retain their
origin; contextual wrapping preserves the error chain. Callers can inspect
variants and causes instead of parsing text. Low-level private codec routines
may use textual format reasons, which are classified at decoder boundaries.

Built-in audio defaults are Rust mappings in `audio_assets.rs`. There is no
required audio TOML file or startup parsing for those defaults. External
scenario/package overrides remain optional and are read at startup.

## Configuration and precedence

The usual setup remains:

```sh
CIVCRAFT_SOURCE_DIR=/path/to/CivilizationIII cargo run --release --features desktop-audio
```

Any installation directory name works. `CIV3_HOME` is accepted when
`CIVCRAFT_SOURCE_DIR` is absent. Steam/registry discovery is not required;
without an explicit installation or standalone package configuration, startup
reports how to configure one.

For scenarios or packaged replacements, set `CIVCRAFT_MEDIA_CONFIG` to a TOML
file:

```toml
install = "/games/CivilizationIII"
scenarios = ["mods/first", "mods/second"]
packaged = ["media"]
standalone = false
```

Paths in the file are relative to that file's directory unless absolute.
`CIVCRAFT_SOURCE_DIR`, then `CIV3_HOME`, override `install`. Unknown configuration
fields are rejected. Scenario/package roots contain logical paths such as
`Art/Terrain/xtgc.pcx` and `Sounds/build/ancient/AncGRfull.mp3`; callers supply
ordered roots explicitly rather than guessing scenario locations.

Normal lookup precedence is:

1. Scenario roots, in configured order.
2. Packaged roots, in configured order.
3. Installation `Conquests/`.
4. Installation `civ3PTW/`.
5. Installation root (vanilla Civ III).

This follows the useful OpenCiv3 normal-mode order without repeating its final
packaged lookup. Explicitly selected terrain themes (including Snoopy) override
terrain lookup. They do not change campaign unit identities or music selection.

For packaged-only operation:

```toml
packaged = ["media"]
standalone = true
```

Standalone ignores scenario and installation roots, even if an installation
environment variable is set. The package must supply the required art and all
92 catalog unit identities; standalone does not synthesize replacement assets.

## Resolver contract

`src/client/media.rs` owns root configuration and shared path resolution.
`ResolvedMedia` reports the actual canonical path, selected root and source
class. Set `CIVCRAFT_MEDIA_TRACE=1` to print that provenance as files resolve.

Resolution normalizes Windows separators and matches every path component
case-insensitively. Exact spelling wins; ambiguous case-only matches error.
Relative `..` references are supported for shared unit animations. Canonical
containment checks reject references and symlinks escaping all configured roots.
Absolute names are rejected as logical requests; already-resolved internal
filenames are checked against configured boundaries before reuse.

Optional lookup returns `None` only when a file is absent. Invalid paths,
ambiguous names, filesystem errors, and decode failures remain errors. Required
missing-media errors identify the requested name and searched roots.

Themes retain resolver instances for images and unit references. PNG requests
also accept PCX, respecting root precedence before format fallback. Decoding,
palette transparency and atlas size checks remain in `tileset.rs`.

Unit INIs use catalog folder basenames, with case-insensitive spelling, rather
than the first directory entry ending in `.ini`. Scenario/package overrides
precede the catalog's explicit edition mapping. FLC and sound references search
the owning INI directory, corresponding same-unit folders across configured
roots, then broader art/media roots. `.amb` components use the same resolver;
the existing broad WAV fallback is deterministic, respects root precedence,
and avoids symlink traversal outside configured roots.

Fog and cursor deliberately retain base-game policy for installed assets;
scenario/package replacements remain eligible. Movement indicators, city
sheets, resources, overlays and routes use the theme resolver.

## Audio catalog, decoding and playback

Rust defaults in `audio_assets.rs` map stable campaign keys to relative media
names, playback bus, loop policy and optional/required policy. Each scenario/package
root may supply `audio.toml` with the same `[cues."key"]` structure. Entries merge
by key; higher-precedence roots win. Example:

```toml
[cues."campaign.ancient.GR"]
path = "Sounds/custom/greek.ogg"
bus = "music"
looped = true
required = false

[cues."buttons.confirm"]
path = "Sounds/custom/confirm.wav"
bus = "ui"
looped = false
required = true
```

Supported buses are `music`, `sfx`, `ui`, and `ambience`. Generic key loading
returns the sound and its bus/loop metadata; players own volume and channel
policy. No new UI/ambience controls are needed until those categories are used.

`src/client/audio_assets.rs` supplies key/path loading and caches decoded
Macroquad sounds by canonical path. Unit sounds use this loader. WAV validation,
MP3 decoding through the existing Symphonia path, and OGG/OGA Vorbis decoding
through Lewton live in `audio_codecs.rs` and feed WAV data to Macroquad. Malformed present audio errors even
when a cue is optional; missing optional cues do not stop startup.

Campaign music uses the same catalog/resolver and format decoder while keeping
its era/culture fallback selection, asynchronous preparation, ordered Ancient
intro clips, cross-fade state machine, and existing mute/volume controls. Full
tracks honor catalog loop metadata. Unit action scheduling and `.amb` cue timing
remain in the unit player.

## Validation and limits

Automated fixtures cover installation precedence, arbitrary root names,
scenario/package precedence and provenance, standalone isolation, config-relative
paths, path errors and symlink containment, shared unit references, all 92 unit
mappings, image case handling, catalog overrides, optional/required cues, malformed
audio and actual synthetic MP3/OGG/OGA decoding. The generated tones in
`tests/fixtures/audio` are independent of proprietary game assets.

Use the repository's nextest/all-target and documentation test commands when
available. Cargo's test runner is the fallback when nextest is absent. Audio
builds on Linux require the ALSA development linker library.

A complete Civ III installation and the intended OpenCiv3 revision are still
needed to compare actual selected files and verify playback/visual parity.
Neither is available in this environment. No visual baseline is substituted or
updated. These external acceptance checks remain unverified; implementation and
synthetic regression coverage do not establish visual parity.

Validation recorded for this change:

- `cargo test --offline --all-targets --features desktop-audio`: 1,106 passed,
  one ignored. Linux audio linking used a temporary linker alias to the installed
  ALSA runtime because the development alias was absent.
- Final client regressions: 92 passed, including the additional `.amb` fallback
  test; final provenance regressions: five passed after diagnostic refinement.
- `cargo clippy --offline --all-targets -- -D warnings` and the equivalent
  audio-enabled command passed. The vendored miniquad dependency still emits its
  pre-existing mutable-static warning.
- `cargo test --offline --doc`, formatting, and diff whitespace checks passed.
- Nextest is unavailable here, so Cargo's test runner was used. Installed-asset
  playback and `--ui-review` were not run because the complete installation is
  absent.

Modularity-refactor validation: the audio-enabled all-target suite passed 1,133
tests with one ignored; the final focused client run passed 95 tests after the
additional intro-assembly regression. Clippy passed with and without desktop
audio, and documentation, formatting and whitespace checks passed. These checks
used Cargo's test runner because nextest is unavailable. Installed-asset visual
and playback checks remain unavailable without a complete Civ III installation.
