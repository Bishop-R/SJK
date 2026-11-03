//! The server end to end on a real socket, with a fake game thread: the page, the map,
//! an edit handed to the game, and refused requests.

use super::*;
use std::io::{Read, Write};

/// Send `head` (and `body`) to the server and read its whole answer: the status and
/// the body.
fn send(port: u16, head: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).unwrap();
    let split = answer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("a head");
    let status = std::str::from_utf8(&answer[9..12])
        .unwrap()
        .parse()
        .unwrap();
    (status, answer[split + 4..].to_vec())
}

fn get(port: u16, path: &str, token: Option<&str>, host: Option<&str>) -> (u16, Vec<u8>) {
    let host = host.map_or_else(|| format!("127.0.0.1:{port}"), str::to_owned);
    let token = token.map_or_else(String::new, |token| format!("X-SJK-Token: {token}\r\n"));
    send(
        port,
        &format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n{token}\r\n"),
        &[],
    )
}

/// Answer the game's requests as a game thread would, until the server is dropped.
fn fake_game(server: &Server, until: impl Fn() -> bool) {
    while !until() {
        match server.next_request() {
            Some(ask) => {
                let answer = match &ask.request {
                    GameRequest::State => Ok(serde_json::json!({ "map": "mp/test" })),
                    GameRequest::Lists => Ok(serde_json::json!({ "models": [] })),
                    GameRequest::Edit(edit) => Ok(serde_json::json!({
                        "ok": true,
                        "lines": [format!("{:?}", edit.command())],
                    })),
                };
                ask.answer(answer);
            }
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
}

#[test]
fn the_page_the_map_and_an_edit_are_served_and_strangers_refused() {
    let mut vfs = VirtualFileSystem::new();
    vfs.mount_memory("test", [("maps/mp/test.bsp", &b"IBSP-bytes"[..])])
        .unwrap();
    let server = Server::start(Some(Arc::new(vfs))).unwrap();
    let token = server.token.clone();
    let port = server.port;
    assert!(server.url().ends_with(&format!("/?t={token}")));
    std::thread::scope(|scope| {
        let client = scope.spawn(|| {
        // The page, with its token in its address.
        let (status, page) = get(port, &format!("/?t={token}"), None, None);
        assert_eq!(status, 200);
        assert!(String::from_utf8(page).unwrap().contains("SJK map editor"));
        // The map's bytes from the game data.
        let (status, map) = get(port, "/api/map?name=mp/test", Some(&token), None);
        assert_eq!((status, map.as_slice()), (200, &b"IBSP-bytes"[..]));
        let (status, _) = get(port, "/api/map?name=mp/none", Some(&token), None);
        assert_eq!(status, 404);
        let (status, _) = get(port, "/api/file?name=../../x", Some(&token), None);
        assert_eq!(status, 400);
        // A question for the game thread.
        let (status, state) = get(port, "/api/edits", Some(&token), None);
        assert_eq!(status, 200);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&state).unwrap()["map"],
            "mp/test"
        );
        // An edit from the page.
        let body = br#"{"op":"turn","degrees":15}"#;
        let origin = format!("http://127.0.0.1:{}", port);
        let (status, answer) = send(
            port,
            &format!(
                "POST /api/edit HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-SJK-Token: {token}\r\n\
                 Origin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                port,
                body.len()
            ),
            body,
        );
        assert_eq!(status, 200);
        assert!(String::from_utf8(answer).unwrap().contains("mapedit_turn"));
        // Refused: no token, a wrong one, another host (DNS rebinding), a post from
        // another site.
        assert_eq!(get(port, "/", None, None).0, 403);
        assert_eq!(get(port, "/api/edits", None, None).0, 403);
        assert_eq!(get(port, "/api/edits", Some("nope"), None).0, 403);
        let rebound = format!("evil.example:{}", port);
        assert_eq!(
            get(port, &format!("/?t={token}"), None, Some(&rebound)).0,
            421
        );
        let (status, _) = send(
            port,
            &format!(
                "POST /api/edit HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-SJK-Token: {token}\r\n\
                 Origin: http://evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                port,
                body.len()
            ),
            body,
        );
        assert_eq!(status, 403);
        });
        fake_game(&server, || client.is_finished());
        client.join().unwrap();
    });
    drop(server);
    // Stopped: nothing answers on its port any more.
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
}

