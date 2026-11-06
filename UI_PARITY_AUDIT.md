# Map UI parity audit — 2026-11-06

Scope: the six city/unit map-presentation gaps identified in the 2026-11-05
audit, compared with the local Freeciv native client, civ2civ3 rules and
Freeciv-web 2D client. This does not claim complete Freeciv UI parity.
Reference paths below are relative to the local `reference` checkout.

## Completed requirements and evidence

| Gap | Implementation | Reference and verification |
| --- | --- | --- |
| Hidden garrisons and occupancy | Idle city units stay hidden; occupied cities show the original Amplio2 occupancy marker. Focused units draw above cities. | Native `client/control.c::find_visible_unit`, `client/tilespec.c` City1 and FocusUnit layers; `city-garrison-idle` / `city-garrison-selected` gallery cases. Server `citytools.c::update_dumb_city` defines occupancy as any unit on the city tile. |
| Unit status overlays | Original HP-decile bars, veteran badges, fortifying/fortified/sentry icons, all supported work icons, auto-worker and auto-explore markers. Stack counts sit clear of the activity glyph. | Web `2dcanvas/tilespec.js::fill_unit_sprite_array`; imported `tiles.spec`, `activities.spec`, `veterancy.spec`; semantic sprite-tag and HP-decile tests; `unit-status` gallery case. |
| Walls and disorder | Walled city variants replace the plain sprite; disorder uses the original overlay. The city manager shares this renderer. | Native `tilespec.c` City1, Amplio2 `cities.spec` / `bases.spec`, civ2civ3 Visible_Walls effect; `city-appearance` case. |
| City styles and image bands | A typed resolver reads imported style requirements, City_Image effects and sprite tags. Last matching technology style wins; sizes 5/9/13/17 add image levels. City ID no longer picks artwork. | Imported civ2civ3 `styles.ruleset`, verbatim city-art effect sections and Amplio2 `cities.spec`. Native tests cover all 100 plain/walled style/image combinations and size boundaries. |
| City-bar information | Faction banner, name and size, plus production and growth/famine timing for owned cities. Private foreign stocks remain hidden. Labels clear both sprite bases and the full selection-ring footprint at every supported zoom. | Native `common/city.c::city_turns_to_build` / `city_turns_to_grow`; city-bar arithmetic and label-clearance tests; desktop and compact city gallery captures. |
| Movement, combat and stack priority | Real simulation events feed bounded non-blocking playback. Movements interpolate; battles show actual participants/HP changes over idle garrisons, with original casualty explosion frames. Bombardment records HP loss without inventing deaths. Pre-capture city artwork and territorial ownership persist until battle playback completes. Stack representatives follow focus, owned transport, owned unit, other transport, other unit; carried units are excluded unless focused. | Native `control.c::find_visible_unit`, `mapview_common.c::decrease_unit_hp_smooth`; ordering and clock-driven playback tests; `unit-movement`, `city-combat`, `city-combat-explosion` cases driven by real Move commands. |

The earlier selection fix uses the original four-frame Amplio2 selection ring
at Freeciv-web's 6 Hz. Labels and ring captures are deterministic in the visual
gallery; ordinary gameplay uses the actual animation clock.

## Fidelity boundaries

CivCraft has six fictional factions rather than Freeciv nations. Each faction
has an explicit initial reference city theme and its existing color banner;
there is no nation-selection feature or borrowed unrelated national flag.
The imported rules control technology transitions, image levels and walls.

Animations observe the single-player simulation; they do not introduce a
client/server boundary or defer game rules. Their queue and snapshots are
excluded from saves. Tests compare complete saved campaign bytes and the
full native gameplay RNG with the observer enabled/disabled for real movement
and combat commands. Playback tests also verify that overrides disappear at
completion. Movement defaults to the native client's 30 ms, combat HP steps
to 10 ms, and casualty explosion frames to 20 ms each.

This work addresses the listed map-presentation issues. It does not reproduce
all Freeciv dialogs, nation configuration, map-view options or the native
client's remembered-city/fog database. Gallery fixtures are explicitly staged
and never change normal campaign generation or saves.

## Validation

Native Rust tests cover the rules resolver, health/activity sprite selection,
city-bar arithmetic and zoom clearance, unit representative ordering, playback
completion and observer independence. The gallery covers 28 scenarios at
1280x720 and 800x600, including before/after real movement and combat states.
The desktop self-test separately exercises normal gameplay and HUD controls.
See `UI_REVIEW.md` for reproducible commands and screenshot/report semantics.

Final validation: 520/520 Rust tests; 278/278 desktop gameplay checks; all
28 gallery scenes passed at both sizes (36 desktop captures, 37 compact
captures). Captures were inspected for status-glyph separation, wall/disorder
art, own/foreign bar contents, label/ring clearance, movement, battle actors,
pre/post-capture ownership and the casualty explosion. Formatting and diff
whitespace checks pass.
