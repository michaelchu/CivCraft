# CivCraft native ruleset content

This directory is the CivCraft-owned TOML content layer for the Civilization
III: Conquests campaign.  It is intentionally separate from
`civcraft_civ2civ3/`, whose `.ruleset` files currently provide the retained
Freeciv engine configuration.

The catalogs are the runtime source for the migrated campaign systems.  The
legacy TSV extraction tables remain only as temporary source material for the
conversion tool while building rules and Freeciv effect adapters are being
cut over.  Do not edit the TSVs directly for a rules change once the
corresponding TOML field exists; regenerate only when reconciling source data,
then keep the TOML and focused tests authoritative.

Current catalogs:

- `units.toml`: 92 native unit identities and tuneable source values.
- `civilizations.toml`: 31 civilizations, city lists, starting technologies,
  and starting-unit profiles.
- `buildings.toml`: 83 building records plus explicit legacy binding status.
- `technologies.toml`: 83 advances, source coordinates, costs, and prerequisite
  graph inputs.
- `governments.toml`: the eight Conquests governments and their support,
  assimilation, resistance, and military-law tables.
- `economy.toml`: city growth, output, upkeep, and founding parameters.
- `settlements.toml`: settlement AI and village outcome parameters.
