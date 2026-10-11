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

## Bridges added

| Bridge | Legs | Rules modelled | Source (read 2026-10-11) |
| --- | --- | --- | --- |
| San Francisco-Oakland Bay Bridge | Sacramento, Stockton and Los Angeles to San Francisco; San Francisco to Portland and Salt Lake City | Explosives and flammable-placarded tank loads banned; placarded packaged freight refused (classes unnamed) | https://www.chp.ca.gov/siteassets/files/ib_hmroutes.pdf (21 CCR 1402.1) |
| Mackinac Bridge | Sault Ste. Marie to Traverse City | Toll: 5 dollars per axle, 25 dollars for a five-axle rig, both directions | https://www.mackinacbridge.org/fares-traffic/fare-schedule/ |
| Chesapeake Bay Bridge-Tunnel | Norfolk to Cape Charles | Toll corrected from an estimated 45 to the published 48 dollars (Class 12, initial crossing) | https://www.cbbt.com/tolls/ |

Bay Bridge legs were matched where archived geometry runs within 60 metres
of the Yerba Buena Island tunnel. The Mackinac toll sits at the St. Ignace
plaza, 52.1 miles along its leg, from the same geometry.

**Posted bridge weight limits and low clearances** were already on the map
as corridor restrictions taken from OpenStreetMap; truck routing already
keeps a legal rig under them, so they stay as spoken signage and are not
duplicated here.

## Researched and left out

- **Baltimore Harbor Tunnel (I-895).** Same MDTA ban, but no leg runs
  through it.
- **Wallace Tunnel, Wheeling Tunnel, I-10 Deck Park Tunnel, Harvey Tunnel,
  Vista Ridge Tunnel, Pigeon River tunnels.** On a leg and flagged in the
  FHWA inventory, but no official rule text was found to model.
- **George Washington Bridge lower level.** Trucks are kept off the lower
  level, but placarded loads may use the upper level, so no load is turned
  away; a bridge in any case.
- **Golden Gate Bridge (San Francisco to Santa Rosa).** It bans explosive
  loads only at rush hours and escorts them otherwise; the game has no
  explosives cargo or escort, so nothing is refused.
- **Mackinac Bridge escorts.** Placarded loads, rigs over 40 tons gross and
  oversize loads cross under bridge escort, not a ban; escorts are not
  modelled. Its 72-ton maximum is above any legal game rig.
- **Wind closures.** The Chesapeake Bay Bridge-Tunnel restricts empty
  tractor-trailers (under 15,000 lb of cargo) from 40 mph winds, and the
  Mackinac Bridge closes in high wind. The game's weather has no wind
  speed yet, so these are not modelled.

## Tolls

Every toll on the map was an estimate until this audit. 62 of the
67 toll events now carry a five-axle rate read from the authority's own
schedule or calculator, with the source and access date on the event.

- **Amount** is what a five-axle rig pays with a transponder: Class 5H in
  Pennsylvania and New York, class 6 (high five-axle) on the Ohio Turnpike,
  the five-axle class elsewhere.
- **Plate rate** is what it pays without one: toll-by-plate in Pennsylvania
  and New York, the video toll in Maryland, the non-E-ZPass fare in Ohio,
  cash in West Virginia and Maine. Where the authority charges one price, the
  two are equal.
- **One-way collection** is modelled where the authority collects in one
  direction only: the Chesapeake Bay Bridge (eastbound), the JFK Memorial
  Highway (northbound), the Pennsylvania Turnpike Gateway barrier
  (eastbound; no toll westbound into Ohio), the Ohio Turnpike Eastgate
  barrier (westbound), the New England Thruway (northbound), the Carquinez
  Bridge (eastbound) and the I-78 Toll Bridge (westbound, into Pennsylvania).
- **Where the leg actually drives decides the fare.** Harrisburg to
  Pittsburgh enters at Carlisle; Cleveland to Toledo runs North Ridgeville to
  Perrysburg; Youngstown to Pittsburgh pays the Toll 376 Beaver Valley
  mainline plaza, not the Gateway barrier; Bangor to Portland leaves I-95 at
  Gardiner for I-295 and pays only the Gardiner toll; West Virginia charges a
  rig the out-of-state E-ZPass rate per mainline plaza.

Sources: Pennsylvania Turnpike 2026 E-ZPass and toll-by-plate schedules
(https://www.paturnpike.com/toll-calculator/toll-schedules), Ohio Turnpike
fare calculator (https://www.ohioturnpike.org/e-zpass/toll-rates), DelDOT
I-95 rates (https://deldot.gov/public.ejs?command=PublicTollRateI95), MDTA
rate tables (https://mdta.maryland.gov/TollRatesTables), West Virginia
Parkways Authority (https://turnpike.wv.gov/toll-rates) and Maine Turnpike
Authority (https://www.maineturnpike.com/traveler-services/tolls), all read
2026-10-11. The New York, Massachusetts, Indiana, Illinois, Kansas, Oklahoma
and Florida rates come from the research recorded in `tools/toll_rates.py`
(2026-07-19); each event names the schedule it came from.

Tolls added where a leg drove a toll road for free: the New York Thruway
on Buffalo, Rochester and Albany to New York and on Erie to Buffalo; the
Northeast Extension on Scranton to Allentown; the New Jersey Turnpike
western spur on Newark to New York; the I-78 Toll Bridge on Newark to
Allentown and Allentown to New York; the Hampton plaza on Portsmouth to
Boston; the Carquinez Bridge on Sacramento to San Francisco; the Chelyan
and Pax barriers on Charleston to Richmond; the Cimarron Turnpike on
Wichita to Tulsa; and the Ohio Turnpike Eastgate barrier on Pittsburgh to
Cleveland. Each leg's road line was checked to pass the plaza or to run the
priced stretch. The Thruway fares to New York City include the Mario Cuomo
Bridge, which is collected southbound only, so the northbound fare is a few
dollars high until the bridge is split out.

Still estimated, because no official figure was matched to the leg's road:
aurora_il_us:rockford_il_us Reagan Tollway settlement; new_york_ny_us:boston_ma_us Massachusetts Turnpike settlement segment; new_york_ny_us:philadelphia_pa_us New Jersey Turnpike ticket entry; new_york_ny_us:philadelphia_pa_us Delaware River Turnpike Toll Bridge settlement point; syracuse_ny_us:utica_ny_us New York State Thruway settlement.

## Whittier

Whittier still has no freight. No real freight receiver with a verifiable
address and coordinates was found, so no facility was added and no career
job drives the tunnel; see [whittier-tunnel.md](whittier-tunnel.md).
