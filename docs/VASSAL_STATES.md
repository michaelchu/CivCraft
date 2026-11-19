# Vassal states

The owner authorized this Civ IV-inspired extension on 2026-11-19. Other
mechanics retain their existing Freeciv authority. Colonies, resource tribute,
Civ IV happiness bonuses and domination scoring are outside this first release.

## Agreements

An independent AI nation can submit to a human or AI master after either side
knows Feudalism. Capitulation requires an existing war; voluntary submission
requires peace. The vassal retains ownership of its cities, units, treasury and
research. It cannot have its own vassals. Cycles, multiple masters, human
subjects, barbarians and No_Diplomacy players are rejected.

Submission is an exclusive treaty term, with no bundled purchases or gifts.
Both proposal and acceptance use the same AI evaluation. AI conquerors consider
enemy capitulation before routine ceasefires; threatened AI nations can also
initiate submission to another AI. These paths share validation, acceptance,
execution and negotiation cooldowns. A pending human offer
requires explicit acceptance and is revalidated against current circumstances.

The master controls the group's external war and peace. An attack on any
member involves both groups. Voluntary submission inherits the subject's wars;
capitulation ends its separate wars and adopts the master's relationships.
Existing alliances that conflict with inherited wars prevent submission, as
does a Senate veto on the master's newly inherited war. Mandatory participation
does not separately consult each subject's Senate. Treaty durations use the two
masters' rules, and expiration propagates across the group.

Protected nations receive mutual military access and can share city tiles.
Vassals provide sight to their master. These permissions do not grant alliance
victory, shared research or the right to use foreign transports. Independently
negotiated vision grants remain distinct from mandatory vassal sight.

## Independence

Voluntary subjects become eligible after 10 turns. Capitulated subjects become
eligible when they retain less than half their original land, or when both land
and population exceed half the master's. Land counts owned non-water tiles;
population sums city sizes. Exact equality does not meet a strict threshold.
Eligibility alone does not end an agreement. AI departure considers protection
failure, trust, visible military strength and threats to its survival.

As a CivCraft convenience, masters can explicitly grant independence. Elimination
of either participant cleans up the agreement. Independence preserves existing
wars and remembered map knowledge, removes mandatory access and sight, and
retains separately negotiated grants. Former partners receive the existing
16-turn armistice to withdraw their garrisons. Negotiation cooldowns prevent immediate
repeat submission offers.

Conquest can end when all surviving regular nations belong to one master's
group. Subjects remain living civilizations with independent scores. Other
victory conditions retain their existing rules.

## Ownership and integration

- `sim/diplomacy/vassalage` owns directional agreements, independence, AI
  evaluation, configured thresholds and its save extension.
- `sim/diplomacy/transitions` computes group changes before mutation. Native
  pact transitions remain in `pacts`; mandatory changes use its raw writer.
- `TreatyOffer` carries the explicit master and agreement type. `offers`
  validates the complete settlement before executing it.
- `content/vassalage.toml` owns mechanical thresholds and AI tuning. Military
  force estimates are shared with existing peace recovery and use visible
  enemy units; human private strategy is not read.
- `Command` is the player's mutation boundary. The client receives projections
  through `ClientSimulation`; scenarios use commands or the existing `Drive`
  wrappers rather than widening simulation mutation visibility.
- Save format `0x2F` appends agreements, negotiation schedules and pending terms
  to the frozen campaign core. Formats `0x26`–`0x2E` load with no vassals.
  Loading validates participants, graph shape, dates and group war consistency.
- The existing diplomacy provider includes agreements and independence reasons.
  `diplomacy.transition` records formation, eligibility changes and dissolution;
  the diplomacy analyzer retains the normal event evidence references.

## Validation

Private tests live beside `vassalage`; persistence tests live beside `save`.
Registered `vassal-capitulation` and `vassal-voluntary` scenarios exercise AI
proposals, human approval, inherited wars and an ordinary campaign turn. Run
these through the release headless runner with one turn and inspect every
scenario check.

The additional runtimes use AI nations in slots 1 and 2, with slot 0 outside
the agreement. They start without vassal agreements and exercise the normal
turn pipeline, with no scripted submission or acceptance:

| Scenario | Turns | Evidence |
| --- | --- | --- |
| `ai-vassal-capitulation` | 2 | AI surrender is proposed, accepted and maintained. |
| `ai-vassal-voluntary` | 2 | An AI obtains protection from another AI, which inherits its war. |
| `ai-vassal-war-peace` | 5 | After formation, a prepared AI war plan produces declarations for both members; normal negotiation subsequently produces a shared ceasefire. |
| `ai-vassal-independence` | 3 | Loss of two remote cities causes protection failure and automatic independence during ordinary turn processing. |

The armies occupy visible islands separated by water so formation checks do not
depend on combat rolls. The war scenario injects only a prepared war plan; the
independence scenario injects city loss through the existing lifecycle step.
Neither directly signs a treaty or changes a vassal agreement. Runtime checks
require the expected number of turns and complete trace coverage. The regression
suite additionally reverses the two AI slots and checks that a conqueror ready
to negotiate requests capitulation before offering a ceasefire.

UI review requires the complete Civ III installation and an explicit baseline
as described in `UI_REVIEW.md`.
