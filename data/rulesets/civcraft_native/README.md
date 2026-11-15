# CivCraft native ruleset content

This directory is the CivCraft-owned TOML content layer for the Civilization
III: Conquests campaign.  It is intentionally separate from
`civcraft_civ2civ3/`, whose `.ruleset` files currently provide the retained
Freeciv engine configuration.

The catalogs are currently loaded by migration/parity tests while the runtime
still consumes the legacy TSV adapters.  Do not edit the TSVs directly for a
rules change once the corresponding TOML field exists; regenerate only when
reconciling source data, then keep the TOML and parity tests authoritative.
