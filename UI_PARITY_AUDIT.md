# Map UI parity audit — 2026-11-05

Scope: local Freeciv native and Freeciv-web 2D city/unit rendering. This is
an inspection of presentation code, not a claim of full UI or gameplay parity.
Reference paths below are relative to the local `reference` checkout.

## Addressed in this change

- Hide unselected units inside cities (`freeciv/client/control.c`,
  `find_visible_unit`; web `javascript/control.js`, same function).
- Render focused units in a final layer over cities and other map objects
  (native `LAYER_FOCUS_UNIT`). Keep city descriptions clear of sprite bases.
- Replace the filled yellow selection diamond with original Amplio2
  `unit.select:0..3` artwork. Web `get_select_sprite` cycles at 6 Hz;
  gallery captures freeze frame zero so animation cannot masquerade as a
  successful UI action in pixel-difference checks.
- Focused units take precedence over transport status when choosing a stack
  representative. Add paired idle/selected city gallery scenes.

## Remaining gaps, in suggested order

1. **Garrison occupancy indicator.** The reference keeps an occupied-city
   marker visible when units are hidden. Our city renderer has no occupancy
   marker, so hidden garrisons are no longer discoverable from the map alone.
   Reference: `freeciv/data/amplio2/cities.spec`, `bases.spec` (`cd.occupied`),
   native `tilespec.c` City1 handling and web `mapview_put_city_bar`.
2. **Unit health, veteran and activity overlays.** Reference
   `fill_unit_sprite_array` adds HP, veteran, activity and agent sprites.
   Our `world.rs::draw_unit` shows an owner dot and stack count; health,
   work, rank and state are shown only in the selected-unit HUD. The model
   already exposes these fields, making this a presentation gap.
3. **Walls and civil-disorder city artwork.** Reference `get_city_sprite`
   chooses wall variants, and City1 adds `city.disorder`. Our city sprite
   selection ignores walls and disorder; disorder appears only in city UI.
4. **City style and size selection.** Our preindustrial appearance varies
   by city ID/owner; later artwork uses hardcoded technology and size bands.
   Reference uses style definitions and City_Image/Visible_Walls effects;
   the web also uses different size bands. These choices need a rules-driven
   resolver rather than more hardcoded sprite coordinates.
5. **City bar information.** Reference city bars include occupancy, nation
   flag and production, with native options for growth and production detail.
   Our map bar contains only name, size and an owner-colored strip. Hidden
   garrison occupancy is the most useful first addition from this group.
6. **Combat/movement presentation and stack choice.** Native
   `find_visible_unit` prioritizes attacker, defender, focus, then owned
   transports/units and other transports/units. Our representative selection
   handles focus and carried status, but has no attacker/defender animation
   state or the full ownership/transporter ordering. Movement/combat animation
   and this selection policy should be reviewed together.

## Verification

Use `--ui-review DIR --ui-scene city-garrison-idle` and
`--ui-review DIR --ui-scene city-garrison-selected` for the same city and six
units with only unit focus changed. These are explicitly staged visual QA
scenes and do not modify normal campaign generation or saves.
