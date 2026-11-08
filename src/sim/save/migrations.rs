//! Save-format upgrades for older campaign data.

use super::*;

pub(super) fn migrate_research_sentinels(sim: &mut Simulation) {
    // Before the Civ III advances were appended, catalog length 87 represented
    // Future Tech and 88 represented the unset research goal. Existing real
    // technology IDs remain unchanged.
    for player in &mut sim.players {
        if player.researching == Tech(87) {
            player.researching = Tech::FUTURE;
        }
        if player.research_goal == Tech(88) {
            player.research_goal = Tech::UNSET;
        }
        let mut known = Vec::with_capacity(player.known.len());
        for tech in player.known.iter().copied() {
            let tech = tech.civ3_equivalent().unwrap_or(tech);
            if !known.contains(&tech) {
                known.push(tech);
            }
        }
        player.known = known;
        player.researching = player
            .researching
            .civ3_equivalent()
            .unwrap_or(player.researching);
        player.research_goal = player
            .research_goal
            .civ3_equivalent()
            .unwrap_or(player.research_goal);
        if player.research_goal.data().is_some_and(|_| !player.research_goal.is_civ3_advance())
        {
            player.research_goal = Tech::UNSET;
        }
    }
    for owner in 0..sim.players.len() {
        let researching = sim.players[owner].researching;
        if researching != Tech::FUTURE
            && (!researching.is_civ3_advance() || sim.has_tech(owner, researching))
        {
            sim.players[owner].researching = sim.research_target(owner);
        }
    }
}

pub(super) fn apply_extras11(sim: &mut Simulation, extras: Extras11) {
    let (current, guard_targets, unit_guards, city_rapture, goto_tiles) = extras;
    sim.climate_current = current;
    sim.ai_guard_targets = guard_targets;
    sim.ai_unit_guards = unit_guards;
    sim.city_rapture = city_rapture;
    sim.unit_goto_tiles = goto_tiles;
}

pub(super) fn decode_v11(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, (current, guard_targets, unit_guards, city_rapture, goto_tiles)): (
        CampaignPayload<Strategy9>,
        Extras11,
    ) = codec().deserialize(payload)?;
    apply_extras11(
        &mut campaign.0,
        (
            current,
            guard_targets,
            unit_guards,
            city_rapture,
            goto_tiles,
        ),
    );
    Ok(campaign)
}

pub(super) fn decode_v12(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, (current, guard_targets, unit_guards, city_rapture, goto_tiles), order): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
    ) = codec().deserialize(payload)?;
    apply_extras11(
        &mut campaign.0,
        (
            current,
            guard_targets,
            unit_guards,
            city_rapture,
            goto_tiles,
        ),
    );
    campaign.0.list_order = order;
    Ok(campaign)
}

pub(super) fn decode_v13(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
    ) = codec().deserialize(payload)?;
    apply_extras11(
        &mut campaign.0,
        (
            current,
            guard_targets,
            unit_guards,
            city_rapture,
            goto_tiles,
        ),
    );
    campaign.0.list_order = order;
    campaign.0.native_random = random;
    campaign.0.cargo_order = cargo;
    Ok(campaign)
}

pub(super) fn decode_v14(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
        observed,
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
        observed_map::ObservedMap,
    ) = codec().deserialize(payload)?;
    apply_extras11(
        &mut campaign.0,
        (
            current,
            guard_targets,
            unit_guards,
            city_rapture,
            goto_tiles,
        ),
    );
    campaign.0.list_order = order;
    campaign.0.native_random = random;
    campaign.0.cargo_order = cargo;
    campaign.0.observed_map = observed;
    Ok(campaign)
}

pub(super) fn decode_v15(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
        observed,
        wrap,
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
        observed_map::ObservedMap,
        MapWrap,
    ) = codec().deserialize(payload)?;
    apply_extras11(
        &mut campaign.0,
        (
            current,
            guard_targets,
            unit_guards,
            city_rapture,
            goto_tiles,
        ),
    );
    campaign.0.list_order = order;
    campaign.0.native_random = random;
    campaign.0.cargo_order = cargo;
    campaign.0.observed_map = observed;
    campaign.0.map_wrap = wrap;
    Ok(campaign)
}

pub(super) fn decode_v10(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, current): (CampaignPayload<Strategy9>, [i32; 2]) =
        codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    Ok(campaign)
}

pub(super) fn upgrade_strategy(payload: CampaignPayload<Strategy8>) -> CampaignPayload<Strategy9> {
    let (game, routes, reserves, missions, founders, observations, strategy) = payload;
    (
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (strategy, Default::default()),
    )
}

pub(super) fn decode_v7(payload: &[u8]) -> Result<CampaignPayload<Strategy8>, bincode::Error> {
    // Development V7 files existed with three, then four, strategy maps.
    // Accept the expanded development layout too; V8 fixes the schema going forward.
    if let Ok(current) = codec().deserialize::<CampaignPayload<Strategy8>>(payload) {
        return Ok(current);
    }
    if let Ok((
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (war, warnings, bulbs, ground),
    )) = codec().deserialize::<CampaignPayload<LegacyStrategy7>>(payload)
    {
        return Ok((
            game,
            routes,
            reserves,
            missions,
            founders,
            observations,
            (
                war,
                warnings,
                bulbs,
                ground,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        ));
    }
    let (game, routes, reserves, missions, founders, observations, (war, warnings, bulbs)) =
        codec().deserialize::<CampaignPayload<LegacyStrategy7Early>>(payload)?;
    Ok((
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (
            war,
            warnings,
            bulbs,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ),
    ))
}
