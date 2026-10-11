# Tunnels and bridges with truck rules

Real tunnels on the map's legs whose published rules turn trucks away. The
rules live on each leg as `crossings` in `data/world_source/legs/*.json`
(written through `tools/world_source.py`), and every entry carries its
official source, the date it was read, and a `basis` note saying which
values were read, derived or assumed. The Anton Anderson Memorial Tunnel's
schedule, toll and gate are documented in [whittier-tunnel.md](whittier-tunnel.md).

## How a crossing plays

- **Dispatch plans around it.** The job board books a regulated load only
  on a legal road. If the legal road is much longer than the open one
  (beyond the usual alternate-route allowance) or there is none, the load
  is not offered.
- **Departure re-checks.** The route menu offers only legal roads and says
  "Routed around the" crossing when the shortest road was dropped. With no
  legal road the load stays at the dock and the rule is read out.
- **No truck is stranded.** A crossing is judged where its leg starts. A
  truck that reaches one it may not use (a save from before these rules,
  for example) is re-planned from that city, and if no legal road continues
  from there it runs back toward the origin and takes the origin's legal
  road. Only the Whittier tunnel's schedule ever holds a truck, and only
  until its next published opening.

## Game loads and how rules read them

The catalog does not name hazard classes, so these mappings are modelling
choices, not measurements:

- **Placarded hazardous materials** (`hazardous`): placarded packaged
  freight of unnamed classes. A crossing that bans some classes outright
  refuses it.
- **Bulk fuel** (`fuel_bulk`): placarded Class 3 flammable liquid in a cargo
  tank.
- **Packaged chemicals** (`chemicals`): regulated but below placard
  thresholds; counted by weight against a pound limit.
- **Other tank freight** (milk, food-grade liquids): a cargo tank with no
  hazardous material, refused only where every cargo tanker is banned.

Size limits use a stock rig: 73.5 feet with a trailer, 25 feet bobtail,
8.5 feet wide, 13.5 feet high.

## Tunnels added

| Tunnel | Legs | Rules modelled | Source (read 2026-10-11) |
| --- | --- | --- | --- |
| Anton Anderson Memorial Tunnel | Bear Valley to Whittier | Placarded loads banned; 400 lb unplacarded limit; 75 ft long, 10 ft wide, 14 ft high without permit | https://dot.alaska.gov/creg/whittiertunnel/vehiclesize.shtml |
| Eisenhower-Johnson Memorial Tunnel | Denver to Salt Lake City; Denver to Silverthorne | Placarded loads banned (must use Loveland Pass while open) | https://www.codot.gov/travel/ejmt/metering |
| Fort McHenry Tunnel | Wilmington to Baltimore; Philadelphia to Baltimore; Washington to Philadelphia | Placarded loads banned, including bulk fuel | https://www.mdta.maryland.gov/TunnelRestrictionsAndVehiclePermits |
| Boston tunnels (Central Artery, Prudential, Tip O'Neill) | Boston to Hartford, Manchester, Providence, Worcester; New York, Portland and Portsmouth to Boston | All hazardous cargo (any amount) and all cargo tankers banned | https://www.mass.gov/info-details/hazardous-material-route-designation |
| Hampton Roads Bridge-Tunnel | Norfolk to Richmond; Richmond to Virginia Beach | Banned classes refuse placarded packaged loads; flammables only non-bulk, so bulk fuel refused | https://law.lis.virginia.gov/admincode/title24/agency30/chapter61/section40/ |
| Chesapeake Bay Bridge-Tunnel (Thimble Shoal and Chesapeake Channel tunnels) | Norfolk to Cape Charles | Banned classes refuse placarded packaged loads; Class 3 tank vehicles banned even empty | https://www.cbbt.com/regulations/ |
| Pennsylvania Turnpike tunnels (Allegheny, Tuscarora, Kittatinny, Blue Mountain, Lehigh) | Baltimore, Harrisburg, Philadelphia, Carlisle and Hagerstown to Pittsburgh; Scranton to Allentown | Table 1 materials and explosives banned; flammable liquid banned in bulk | https://www.paturnpike.com/commercial/permits-restrictions/hazardous-materials-(placarded-loads) |
| Interstate 90 tunnels across Lake Washington (Mount Baker Ridge, Mercer Island lid) | Seattle to Moses Lake, Wenatchee, Spokane, Tri-Cities and Yakima | Flammable cargo banned, so bulk fuel detours | https://content.govdelivery.com/accounts/WADOT/bulletins/3df8359 |

Supporting sources: Colorado rule 2 CCR 601-8, 3.02 (EJMT placarded loads
use Loveland Pass); COMAR 11.07.01.04 (Maryland tunnels); 700 CMR 7.11
(Massachusetts); 24VAC30-61-40 and Code of Virginia 46.2-1110 (Virginia
tunnels); 67 Pa. Code 601.5 (Turnpike).

**How legs were matched.** Each leg's archived road geometry was compared
with the FHWA National Tunnel Inventory (2026 release) and with portal
coordinates; only legs whose geometry runs through the bore were tagged.
The map has one record per leg, crossed in either direction, so each rule
applies both ways. Legs without archived geometry were checked against
their corridor route points.

**Assumptions to know.** EJMT: Loveland Pass is not on the map and its
closures are not modelled, so placarded loads are always planned off these
legs. Fort McHenry: unplacarded packaged chemicals are not refused, since
the propane container limits are not modelled. Boston: the map's Boston
legs end at the downtown city point through the tunnels, so most Boston
hazardous and tank freight is not offered; Barnstable to Boston stays open.

## Researched and left out

- **Baltimore Harbor Tunnel (I-895).** Same MDTA ban, but no leg runs
  through it.
- **Wallace Tunnel, Wheeling Tunnel, I-10 Deck Park Tunnel, Harvey Tunnel,
  Vista Ridge Tunnel, Pigeon River tunnels.** On a leg and flagged in the
  FHWA inventory, but no official rule text was found to model.
- **George Washington Bridge lower level.** Trucks are kept off the lower
  level, but placarded loads may use the upper level, so no load is turned
  away; a bridge in any case.
- **San Francisco-Oakland Bay Bridge (Yerba Buena tunnel).** Its I-80
  hazardous-material rule is a bridge rule; left for the bridge pass.
- **Bridges.** Posted weight limits, low clearances, hazmat rules, tolls and
  wind closures on bridges are not added yet; the Chesapeake Bay
  Bridge-Tunnel's toll and wind policy are recorded for that pass.

## Whittier

Whittier still has no freight. No real freight receiver with a verifiable
address and coordinates was found, so no facility was added and no career
job drives the tunnel; see [whittier-tunnel.md](whittier-tunnel.md).
