# The Toy Master

Secret toys SJK's creator, the Toy Master, hides on maps for SJK players to find (Sol's
idea, 30/10/2026). The surprise is the point, so this repository holds only the
mechanism: a prop that counts `+use` presses and plays a show. What each toy is, its
model, its sound, its show and the achievement for finding it, lives on the SJK hub,
which tells a client only of the toys standing on its map and names an achievement only
to whoever unlocked it. Code: [toy_master.rs](../crates/sjk-viewer/src/toy_master.rs),
the hub calls in [sjk-identity's toy_master.rs](../crates/sjk-identity/src/toy_master.rs).

No game server knows of a toy. SJK clients alone draw it, it has no collision (players
walk through it), and players on other clients see someone pressing Use at nothing.

## What the hub sends

`GET /v1/secrets` (once registered, then every 10 minutes, and at once after the
creator hides or removes one) lists every placed toy: its id, the map file it stands in
(name and BSP checksum, as [mystery boxes](mystery-boxes.md)), a server when it is
placed for one only, where it stands and faces, and how it looks and plays:

- `model`, an MD3 at a game path, from the hub's [asset packs](identity.md#asset-packs) (the client
  downloads and mounts them; a pack that arrives during a match is drawn from the next
  map), grown by `scale`;
- `sound`, the sound its show plays, which may come from a pack a player does not have
  (the Sith cloud's optional [extras](sith-cloud.md)): without it the show plays
  silent;
- `show`: how long it shrinks away, stays gone and pops back, how long it spins and its
  fastest spin.

The creator's list also names each toy's kind and the kinds there are.

## Finding one

Standing at a toy (within 80 units across the floor and 96 up or down) and facing it
(within 60 degrees), each `+use` press counts. Nothing says a toy is there: it is a
surprise. The client sends the presses on the toy twice a second at most
(`POST /v1/secrets/use`, at most 10 a time), with the server and the map load
(the game's `sv_serverid`, which every player of a server reads alike).

- The player's first press on a kind of toy finds it: the hub unlocks that toy's hidden
  achievement and its kyber crystal, and the client announces it as any achievement
  (the unlock card and a console line).
- The presses of every player on the server add up, towards a number of presses the hub
  rolls for each map load and never tells. The presses that reach it start the toy's
  show, once for that map load: the toy shrinks away (`sound/player/teleout.mp3` and the
  `mp/spawn` sparkle), stays gone, pops back past its size and settles
  (`sound/player/telein.mp3`, the sparkle again) and spins, faster and faster, then
  slowing to a stop, while its sound plays from where it stands (heard to 1400 units).
  The hub tells the server's other SJK players through its toy event (`secrets`,
  `show_<id>`), so they see and hear the same show.
- Once a toy played its show on a map load, presses on it count no more until the next
  (a player who has not found it yet still finds it with one press).

Every sound plays on a source of the toy's own (`0x5300_0000` and its id), and the
show's sound stops when the show ends or the player leaves the map. Settings > Sound >
Toys, "Toy Master sounds" (`s_toyMasterSound`), silences them.

## The "?" and the achievements

Collection > Toys has a "?" at the right of its column heads, the Toy Master's: its
hover card says the Toy Master hides toys on maps, that each one found is a secret
achievement, and names the ones the player found. It turns gold once they found one.

A Toy Master's achievement is not in the client's catalogue (`achievements::ALL`):
a public catalogue cannot keep a secret. The hub sends it with the player's other
achievements only once it is unlocked, with its name, what it was for and its group,
`toy_master` (`sjk_identity::Achievement::name`, `description`, `group`), and the client
keeps it for the session (`achievements::secret_kind`). The Achievements wall shows the
ones found in a "Toy Master" category on its Fun page, after the toys', on the first band
with room ("2 found"); they count in the unlocked total and in the total alike, so nothing tells
how many there are. Other players' cards count the ones they found the same way.

## Hiding one (SJK's creator)

| Command | Does |
| --- | --- |
| `toymaster_place <kind> [server]` | Hide a toy of that kind where you aim, facing you: on every server running this map file, or with `server` on this one only |
| `toymaster_remove [id]` | Take away the toy nearest you (within 256 units), or the one with that id |
| `toymaster_list` | The kinds there are, and the toys on this map with their ids and distance |

All three are the creator's only. Placing needs the map's BSP checksum (as map edits
do, `sv_mapChecksum`). The hub takes the creator's request only; every placing and
removal goes to the staff log.

## Limits

- The hub cannot tell where a player stands: the reach is the client's, so a modified
  client could press a toy from anywhere. A toy's show comes at most once every 10
  minutes on a server whatever map load a client names.
- A toy can only do what the client's show knows: shrink away, pop back and spin to a
  sound. A toy that does something else needs a client release for that behaviour (named
  for what it does, never for the toy).
- Verified: unit tests (the show's phases and spin, the presses gathered and sent, the
  faced toy, the toys of a map file, a show's sounds once each; the hub-named
  achievements, the wall's bands; on the hub, placing and removing as the creator only,
  a find unlocking once, every player's presses adding up to one show per map load, the
  gap between shows, a toy placed for one server), and world shots of the "?" and the
  wall (`duel6_sjk_toy_master`) and of a toy in each phase from a hub pack
  (`duel6_toy_master`, with `SJK_TEST_TOY_PACK` and `SJK_TEST_TOY_MODEL`). Not tried in
  a game with a hub yet.
