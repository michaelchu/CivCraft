use super::*;

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
