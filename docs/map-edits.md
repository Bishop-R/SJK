# Map edits

SJK's own fixes to maps, drawn by SJK clients only (Sol's request, 18/10/2026): staff hide
a map's props and surfaces, place models and swap shaders in game, publish them through
the hub, and every SJK player sees them while `cg_mapEdits` is on (Settings > GAME > "SJK
map edits", on by default). Code: [map_edits](../crates/sjk-viewer/src/map_edits/mod.rs),
the hub copies in [sjk-identity's map_edits](../crates/sjk-identity/src/map_edits.rs).

## What an edit does

| Edit | Drawn | Not changed |
| --- | --- | --- |
| `hide_prop` | the map's `misc_model_static` prop no longer draws | its lamps, if it has any ([static lamps](rendering.md)) |
| `hide_surface` | a BSP surface draws nothing, and picks and impact marks skip it | the bake around it |
| `place` | an MD3 from the game data at a position, turned and scaled | lighting: entity light only, as props get |
| `remap` | a shader drawn instead of another wherever the map draws it | |
| `show_face` | one caulk or nodraw side of a world brush, drawn with a shader | collision, the bake |

Edits change what is drawn, never collision. The server keeps every brush and entity: a
hidden prop or surface with clip brushes is an invisible wall, and a placed model can be
walked and shot through. Server entities (breakables, doors, movers) are refused by
the hide tool: the game needs them.

Many maps keep no props of their own: the map compiler merges `misc_model`s into the
world, and only `misc_model_static` stays an entity (retail: none on ffa1-ffa4, 51 on
duel6, 84 on ffa5, 189 on siege_korriban). A merged model is hidden as surfaces:
`mapedit_hide group` takes the triangle-soup surfaces touching the one aimed at,
through shared corners (at most 256), which is usually the whole model.

Caulk and nodraw sides have no drawn surface at all: q3map2 makes none for them, so a
remap cannot reach them (`r_showBrushes` rebuilds them from the planes for the same
reason). A `show_face` edit names one side of a world brush by its BSP numbers (with
its shader, checked on load) and draws it with the shader chosen, rebuilt from its
brush's planes, textured as Radiant textures a new brush (the axial projection at scale
0.5, no shift or turn, the image's own size read from its header) and lit like a placed
model from the face's middle ([faces.rs](../crates/sjk-viewer/src/map_edits/faces.rs)).
Only sides of solid brushes are offered (caulk, nodraw, an invisible wall), never clip,
trigger or water. Each face is one small mesh, built once.

A remap edit wins over the map's own worldspawn remaps and the server's, so a fix made
for a server's remaps holds; a console `remapShader` made after them still wins
([rendering.md](rendering.md#server-shader-remaps)). `listRemaps` names them `edit`.

## Documents and servers

Each map's edits are one JSON document per map and BSP checksum (`CM_LoadMap`'s, the
one servers send as `sv_mapChecksum`), so they apply only to that exact file:
`GameData/SJK/map_edits/<map>_<checksum>.json`, `mp/ffa3` as `mp_ffa3_-1234.json`.
Entity and surface numbers are the BSP's own. Each edit has an id and applies on every
server, or on one server only (`"server": "ip:port"`): the same map can carry fixes for
all servers and others for one server's remaps or layout. The map's checksum is read on
a worker, and only when a document of the map is there or the editor is on.

The identity worker reads the hub's list every 10 minutes once registered, and at once
when the editor turns on, after a publish, when `mapedits_sync` (Settings > Game "Sync
map edits", any player) asks, and when the hub's map edits stamp changes: every presence
answer (polled every 5 seconds on a server) carries it, the sum of every map's revision,
which grows with every publish, delete or undo (hub `PROTOCOL.md`, 03/11/2026; an older
hub leaves it out and only the timed look remains). The first stamp seen on a run counts
as a change. So a publish reaches the players on a server within seconds. Each look
downloads the documents newer than its copies and removes the copies of documents the hub no longer lists. A copy changed on
this PC and not yet published (`"unpublished": true`) is never overwritten or removed.
The loaded map's document is read again when a copy changes. What each look did goes in
the log when it changes (`map edits: 3 maps on the hub, 1 copy changed`, or `map edits:
not in step with the hub: <why>`), and the panel shows a failure at its foot. A hub
without map edits answers 404, worded "This SJK hub does not keep map edits yet"
(`sjk_identity::map_edits::refusal`).

A publish replaces the hub's whole list for the map: two staff members editing one map
should take turns, or the later publish drops the other's edits.

## The editor (staff)

`mapedit` turns the editor on for SJK staff (the hub's staff flag), and so does the
in-game menu's Map editor icon (a staff key's, beside Staff tools), which opens [the
panel](#the-panel) too. While it is on, the world-note key selects (its outline shows
what an edit will touch) and opens the panel, never a note; a command without a selection
acts on what is under the crosshair.

| Command | Does |
| --- | --- |
| `mapedit [on\|off]` | Turn the editor on or off |
| `mapedit_panel` | Open [the panel](#the-panel) (turning the editor on) |
| `mapedit_hide [group]` | Hide the selected prop or surface (`group`: the whole merged model); on a placed model, remove it |
| `mapedit_place [model]` | Place an MD3 where you aim, facing you: the path given, or a copy of the selected model |
| `mapedit_turn [degrees]` | Turn the current placed model (15 by default) |
| `mapedit_move <up> \| <x y z>` | Move it |
| `mapedit_scale <factor>` | Scale it (0.01 to 64 times) |
| `mapedit_grab` | It follows your aim until `mapedit_grab` again; meanwhile the mouse wheel turns it (15 degrees a notch, 1 with Shift) instead of changing weapon |
| `mapedit_remap <shader> \| -` | Draw `shader` instead of the selected one; `-` removes that remap edit. On a caulk or nodraw face: draw that face with `shader`, `-` draws it as nothing again (as does `mapedit_hide`) |
| `mapedit_scope [all\|server]` | New edits for all servers (the default) or only this one |
| `mapedit_list`, `mapedit_delete <id>` | List, remove one |
| `mapedit_undo`, `mapedit_redo` | Undo the last change to the map's edits (a hide, a placing, a turn, a move, a remap, a delete), or redo it |
| `mapedit_publish` | Send the map's document to the hub for every SJK player |

Every change is saved to the map's document at once and drawn at once; it reaches other
players only with `mapedit_publish`, whose answer (published with the hub's revision, or
why not) shows in the panel, the chat and the log. The current placed model is the last
placed, the placed model selected, or the one chosen in the Edits list. Undo keeps the
last 100 changes of the map ([editor.rs](../crates/sjk-viewer/src/map_edits/editor.rs),
`History`), forgotten when the map changes or the hub brings another staff member's
version.

## The panel

The editor's commands as an SJK UI panel (19/10/2026, Sol's request:
[sjk-ui.md](sjk-ui.md#map-editor) has its look), in
[panel.rs](../crates/sjk-viewer/src/map_edits/panel.rs). With the editor on, the
world-note key selects what is under the crosshair and opens the panel, the pointer freed;
`mapedit` opens it when it turns the editor on, `mapedit_panel` (bindable) at any time. A
click on the scene left of the panel selects what is under the pointer instead. Escape, or
Aim, hides it to aim again; a strip at the top of the screen says the editor is on and
which key edits. While the editor is on a selection never goes stale (a world note's
waits 20 seconds), until the map changes.

| Page | Offers |
| --- | --- |
| Selected | what it is (map prop, placed model, surface, game entity, hidden face: a caulk or nodraw side nearer than any drawn surface) with its model and shader; Hide (Remove for a placed model), Hide whole model (a merged model's surfaces), Place a copy; Draw another shader: a search over the shaders a remap may draw (the world's `textures/` and the map objects' `models/`, shaders and images, never the player models'), opened on the selection's own folder with its own shader chosen, five rows; Remap (or a second click on the chosen row) and Remove remap. A name the list does not know is tried as typed |
| Place | every MD3 of the game data outside the player models, narrowed by a search, six rows; Place where you aimed (or a second click on the chosen row); the current placed model's Turn left and right (15 degrees), Up and Down (8 units), Left, Right, Away and Closer (8 units along the world axis nearest the view's right and ahead, so steps add up straight), Smaller and Bigger (by 1.1), Grab and Remove. Shift held makes the steps 1 degree, 1 unit and a tenth of the growth |
| Edits | the map's edits, newest first, named shortly (`3: Placed: crate.md3`, `Hidden surface: floor (1043)`, `Remap: wall -> wall2`, a server's address when for one), those for another server dimmed; choosing a placed model makes it the one Place's buttons move; Delete the chosen one, Undo and Redo |

Under them on every page: New edits for All servers or This server, the last command's
answer (ember when refused; Publish's comes when the hub answers; with nothing to say, a
failed look at the hub), Publish (gold while the map has edits not published), Aim and
Editor off. A click on the scene selects what is under the pointer and shows it on
Selected; on Place it stays there and chooses where Place puts the model. Grab hides the
panel and the model follows the aim, the wheel turning it; the world-note key puts it
down and opens the panel again. Keys: Up and Down (along a list too; in a search they
choose in its list), Tab, Left and Right on the tabs and the scope, Enter acts (in a
search it places or remaps), Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z) redoes, Delete on an
edit deletes it. Each button runs the command the console would (`mapedit_hide`,
`mapedit_place <model>`, ...), so both stay one editor.

## The browser editor (creator only for now)

`mapedit_web` edits the map in a web browser (03/11/2026, Sol's request). For now only
SJK's creator may use it (the hub's creator flag, Sol's key alone): the command refuses
everyone else, and the game checks the flag again on every request the page makes, so a
running page stops editing when it goes. The command turns the editor on, starts a small
HTTP server in the client on 127.0.0.1 at a free port and opens the default browser on
`http://127.0.0.1:<port>/?t=<token>`. Code:
[map_edits/web](../crates/sjk-viewer/src/map_edits/web/mod.rs).

The page draws the map the player is on in 3D with WebGL, with its shader scripts, its
`misc_model_static` props and its flares, and SJK's edits as the game draws them: hidden
props and surfaces left out, remap edits applied, placed models and shown faces drawn. It
starts at the player's view (My view in game goes back there). Drag looks, W A S D flies,
Q and E go down and up, Shift is faster, the wheel sets the speed. Its panel has the
in-game panel's pages: Selected (a click on the map selects what the game's own pick
finds along that ray; Hide, Hide whole model, Place a copy, a shader list with pictures
and Remap, Remove remap), Place (the models list; a click on the map places the chosen
one, then Turn, Up, Down, Left, Right, Away, Closer, Smaller, Bigger, Remove, Shift for
fine steps) and Edits (the list, Delete, Undo, Redo; a double click flies to a placed
model), and under them the scope (All servers, This server), the last answer and Publish.
H hides the selection, Ctrl+Z and Ctrl+Y undo and redo.

Every edit is a request the game thread runs as the in-game editor's command
(`mapedit_hide`, `mapedit_place <model>`, `mapedit_turn`, `mapedit_move x y z`, ...), so
it is drawn in the game at once, saved, undone and published exactly like the in-game
editor's, and the in-game panel shows its answer. The page reads the game's state about
once a second (`api/edits`: the map, the edits, what they draw, the selection, the view),
so a map change loads the new map in the page; the server stops with `mapedit off` and
when the game quits.

The drawing is VisionJK's web admin page (by Blackwolf, GPL-2.0, used with his approval;
VisionJK's panel comes from JKAdmin's by Lixsl), cut down to the map: its files under
[web/page/vjk](../crates/sjk-viewer/src/map_edits/web/page/vjk) are copies, each headed
with where it comes from and what SJK changed (the batches keep each surface's triangles so
an edit can hide one; the props keep their entity numbers), `models.js` is its MD3 loading
and drawing taken from three of its files, and `editor-remaps.js` is only the part that
draws remaps and the shaders' pictures. Players, the HUD, effects, weapons, server
entities, clips, routes and bots are left out: map edits change only what is drawn.
`web/page/sjk` is SJK's own (the stubs for the parts left out, the edits, the panel, the
camera and the frame). The page is put together at compile time from `index.html`'s
`<!--#include -->` lines ([page.rs](../crates/sjk-viewer/src/map_edits/web/page.rs)), no
build tools.

The server ([http.rs](../crates/sjk-viewer/src/map_edits/web/http.rs)) answers only on
127.0.0.1, only to requests that name it as `127.0.0.1:<port>` or `localhost:<port>` in
`Host` (a site that points a name of its own at 127.0.0.1 is refused), and only with the
random token: in the page's address, then as the `X-SJK-Token` header of every `api/`
request. A POST must come from the page (`Origin`) as JSON. Files are read only from the
game data, by a path the game data accepts (no `..`, no absolute path):

| Request | Answers |
| --- | --- |
| `GET api/map?name=<map>` | `maps/<map>.bsp` |
| `GET api/file?name=<path>` | a file of the game data (images, shader scripts, models) |
| `GET api/shaders` | `{"files": [...]}`: the shader scripts in the order the game reads them |
| `GET api/list?kind=dir&dir=&ext=` | the folders and files of a folder, as VisionJK's |
| `GET api/edits` | the game's state (above) |
| `GET api/lists` | the models to place and the shaders a remap may draw |
| `POST api/edit` | `{"op": "select"\|"hide"\|"place"\|"turn"\|"move"\|"scale"\|"remap"\|"scope"\|"current"\|"delete"\|"undo"\|"redo"\|"publish", ...}` |

Not yet: the map's own and the server's remaps are not drawn in the page (only remap
edits), nor the selection's outline (a marker stands at the point chosen); a mover is drawn
where the map file has it; Grab has no page version.

## Limits

- Only MD3s already in the player's game data can be placed; a custom model needs a pack
  the players have (an SJK hub pack).
- A hidden prop's lamps and a hidden surface's bake stay; a placed model casts no baked
  shadow.
- Edits are not drawn in the menu world.
- The hub's side is the hub repository's `map-edits` branch (19/10/2026). Until the live
  hub runs it, it answers 404: Publish says the hub does not keep map edits yet, and
  players get no copies (found 28/10/2026: the branch had never been merged there).
- A shown face's texture always lies at Radiant's default; the map's own projection of
  that side is not in the BSP.
- Verified: unit tests (documents, scopes, the remap order, the folder's sync against a
  fake hub, the refusals' words, undo and the panel's lists), the hub round trip against
  a local hub (`staff_publish_map_edits_that_every_key_reads` in `hub_e2e.rs`), and the
  world shot `duel6_prop_hidden_copied_and_floor_remapped` (a duel6 prop hidden, a surface
  hidden, a copy placed, a wall remapped, then all taken back), and
  `duel6_map_editor_panel` for the panel; `caulk_face_shown` (02/11/2026) selects a
  duel6 caulk wall under the crosshair and draws it with a wall shader. Not tried in a
  game.
- The browser editor (03/11/2026): unit tests of its HTTP parsing, the Host, token and
  origin checks, the paths and the edit requests, an end-to-end test of the server on a
  socket with a fake game thread (the page, `api/map`, an edit, refusals), and the page in
  headless Chrome (SwiftShader) on retail duel6 served by the ignored test
  `serve_the_page_for_a_browser` (no script errors; the map, its props and the panel drawn;
  a hidden prop, a placed copy, hidden surfaces and a remap from a fake state drawn; a
  click's request reaching the server). Not tried with the game running.