/// For a look at the page in a browser without the game: serves the retail game data
/// (`SJK_MAPEDIT_WEB_GAMEDATA`, its `base` folder) as if the game were on
/// `SJK_MAPEDIT_WEB_MAP` (mp/duel6 by default) for `SJK_MAPEDIT_WEB_SECONDS`, printing
/// the page's address. Edits are refused: there is no game.
#[test]
#[ignore = "serves a page for a manual look in a browser"]
fn serve_the_page_for_a_browser() {
    let base = std::env::var("SJK_MAPEDIT_WEB_GAMEDATA")
        .expect("SJK_MAPEDIT_WEB_GAMEDATA: the GameData folder");
    let map = std::env::var("SJK_MAPEDIT_WEB_MAP").unwrap_or_else(|_| "mp/duel6".into());
    let seconds: u64 = std::env::var("SJK_MAPEDIT_WEB_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(120);
    let mut vfs = VirtualFileSystem::new();
    vfs.mount_pk3_directory(std::path::Path::new(&base).join("base"))
        .unwrap();
    let vfs = Arc::new(vfs);
    let models = crate::map_edits::panel::placeable_models(&vfs);
    // Images of textures/ as shaders to remap to (the game adds its scripts' names).
    let shaders: Vec<String> = vfs
        .paths()
        .iter()
        .filter_map(|path| {
            let name = path.as_str();
            (name.starts_with("textures/") && name.ends_with(".jpg"))
                .then(|| name.trim_end_matches(".jpg").to_owned())
        })
        .collect();
    // With SJK_MAPEDIT_WEB_DEMO set, edits as the game would apply them: the first prop
    // hidden, a copy of its model placed beside it, the first 40 surfaces hidden, the
    // first surface's shader remapped.
    let demo = std::env::var_os("SJK_MAPEDIT_WEB_DEMO").is_some();
    let bsp = vfs
        .read(&format!("maps/{map}.bsp"))
        .unwrap()
        .map(|asset| sjk_bsp::Bsp::parse(&asset.bytes).unwrap());
    let (mut props, mut placed, mut remaps) = (vec![], vec![], vec![]);
    if let (true, Some(bsp)) = (demo, &bsp) {
        let lump = sjk_entity::parse_entity_lump(bsp.entities()).unwrap();
        if let Some((number, prop)) = lump
            .iter()
            .enumerate()
            .find(|(_, e)| e.get("classname") == Some("misc_model_static"))
        {
            props.push(number);
            let origin: Vec<f32> = prop
                .get("origin")
                .unwrap_or("0 0 0")
                .split_whitespace()
                .map(|v| v.parse().unwrap_or(0.0))
                .collect();
            placed.push(serde_json::json!({
                "id": 2, "model": prop.get("model"), "origin": [origin[0] + 96.0, origin[1], origin[2]],
                "angles": [0, 45, 0], "scale": [1.5, 1.5, 1.5],
            }));
        }
        let first = bsp.render().surfaces()[0].shader;
        remaps.push(serde_json::json!([
            bsp.shaders()[first].name_lossy(),
            "textures/yavin/bblock3"
        ]));
    }
    let surfaces: Vec<u32> = if demo { (0..40).collect() } else { vec![] };
    let server = Server::start(Some(vfs)).unwrap();
    println!("SJK_MAPEDIT_WEB_URL={}", server.url());
    let end = std::time::Instant::now() + Duration::from_secs(seconds);
    while std::time::Instant::now() < end {
        let Some(ask) = server.next_request() else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        let answer = match &ask.request {
            GameRequest::State => Ok(serde_json::json!({
                "map": map, "checksum": 1, "editor": true, "enabled": true, "scope": "all",
                "server": null, "revision": 0, "unpublished": false, "edits": [], "current": null,
                "undo": false, "redo": false, "hidden_props": props, "hidden_surfaces": surfaces,
                "placed": placed, "remaps": remaps, "faces": [], "selection": null, "view": null,
                "answer": null, "hub": null,
            })),
            GameRequest::Lists => Ok(serde_json::json!({ "models": models, "shaders": shaders })),
            GameRequest::Edit(_) => Err("no game: this is the page's test server".into()),
        };
        ask.answer(answer);
    }
}
