//! Freeciv engine identifier tables generated from the pinned source enums.
//! Source revision: 9ff184b6eb128ca636c7ad6269e27aa959600006.
//! Sources: gen_headers/enums/*.def and common/fc_types.h, player.h, tech.h, road.h, extras.h, unittype.h, city.c.
//! Regenerate these lists from their enum/identifier definitions when updating Freeciv.

/// Declares a typed engine identifier with its Freeciv spelling. Rule files are
/// resolved to these values once at load so evaluation never compares strings.
macro_rules! identifier_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident, names = $names:ident {
            $($variant:ident => $text:literal,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis enum $name {
            $($variant,)*
        }

        pub(super) const $names: &[&str] = &[$($text,)*];

        impl $name {
            pub(crate) const ALL: &[Self] = &[$(Self::$variant,)*];

            /// The identifier exactly as the Freeciv engine spells it.
            pub(crate) const fn name(self) -> &'static str {
                $names[self as usize]
            }

            /// Freeciv's case-insensitive identifier lookup.
            pub(crate) fn parse(value: &str) -> Option<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|kind| kind.name().eq_ignore_ascii_case(value))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

identifier_enum! {
    /// Freeciv universal (requirement source) types, in engine enum order.
    pub(crate) enum RequirementKind, names = REQUIREMENT_KINDS {
        None => "None",
        Achievement => "Achievement",
        Action => "Action",
        Activity => "Activity",
        Tech => "Tech",
        Age => "Age",
        Ai => "AI",
        CityStatus => "CityStatus",
        CityTile => "CityTile",
        Counter => "Counter",
        DiplRel => "DiplRel",
        DiplRelTile => "DiplRelTile",
        DiplRelTileOther => "DiplRelTileOther",
        DiplRelUnitAny => "DiplRelUnitAny",
        DiplRelUnitAnyOther => "DiplRelUnitAnyOther",
        Extra => "Extra",
        ExtraFlag => "ExtraFlag",
        FormAge => "FormAge",
        FutureTechs => "FutureTechs",
        Good => "Good",
        Gov => "Gov",
        GovFlag => "GovFlag",
        Building => "Building",
        BuildingFlag => "BuildingFlag",
        BuildingGenus => "BuildingGenus",
        MaxLatitude => "MaxLatitude",
        MaxTopUnitsOnTile => "MaxTopUnitsOnTile",
        MaxUnitsOnTile => "MaxUnitsOnTile",
        MinCalFrag => "MinCalFrag",
        MinCities => "MinCities",
        MinCulture => "MinCulture",
        MinForeignPct => "MinForeignPct",
        MinHitPoints => "MinHitPoints",
        MinLatitude => "MinLatitude",
        MinMoveFrags => "MinMoveFrags",
        MinSize => "MinSize",
        MinTechs => "MinTechs",
        MinVeteran => "MinVeteran",
        MinYear => "MinYear",
        Nation => "Nation",
        Nationality => "Nationality",
        NationGroup => "NationGroup",
        OriginalOwner => "OriginalOwner",
        OutputType => "OutputType",
        PlayerFlag => "PlayerFlag",
        PlayerState => "PlayerState",
        RoadFlag => "RoadFlag",
        ServerSetting => "ServerSetting",
        Site => "Site",
        Specialist => "Specialist",
        Style => "Style",
        TechFlag => "TechFlag",
        Terrain => "Terrain",
        TerrainAlter => "TerrainAlter",
        TerrainClass => "TerrainClass",
        TerrainFlag => "TerrainFlag",
        Tiledef => "Tiledef",
        TiledefConnected => "TiledefConnected",
        Topology => "Topology",
        UnitClassFlag => "UnitClassFlag",
        UnitClass => "UnitClass",
        UnitState => "UnitState",
        UnitTypeFlag => "UnitTypeFlag",
        UnitType => "UnitType",
        Wrap => "Wrap",
    }
}

pub(super) const ACTION_NAMES: &[&str] = &[
    "Establish Embassy",
    "Establish Embassy Stay",
    "Investigate City",
    "Investigate City Spend Unit",
    "Poison City",
    "Poison City Escape",
    "Steal Gold",
    "Steal Gold Escape",
    "Sabotage City",
    "Sabotage City Escape",
    "Targeted Sabotage City",
    "Targeted Sabotage City Escape",
    "Sabotage City Production",
    "Sabotage City Production Escape",
    "Steal Tech",
    "Steal Tech Escape Expected",
    "Targeted Steal Tech",
    "Targeted Steal Tech Escape Expected",
    "Incite City",
    "Incite City Escape",
    "Establish Trade Route",
    "Enter Marketplace",
    "Help Wonder",
    "Bribe Unit",
    "Bribe Stack",
    "Capture Units",
    "Sabotage Unit",
    "Sabotage Unit Escape",
    "Found City",
    "Join City",
    "Steal Maps",
    "Steal Maps Escape",
    "Suitcase Nuke",
    "Suitcase Nuke Escape",
    "Explode Nuclear",
    "Nuke City",
    "Nuke Units",
    "Destroy City",
    "Expel Unit",
    "Disband Unit Recover",
    "Disband Unit",
    "Home City",
    "Unit Make Homeless",
    "Upgrade Unit",
    "Convert Unit",
    "Airlift Unit",
    "Attack",
    "Attack 2",
    "Suicide Attack",
    "Suicide Attack 2",
    "Surgical Strike Building",
    "Surgical Strike Production",
    "Conquer City Shrink",
    "Conquer City Shrink 2",
    "Conquer City Shrink 3",
    "Conquer City Shrink 4",
    "Bombard",
    "Bombard 2",
    "Bombard 3",
    "Bombard 4",
    "Bombard Lethal",
    "Bombard Lethal 2",
    "Build Road",
    "Build Road 2",
    "Build Irrigation",
    "Build Irrigation 2",
    "Build Mine",
    "Build Mine 2",
    "Build Base",
    "Build Base 2",
    "Pillage",
    "Pillage 2",
    "Transport Board",
    "Transport Board 2",
    "Transport Board_3",
    "Transport Deboard",
    "Transport Embark",
    "Transport Embark 2",
    "Transport Embark 3",
    "Transport Embark 4",
    "Transport Disembark",
    "Transport Disembark 2",
    "Transport Disembark 3",
    "Transport Disembark 4",
    "Transport Load",
    "Transport Load 2",
    "Transport Load 3",
    "Transport Unload",
    "Spread Plague",
    "Spy Attack",
    "Conquer Extras",
    "Conquer Extras 2",
    "Conquer Extras 3",
    "Conquer Extras 4",
    "Enter Hut",
    "Enter Hut 2",
    "Enter Hut 3",
    "Enter Hut 4",
    "Frighten Hut",
    "Frighten Hut 2",
    "Frighten Hut 3",
    "Frighten Hut 4",
    "Heal Unit",
    "Heal Unit 2",
    "Paradrop Unit",
    "Paradrop Unit Conquer",
    "Paradrop Unit Frighten",
    "Paradrop Unit Frighten Conquer",
    "Paradrop Unit Enter",
    "Paradrop Unit Enter Conquer",
    "Wipe Units",
    "Spy Escape",
    "Unit Move",
    "Unit Move 2",
    "Unit Move 3",
    "Teleport",
    "Teleport2",
    "Teleport3",
    "Teleport Conquer",
    "Teleport Frighten",
    "Teleport Frighten Conquer",
    "Teleport Enter",
    "Teleport Enter Conquer",
    "Clean",
    "Clean 2",
    "Collect Ransom",
    "Fortify",
    "Fortify 2",
    "Cultivate",
    "Cultivate 2",
    "Plant",
    "Plant 2",
    "Transform Terrain",
    "Transform Terrain 2",
    "Scout",
    "Gain Veterancy",
    "Escape",
    "Civil War",
    "Finish Unit",
    "Finish Building",
    "User Action 1",
    "User Action 2",
    "User Action 3",
    "User Action 4",
];

identifier_enum! {
    /// Freeciv engine effect types, in engine enum order.
    pub(crate) enum EffectKind, names = EFFECT_NAMES {
        TechParasite => "Tech_Parasite",
        Airlift => "Airlift",
        AnyGovernment => "Any_Government",
        CapitalCity => "Capital_City",
        EnableNuke => "Enable_Nuke",
        EnableSpace => "Enable_Space",
        SpecialistOutput => "Specialist_Output",
        OutputBonus => "Output_Bonus",
        OutputBonus2 => "Output_Bonus_2",
        OutputAddTile => "Output_Add_Tile",
        OutputIncTile => "Output_Inc_Tile",
        OutputPerTile => "Output_Per_Tile",
        OutputWastePct => "Output_Waste_Pct",
        ForceContent => "Force_Content",
        GiveImmTech => "Give_Imm_Tech",
        GrowthFood => "Growth_Food",
        ShrinkFood => "Shrink_Food",
        HaveEmbassies => "Have_Embassies",
        MakeContent => "Make_Content",
        MakeContentMil => "Make_Content_Mil",
        MakeContentMilPer => "Make_Content_Mil_Per",
        MakeHappy => "Make_Happy",
        NoAnarchy => "No_Anarchy",
        NukeProof => "Nuke_Proof",
        PolluPopPct => "Pollu_Pop_Pct",
        PolluPopPct2 => "Pollu_Pop_Pct_2",
        PolluProdPct => "Pollu_Prod_Pct",
        RevealCities => "Reveal_Cities",
        RevealMap => "Reveal_Map",
        InciteCostPct => "Incite_Cost_Pct",
        SizeAdj => "Size_Adj",
        SizeUnlimit => "Size_Unlimit",
        SsStructural => "SS_Structural",
        SsComponent => "SS_Component",
        SsModule => "SS_Module",
        SpyResistant => "Spy_Resistant",
        MoveBonus => "Move_Bonus",
        UnitNoLosePop => "Unit_No_Lose_Pop",
        UnitRecover => "Unit_Recover",
        UpgradeUnit => "Upgrade_Unit",
        UpkeepFree => "Upkeep_Free",
        TechUpkeepFree => "Tech_Upkeep_Free",
        NoUnhappy => "No_Unhappy",
        VeteranBuild => "Veteran_Build",
        VeteranCombat => "Veteran_Combat",
        VeteranWork => "Veteran_Work",
        HpRegen => "HP_Regen",
        CityVisionRadiusSq => "City_Vision_Radius_Sq",
        UnitVisionRadiusSq => "Unit_Vision_Radius_Sq",
        DefendBonus => "Defend_Bonus",
        TraderoutePct => "Traderoute_Pct",
        GainAiLove => "Gain_AI_Love",
        TurnYears => "Turn_Years",
        SlowDownTimeline => "Slow_Down_Timeline",
        CivilWarChance => "Civil_War_Chance",
        CivilWarCityBonus => "Civil_War_City_Bonus",
        MigrationPct => "Migration_Pct",
        EmpireSizeBase => "Empire_Size_Base",
        EmpireSizeStep => "Empire_Size_Step",
        MaxRates => "Max_Rates",
        MartialLawByUnit => "Martial_Law_By_Unit",
        MartialLawMax => "Martial_Law_Max",
        RaptureGrow => "Rapture_Grow",
        RevolutionUnhappiness => "Revolution_Unhappiness",
        HasSenate => "Has_Senate",
        InspirePartisans => "Inspire_Partisans",
        HappinessToGold => "Happiness_To_Gold",
        Fanatics => "Fanatics",
        NoDiplomacy => "No_Diplomacy",
        TradeRevenueBonus => "Trade_Revenue_Bonus",
        UnhappyFactor => "Unhappy_Factor",
        UpkeepPct => "Upkeep_Pct",
        UnitUpkeepFreePerCity => "Unit_Upkeep_Free_Per_City",
        OutputWaste => "Output_Waste",
        OutputWasteByDistance => "Output_Waste_By_Distance",
        OutputPenaltyTile => "Output_Penalty_Tile",
        OutputIncTileCelebrate => "Output_Inc_Tile_Celebrate",
        CityUnhappySize => "City_Unhappy_Size",
        CityRadiusSq => "City_Radius_Sq",
        CityBuildSlots => "City_Build_Slots",
        UpgradePricePct => "Upgrade_Price_Pct",
        VisibleWalls => "Visible_Walls",
        TechCostPct => "Tech_Cost_Pct",
        Shield2GoldPct => "Shield2Gold_Pct",
        TileWorkable => "Tile_Workable",
        CityImage => "City_Image",
        BuildingBuildCostPct => "Building_Build_Cost_Pct",
        MaxTradeRoutes => "Max_Trade_Routes",
        GovCenter => "Gov_Center",
        CombatRounds => "Combat_Rounds",
        BuildingBuyCostPct => "Building_Buy_Cost_Pct",
        UnitBuildCostPct => "Unit_Build_Cost_Pct",
        UnitBuyCostPct => "Unit_Buy_Cost_Pct",
        NotTechSource => "Not_Tech_Source",
        EnemyCitizenUnhappyPct => "Enemy_Citizen_Unhappy_Pct",
        IrrigationPct => "Irrigation_Pct",
        MiningPct => "Mining_Pct",
        OutputTilePunishPct => "Output_Tile_Punish_Pct",
        UnitBribeCostPct => "Unit_Bribe_Cost_Pct",
        Victory => "Victory",
        Performance => "Performance",
        History => "History",
        NationalPerformance => "National_Performance",
        NationalHistory => "National_History",
        TurnFragments => "Turn_Fragments",
        MaxStolenGoldPm => "Max_Stolen_Gold_Pm",
        ThiefsSharePm => "Thiefs_Share_Pm",
        RetirePct => "Retire_Pct",
        IllegalActionMoveCost => "Illegal_Action_Move_Cost",
        HaveContacts => "Have_Contacts",
        CasusBelliCaught => "Casus_Belli_Caught",
        CasusBelliSuccess => "Casus_Belli_Success",
        ActionOddsPct => "Action_Odds_Pct",
        BorderVision => "Border_Vision",
        StealingsIgnore => "Stealings_Ignore",
        OutputWasteByRelDistance => "Output_Waste_By_Rel_Distance",
        BuildingSaboteurResistant => "Building_Saboteur_Resistant",
        UnitSlots => "Unit_Slots",
        AttackBonus => "Attack_Bonus",
        ConquestTechPct => "Conquest_Tech_Pct",
        ActionSuccessActorMoveCost => "Action_Success_Actor_Move_Cost",
        ActionSuccessTargetMoveCost => "Action_Success_Target_Move_Cost",
        InfraPoints => "Infra_Points",
        FortifyDefenseBonus => "Fortify_Defense_Bonus",
        MapsStolenPct => "Maps_Stolen_Pct",
        UnitShieldValuePct => "Unit_Shield_Value_Pct",
        CasusBelliComplete => "Casus_Belli_Complete",
        IllegalActionHpCost => "Illegal_Action_HP_Cost",
        PopcostFree => "Popcost_Free",
        HealUNitPct => "Heal_UNit_Pct",
        NukeBlastRadius1Sq => "Nuke_Blast_Radius_1_Sq",
        ActionResistPct => "Action_Resist_Pct",
        OutputBonusAbsolute => "Output_Bonus_Absolute",
        MinHpPct => "Min_HP_Pct",
        HpRegen2 => "HP_Regen_2",
        TechParasitePctMax => "Tech_Parasite_Pct_Max",
        HealthPct => "Health_Pct",
        ActivityTime => "Activity_Time",
        SurplusWastePct => "Surplus_Waste_Pct",
        SurplusWastePctByRelDistance => "Surplus_Waste_Pct_By_Rel_Distance",
        TechLeakage => "Tech_Leakage",
        ImprUpkeepReduction => "Impr_Upkeep_Reduction",
        CulturePct => "Culture_Pct",
        ArmisticeLengthPct => "Armistice_Length_Pct",
        CeasefireLengthPct => "Ceasefire_Length_Pct",
        UserEffect1 => "User_Effect_1",
        UserEffect2 => "User_Effect_2",
        UserEffect3 => "User_Effect_3",
        UserEffect4 => "User_Effect_4",
    }
}

pub(super) const CITY_STATUS_NAMES: &[&str] = &[
    "OwnedByOriginal",
    "Starved",
    "Disorder",
    "Celebration",
    "Transferred",
    "CapitalConnected",
];

pub(super) const BUILDING_FLAG_NAMES: &[&str] = &[
    "VisibleByOthers",
    "SaveSmallWonder",
    "Gold",
    "Infra",
    "DisasterProof",
    "Indestructible",
];

pub(super) const GOV_FLAG_NAMES: &[&str] = &[];

pub(super) const TERRAIN_CLASS_NAMES: &[&str] = &["Land", "Oceanic"];

pub(super) const TERRAIN_ALTER_NAMES: &[&str] =
    &["CanBase", "CanIrrigate", "CanMine", "CanPlace", "CanRoad"];

pub(super) const TERRAIN_FLAG_NAMES: &[&str] = &[
    "CanHaveRiver",
    "EnterBorders",
    "FreshWater",
    "Frozen",
    "NotGenerated",
    "NoBarbs",
    "NoCities",
    "NoZoc",
    "Starter",
    "UnsafeCoast",
];

pub(super) const ACTIVITY_NAMES: &[&str] = &[
    "Idle",
    "Cultivate",
    "Mine",
    "Irrigate",
    "Fortified",
    "Pillage",
    "Transform",
    "Fortifying",
    "Clean",
    "Base",
    "Road",
    "Convert",
    "Plant",
];

pub(super) const AI_LEVEL_NAMES: &[&str] = &[
    "Restricted",
    "Novice",
    "Easy",
    "Normal",
    "Hard",
    "Cheating",
    "Experimental",
    "Away",
];

pub(super) const CITY_TILE_NAMES: &[&str] = &[
    "Center",
    "Claimed",
    "Extras Owned",
    "Worked",
    "Same Continent",
    "Bordering TClass Region",
];

pub(super) const PLAYER_STATE_NAMES: &[&str] = &["Barbarian", "HasCapital"];

pub(super) const UNIT_STATE_NAMES: &[&str] = &[
    "Transported",
    "OnLivableTile",
    "Transporting",
    "HasHomeCity",
    "OnNativeTile",
    "InNativeExtra",
    "MovedThisTurn",
];

pub(super) const IMPR_GENUS_NAMES: &[&str] = &[
    "GreatWonder",
    "SmallWonder",
    "Improvement",
    "Special",
    "Convert",
];

pub(super) const PLAYER_FLAG_NAMES: &[&str] = &["ai", "ScenarioReserved", "FirstCity"];

pub(super) const TOPOLOGY_NAMES: &[&str] = &["ISO", "Hex", "WrapX", "WrapY"];

pub(super) const WRAP_NAMES: &[&str] = &["WrapX", "WrapY"];

pub(super) const TECH_FLAG_NAMES: &[&str] = &["Bonus_Tech", "Bridge", "Build_Airborne"];

pub(super) const ROAD_FLAG_NAMES: &[&str] = &["River", "UnrestrictedInfra", "JumpFrom", "JumpTo"];

pub(super) const EXTRA_FLAG_NAMES: &[&str] = &[
    "NativeTile",
    "Refuel",
    "TerrChangeRemoves",
    "AutoOnCityCenter",
    "AlwaysOnCityCenter",
    "ConnectLand",
    "GlobalWarming",
    "NuclearWinter",
    "ShowFlag",
    "NaturalDefense",
    "NoStackDeath",
    "CauseZoC",
];

pub(super) const UNIT_TYPE_FLAG_NAMES: &[&str] = &[
    "HasNoZOC",
    "IgZOC",
    "NonMil",
    "IgTer",
    "FieldUnit",
    "Provoking",
    "NeverProtects",
    "Workers",
    "Diplomat",
    "CoastStrict",
    "Coast",
    "Spy",
    "Only_Native_Attack",
    "Fanatic",
    "GameLoss",
    "Unique",
    "EvacuateFirst",
    "SuperSpy",
    "NoHome",
    "Unreserved",
    "CityBuster",
    "NoBuild",
    "BadWallAttacker",
    "BadCityDefender",
    "BarbarianOnly",
    "NewCityGamesOnly",
    "CanKillEscaping",
    "RandomMovement",
    "Flagless",
    "ProvidesRansom",
];

pub(super) const UNIT_CLASS_FLAG_NAMES: &[&str] = &[
    "TerrainSpeed",
    "TerrainDefense",
    "DamageSlows",
    "CanOccupyCity",
    "BuildAnywhere",
    "Unreachable",
    "CollectRansom",
    "ZOC",
    "DoesntOccupyTile",
    "AttackNonNative",
    "KillCitizen",
    "HutFrighten",
    "NonNatBombardTgt",
];

pub(super) const DIPLO_REL_NAMES: &[&str] = &[
    "Armistice",
    "War",
    "Cease-fire",
    "Peace",
    "Alliance",
    "Never met",
    "Team",
    "Gives shared vision",
    "Receives shared vision",
    "Hosts embassy",
    "Has embassy",
    "Hosts real embassy",
    "Has real embassy",
    "Has Casus Belli",
    "Provided Casus Belli",
    "Foreign",
    "Has team embassy",
    "Hosts team embassy",
];

pub(super) const OUTPUT_TYPE_NAMES: &[&str] =
    &["Food", "Shield", "Trade", "Gold", "Luxury", "Science"];

pub(super) fn contains_case_insensitive(names: &[&str], value: &str) -> bool {
    names.iter().any(|known| known.eq_ignore_ascii_case(value))
}
