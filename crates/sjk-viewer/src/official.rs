//! The SJK team's announcements and contributor reward rounds in the client
//! (`docs/hub-chat.md`, "Announcements and reward rounds"; the hub's side is
//! `PROTOCOL.md`, "Announcements" and "Reward rounds").
//!
//! | piece | module |
//! | --- | --- |
//! | the chat lines' words and marks | [`words`] |
//! | `announcements_seen.txt` | [`seen`] |
//! | the windows: an announcement, the rounds, a claim, the composer | [`window`] |
//!
//! Announcements show in the SJK chat only, never in a game server's chat. The identity
//! service reads them at the registration and every 30 minutes, and the feed relays new
//! ones as they come; those not shown yet (at most the newest three, or only the newest
//! with no seen file) go to the end of the SJK chat once its first answer came
//! ([`crate::GpuState::follow_official`]). A click on a line in the docked chat or the chat
//! page opens it in full ([`request`]); the game's chat names the `announcement` command.
//! A command closes the console as it opens the window; a click on the chat page keeps
//! it under the window. While the window shows, the cursor is free and it takes every key.
//!
//! A reward round's line opens the round list (or `rewards`). A reward waiting for the
//! player opens the claim window as the medal pop-up does: on the main menu, or when the
//! game menu opens in a match, never over play; a match gets an unlock card once per
//! reward. Claim asks the hub; the gift holocrons it gives then arrive through the
//! holocron pop-up, and the window shows the next reward or closes. Later puts it away
//! until the next menu.
//!
//! The creator writes announcements with `announce` (the hub refuses anyone else).
//! `debug_announce` and `debug_reward` rehearse the lines and windows offline: nothing is
//! sent and nothing is counted as seen.

pub(crate) mod seen;
pub(crate) mod window;
pub(crate) mod words;

use sjk_identity::Official as Mark;
use sjk_identity::news::{Announcement, FeedRound, Reward, Round, RoundPerson};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use window::{Choice, Composer, Page, Window};
use words::Target;

/// `announcement [number]`: read an announcement in full (no number: the newest).
pub(crate) const ANNOUNCEMENT_COMMAND: &str = "announcement";
pub(crate) const ANNOUNCEMENT_HELP: &str =
    "Read the SJK team's announcement in full: announcement [number] (none: the newest)";
/// `rewards`: the reward rounds.
pub(crate) const REWARDS_COMMAND: &str = "rewards";
pub(crate) const REWARDS_HELP: &str = "Show the SJK team's contributor reward rounds";
/// `announce`: the creator's announcement window.
pub(crate) const ANNOUNCE_COMMAND: &str = "announce";
pub(crate) const ANNOUNCE_HELP: &str = "SJK team tool";
/// `debug_announce [release|long]`: rehearse an announcement offline.
pub(crate) const DEBUG_ANNOUNCE_COMMAND: &str = "debug_announce";
pub(crate) const DEBUG_ANNOUNCE_HELP: &str =
    "Rehearse an SJK team announcement, nothing sent: debug_announce [news|release|long|picture]";
/// `debug_reward [list]`: rehearse a contributor reward offline.
pub(crate) const DEBUG_REWARD_COMMAND: &str = "debug_reward";
pub(crate) const DEBUG_REWARD_HELP: &str =
    "Rehearse a contributor reward and its round, nothing sent: debug_reward [list]";
/// What `announce` says to anyone but the SJK team (staff, the creator among them).
pub(crate) const STAFF_ONLY: &str = "Only the SJK team can announce";

/// Numbers rehearsals start at: far above anything the hub has made.
const REHEARSAL_ID: u64 = 4_000_000_000;

/// What the docked chat and the chat page asked to open, for the next frame.
static REQUESTS: Mutex<Vec<Target>> = Mutex::new(Vec::new());

/// Open `target` next frame: a click on an SJK team's line in the docked chat or the chat
/// page.
pub(crate) fn request(target: Target) {
    let mut requests = REQUESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if requests.len() < 8 {
        requests.push(target);
    }
}

fn take_requests() -> Vec<Target> {
    std::mem::take(
        &mut *REQUESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

/// Whether build `installed` is older than release `version` (a local build, which has no
/// version, is not).
pub(crate) fn older_than(installed: &str, version: &str) -> bool {
    match (
        crate::update::parse_version(installed),
        crate::update::parse_version(version),
    ) {
        (Some(installed), Some(version)) => installed < version,
        _ => false,
    }
}

/// A picture chosen for an announcement: its bytes and type, or why there are none.
type Picked = Result<(Vec<u8>, &'static str), String>;

/// The client's side of announcements and rewards.
#[derive(Default)]
pub(crate) struct Official {
    pub(crate) window: Window,
    /// What has been shown, for the key it belongs to, and the settings folder.
    seen: Option<seen::Seen>,
    directory: PathBuf,
    /// The page was opened by the player (a click, a command): it shows over the console
    /// too, and anywhere.
    asked: bool,
    /// Later was taken: the claim window waits until the menus close and open again.
    later: bool,
    /// Rewards whose card a match gave.
    carded: Vec<u64>,
    /// The serials of the last claim, announcement and news picture outcomes followed.
    claim_serial: u64,
    announce_serial: u64,
    image_serial: u64,
    /// The picture the composer's Ctrl+I is choosing: the file dialog's thread answers
    /// with its bytes and type, or why there are none.
    picking: Option<std::sync::mpsc::Receiver<Picked>>,
    /// The rewards' revision the round list last read.
    rewards_revision: u64,
    /// Rehearsals: announcements, rewards waiting and rounds, made up offline.
    rehearsed: Vec<Announcement>,
    rehearsed_rewards: Vec<Reward>,
    rehearsed_rounds: Vec<Round>,
}

impl Official {
    /// Of the hub's `list` (newest first) for key `key_id`, the announcements not shown yet,
    /// oldest first, reading the seen file from `directory` for a new key.
    fn unshown(
        &mut self,
        directory: &Path,
        key_id: &str,
        list: &[Announcement],
    ) -> Vec<Announcement> {
        if self
            .seen
            .as_ref()
            .is_none_or(|seen| seen.key_id() != key_id)
        {
            self.seen = Some(seen::Seen::load(directory, key_id));
        }
        directory.clone_into(&mut self.directory);
        self.seen
            .as_ref()
            .map(|seen| seen.to_show(list))
            .unwrap_or_default()
    }

    /// The seen file of `key_id`, read from `directory` when another key's was held.
    fn seen_for(&mut self, directory: &Path, key_id: &str) -> &mut seen::Seen {
        if self
            .seen
            .as_ref()
            .is_none_or(|seen| seen.key_id() != key_id)
        {
            self.seen = Some(seen::Seen::load(directory, key_id));
        }
        directory.clone_into(&mut self.directory);
        self.seen.get_or_insert_with(seen::Seen::default)
    }

    fn save_seen(&self) {
        if let Some(seen) = &self.seen
            && let Err(error) = seen.save(&self.directory)
        {
            crate::log::progress(format_args!(
                "announcements: cannot write {}: {error}",
                seen::FILE
            ));
        }
    }

    /// The highest announcement `key_id` read on the News panel, given the hub's `list`
    /// ([`seen::Seen::read_given`]); `u64::MAX` while nothing can be decided yet.
    pub(crate) fn news_read(
        &mut self,
        directory: &Path,
        key_id: &str,
        list: &[Announcement],
    ) -> u64 {
        let (read, decided) = self.seen_for(directory, key_id).read_given(list);
        if decided {
            self.save_seen();
        }
        read
    }

    /// Count announcement `id` and those before it as read on the News panel.
    pub(crate) fn mark_news_read(&mut self, id: u64) {
        if self.seen.as_mut().is_some_and(|seen| seen.mark_read(id)) {
            self.save_seen();
        }
    }

    /// The rehearsed announcements (`debug_announce`), newest last.
    pub(crate) fn rehearsed(&self) -> &[Announcement] {
        &self.rehearsed
    }

    /// Count announcements `ids` as shown and write the file.
    fn mark_shown(&mut self, ids: &[u64]) {
        let (Some(seen), Some(highest)) = (self.seen.as_mut(), ids.iter().max()) else {
            return;
        };
        seen.mark(*highest);
        if let Err(error) = seen.save(&self.directory) {
            crate::log::progress(format_args!(
                "announcements: cannot write {}: {error}",
                seen::FILE
            ));
        }
    }

    /// Show `page` as the player had asked for it, for a world shot.
    #[cfg(test)]
    pub(crate) fn show_for_shot(&mut self, page: Page) {
        self.open(page, true);
    }

    /// Open `page`, asked by the player or not.
    fn open(&mut self, page: Page, asked: bool) {
        self.window.page = Some(page);
        self.asked = asked;
    }
}

/// The rounds page on round `round` (or the newest) of `rounds`.
fn rounds_page(rounds: Vec<Round>, round: Option<u64>) -> Page {
    let index = round
        .and_then(|id| rounds.iter().position(|known| known.id == id))
        .unwrap_or(0);
    let wanted = round.filter(|id| !rounds.iter().any(|known| known.id == *id));
    Page::Rounds {
        rounds,
        index,
        scroll: 0,
        wanted,
    }
}

/// A made-up announcement for `debug_announce`.
pub(crate) fn rehearsal_announcement(kind: &str, id: u64, at: i64) -> Result<Announcement, String> {
    let (release, text) = match kind {
        "" | "news" => (
            false,
            "Thank you all for testing SJK this week.\n\nWhat comes next:\n- Contributor rewards, claimed in the game\n- Announcements like this one, in SJK chat",
        ),
        "release" => (
            true,
            "A new SJK release is out.\n\n- Announcements in SJK chat\n- Contributor reward rounds with Claim",
        ),
        "picture" => (
            false,
            "A picture an announcement shows (a rehearsal has none to read, so its frame shows):\n[[image:0123456789abcdef]]\nAnd a line under it.",
        ),
        "long" => (
            false,
            "This rehearsal holds every kind of line an announcement can: a link to [your Collection](open:collection), one to [a setting](setting:cg_nameplate) and a chip, `news`.\n\n[[switch:cg_nameplate]]\n\nA paragraph that runs long enough to wrap onto several lines of the card, so the wrapping, the margins and the line height can all be checked by eye.\n\n- A first point\n- A second point that is long enough to wrap onto a second line under its bullet\n- A third\n\nOne\nTwo\nThree\nFour\nFive\nSix\nSeven\nEight\nNine\nTen\nEleven\nTwelve",
        ),
        other => {
            return Err(format!(
                "{DEBUG_ANNOUNCE_COMMAND}: {other} is none of news, release, long, picture"
            ));
        }
    };
    let images: BTreeMap<String, [u32; 2]> = if kind == "picture" {
        [("0123456789abcdef".to_owned(), [1600, 900])].into()
    } else {
        BTreeMap::new()
    };
    Ok(Announcement {
        id,
        at,
        kind: if release { "release" } else { "news" }.to_owned(),
        title: if release {
            "SJK 2099.0101.1".to_owned()
        } else {
            "Rehearsal: news from the SJK team".to_owned()
        },
        text: text.to_owned(),
        version: if release {
            "2099.0101.1".to_owned()
        } else {
            String::new()
        },
        images,
    })
}

/// The picture at `path` for an upload: its bytes and type, refused when it is too big
/// for the hub or not a PNG or a JPEG.
fn read_news_image(path: &Path) -> Picked {
    let bytes = std::fs::read(path).map_err(|error| format!("Cannot read the picture: {error}"))?;
    if bytes.len() > sjk_identity::news::NEWS_UPLOAD_MAX {
        return Err("The picture is over 3 MB: make it smaller first.".to_owned());
    }
    let mime = match image::guess_format(&bytes) {
        Ok(image::ImageFormat::Png) => "image/png",
        Ok(image::ImageFormat::Jpeg) => "image/jpeg",
        _ => return Err("Only PNG and JPEG pictures.".to_owned()),
    };
    Ok((bytes, mime))
}

/// A made-up reward and its round for `debug_reward`.
pub(crate) fn rehearsal_reward(id: u64, at: i64) -> (Reward, Round) {
    let title = "Rehearsal: Contributor rewards #0".to_owned();
    let reason = "Found and wrote up the fog that followed the camera".to_owned();
    let reward = Reward {
        id,
        round: id,
        round_title: title.clone(),
        at,
        reason: reason.clone(),
        holocrons: BTreeMap::from([("rare".to_owned(), 2), ("legendary".to_owned(), 1)]),
        credits: 500,
        shards: 25,
        claimed: None,
    };
    let person = |name: &str, reason: &str, verified| RoundPerson {
        key_id: String::new(),
        name: name.to_owned(),
        verified,
        reason: reason.to_owned(),
    };
    let mut people = vec![
        person("^2You", &reason, true),
        person("^5Creyon", "Blade signatures, round after round", true),
        person("^3Lumaya", "Wall grab camera fixes", true),
        person("", "Bug reports with logs", false),
    ];
    for index in 1..=10 {
        people.push(person(
            &format!("Tester {index}"),
            "Played the alpha and reported what broke",
            index % 2 == 0,
        ));
    }
    let round = Round {
        id,
        at,
        title,
        note: "Thank you all: SJK is better for every one of you.".to_owned(),
        people,
    };
    (reward, round)
}

impl crate::GpuState {
    /// Twice a second, with the identity's settings: put the announcements not shown yet
    /// into the SJK chat (once it can take them), and remember them as shown.
    pub(crate) fn follow_official(&mut self) {
        let Some(console) = self.console.as_ref() else {
            return;
        };
        let directory = console.config_directory().to_path_buf();
        let Some((key_id, list)) = crate::player_identity::with_snapshot(|snapshot| {
            (
                snapshot.local_key_id.clone(),
                snapshot.announcements.clone(),
            )
        }) else {
            return;
        };
        let new = self.official.unshown(&directory, &key_id, &list);
        if new.is_empty() {
            return;
        }
        let held = crate::player_identity::with_service(|service| service.post_announcements(&new))
            .unwrap_or_default();
        self.official.mark_shown(&held);
    }

    /// Open what the chat asked for, follow the hub's answers, offer a waiting reward,
    /// then say whether the window shows this frame. `others` is another card on show or
    /// waiting; a page the player asked for shows over the console too, a claim only on
    /// the main menu or the game menu.
    pub(crate) fn prepare_official(&mut self, others: bool) -> bool {
        for target in take_requests() {
            self.open_target(target);
        }
        let snapshot = crate::player_identity::with_snapshot(|snapshot| {
            (
                snapshot.rewards.clone(),
                snapshot.rewards_revision,
                snapshot.claim.clone(),
                snapshot.announce.clone(),
            )
        });
        let (rewards, rewards_revision, claim, announce) = snapshot.unwrap_or_default();
        let rewards = rewards.unwrap_or_default();
        let mut pending: Vec<(Reward, bool)> = self
            .official
            .rehearsed_rewards
            .iter()
            .map(|reward| (reward.clone(), true))
            .collect();
        pending.extend(rewards.pending.iter().map(|reward| (reward.clone(), false)));
        self.follow_claim(claim, &pending);
        self.follow_announce(announce);
        self.follow_news_image();
        // The round list follows the rounds as they are read.
        if rewards_revision != self.official.rewards_revision {
            self.official.rewards_revision = rewards_revision;
            if let Some(Page::Rounds {
                rounds,
                wanted,
                index,
                ..
            }) = self.official.window.page.as_mut()
            {
                let mut all = self.official.rehearsed_rounds.clone();
                all.extend(rewards.rounds.iter().cloned());
                if let Some(id) = wanted.take() {
                    *index = all.iter().position(|round| round.id == id).unwrap_or(0);
                }
                *rounds = all;
            }
        }
        let menu_visible = self
            .client_menu
            .as_ref()
            .is_some_and(crate::menu::ClientMenu::is_visible);
        let main_menu = self.live_session.is_none()
            && self
                .client_menu
                .as_ref()
                .is_some_and(|menu| menu.is_visible() && menu.on_main_menu());
        let place = main_menu || (self.game_menu && !menu_visible);
        if !place {
            self.official.later = false;
        }
        let console_open = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::is_open);
        if self.official.window.page.is_none()
            && !self.official.later
            && place
            && !console_open
            && let Some((reward, rehearsal)) = pending.first().cloned()
        {
            self.official.open(
                Page::Claim {
                    reward,
                    rehearsal,
                    busy: false,
                    message: String::new(),
                },
                false,
            );
        }
        // In a match, a card once per reward: the window waits for the game menu.
        if self.live_session.is_some() && !self.game_menu && !menu_visible {
            for (reward, _) in &pending {
                if !self.official.carded.contains(&reward.id) {
                    self.official.carded.push(reward.id);
                    self.unlock_toast
                        .push(crate::unlock_toast::Unlock::Reward { id: reward.id });
                }
            }
        }
        let away = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::window_away);
        let busy = self.text_dialog.is_open()
            || self.world_load_task.is_some()
            || self.world_install_task.is_some()
            || self.pending_map_reload;
        let open = self.official.window.page.is_some()
            && !others
            && !away
            && !busy
            && (self.official.asked || (place && !console_open));
        let was_open = self.official.window.is_open();
        self.official.window.set_open(open);
        if open != was_open {
            // The window takes every key and click: nothing stays held under it, and the
            // cursor is free while it shows, in a match too.
            if open {
                self.gameplay_input.release_keys();
            }
            self.sync_cursor_policy();
        }
        if open && matches!(self.official.window.page, Some(Page::Claim { .. })) {
            self.unlock_toast.withdraw(Instant::now(), |unlock| {
                matches!(unlock, crate::unlock_toast::Unlock::Reward { .. })
            });
        }
        open
    }

    /// The hub answered a claim: the claim window shows the next reward, closes, or says
    /// why it was refused.
    fn follow_claim(
        &mut self,
        claim: Option<sjk_identity::ClaimOutcome>,
        pending: &[(Reward, bool)],
    ) {
        let Some(claim) = claim.filter(|claim| claim.serial != self.official.claim_serial) else {
            return;
        };
        self.official.claim_serial = claim.serial;
        let Some(Page::Claim {
            reward,
            busy,
            message,
            ..
        }) = self.official.window.page.as_mut()
        else {
            return;
        };
        if !*busy || reward.id != claim.id {
            return;
        }
        *busy = false;
        if claim.done() {
            self.official.carded.retain(|id| *id != claim.id);
            self.next_claim(claim.id, pending);
        } else {
            message.clone_from(&claim.message);
        }
    }

    /// Show the reward waiting after `claimed`, or close the window.
    fn next_claim(&mut self, claimed: u64, pending: &[(Reward, bool)]) {
        match pending.iter().find(|(reward, _)| reward.id != claimed) {
            Some((reward, rehearsal)) => self.official.open(
                Page::Claim {
                    reward: reward.clone(),
                    rehearsal: *rehearsal,
                    busy: false,
                    message: String::new(),
                },
                self.official.asked,
            ),
            None => {
                self.official.window.page = None;
                self.official.asked = false;
            }
        }
    }

    /// The hub answered the creator's announcement: the composer closes, or says why not.
    fn follow_announce(&mut self, announce: Option<sjk_identity::ReportOutcome>) {
        let Some(outcome) =
            announce.filter(|outcome| outcome.serial != self.official.announce_serial)
        else {
            return;
        };
        self.official.announce_serial = outcome.serial;
        let Some(Page::Compose(composer)) = self.official.window.page.as_mut() else {
            return;
        };
        if !composer.busy {
            return;
        }
        composer.busy = false;
        composer.confirm = false;
        if outcome.sent {
            self.official.window.page = None;
            self.official.asked = false;
            if let Some(console) = self.console.as_mut() {
                console.push_log("announce: sent to every SJK player");
            }
        } else {
            composer.message.clone_from(&outcome.message);
        }
    }

    /// Ctrl+I in the composer: the system's file dialog on a thread of its own (it
    /// blocks), then the picture read for the upload.
    fn pick_news_image(&mut self) {
        let (answer, picked) = std::sync::mpsc::channel();
        self.official.picking = Some(picked);
        let spawned = std::thread::Builder::new()
            .name("sjk-news-picture".to_owned())
            .spawn(move || {
                let picked = crate::file_dialog::pick_file(
                    "A picture for the announcement",
                    "Pictures",
                    &["png", "jpg", "jpeg"],
                )
                .ok_or_else(|| "No picture chosen.".to_owned())
                .and_then(|path| read_news_image(&path));
                let _ = answer.send(picked);
            });
        if spawned.is_err()
            && let Some(Page::Compose(composer)) = self.official.window.page.as_mut()
        {
            composer.uploading = false;
            "The file dialog could not open.".clone_into(&mut composer.message);
        }
    }

    /// The picture chosen is sent to the hub; the hub's answer puts its line in the text.
    fn follow_news_image(&mut self) {
        if let Some(picked) = self.official.picking.as_ref()
            && let Ok(picked) = picked.try_recv()
        {
            self.official.picking = None;
            match picked {
                Ok((bytes, mime)) => {
                    let sent = crate::player_identity::with_service(|service| {
                        service.upload_news_image(bytes, mime);
                    });
                    if sent.is_none()
                        && let Some(Page::Compose(composer)) = self.official.window.page.as_mut()
                    {
                        composer.uploading = false;
                        "Turn the SJK identity on to upload pictures"
                            .clone_into(&mut composer.message);
                    } else if let Some(Page::Compose(composer)) = self.official.window.page.as_mut()
                    {
                        "Uploading the picture...".clone_into(&mut composer.message);
                    }
                }
                Err(why) => {
                    if let Some(Page::Compose(composer)) = self.official.window.page.as_mut() {
                        composer.uploading = false;
                        composer.message = why;
                    }
                }
            }
        }
        let outcome =
            crate::player_identity::with_snapshot(|snapshot| snapshot.news_image.clone()).flatten();
        let Some((serial, outcome)) =
            outcome.filter(|(serial, _)| *serial != self.official.image_serial)
        else {
            return;
        };
        self.official.image_serial = serial;
        let Some(Page::Compose(composer)) = self.official.window.page.as_mut() else {
            return;
        };
        if !composer.uploading {
            return;
        }
        match outcome {
            Ok(image) => composer.add_image(&image.image),
            Err(why) => {
                composer.uploading = false;
                composer.message = why;
            }
        }
    }

    /// Open what a line or a command names.
    fn open_target(&mut self, target: Target) {
        match target {
            Target::Announcement(id) => {
                if let Some(announcement) = self.find_announcement(Some(id)) {
                    self.open_announcement(announcement);
                }
            }
            Target::Round(id) => self.open_rounds(Some(id)),
        }
    }

    /// Announcement `id` (or the newest) as the hub listed it, the chat holds it or a
    /// rehearsal made it.
    fn find_announcement(&self, id: Option<u64>) -> Option<Announcement> {
        let listed =
            crate::player_identity::with_snapshot(|snapshot| snapshot.announcements.clone())
                .unwrap_or_default();
        let chat: Vec<Announcement> = crate::player_identity::with_chat(|chat| {
            chat.messages
                .iter()
                .filter_map(|message| match &message.official {
                    Some(Mark::Announcement(announcement)) => Some(announcement.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
        let mut all: Vec<Announcement> = listed
            .into_iter()
            .chain(chat)
            .chain(self.official.rehearsed.iter().cloned())
            .collect();
        match id {
            Some(id) => all.into_iter().find(|announcement| announcement.id == id),
            None => {
                // The newest: the hub's, else a rehearsal.
                all.sort_by_key(|announcement| (announcement.id < REHEARSAL_ID, announcement.id));
                all.pop()
            }
        }
    }

    /// Read `announcement` in full: the News panel on it.
    fn open_announcement(&mut self, announcement: Announcement) {
        self.open_news_on(&announcement);
    }

    /// Open the round list on round `round` (or the newest), reading the rounds again.
    fn open_rounds(&mut self, round: Option<u64>) {
        let mut rounds = self.official.rehearsed_rounds.clone();
        rounds.extend(
            crate::player_identity::with_snapshot(|snapshot| {
                snapshot
                    .rewards
                    .as_ref()
                    .map(|rewards| rewards.rounds.clone())
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        );
        crate::player_identity::with_service(sjk_identity::Service::refresh_rewards);
        self.official.open(rounds_page(rounds, round), true);
    }

    /// Draw the window. It darkens everything, so every font batch appended before it is
    /// dropped (text draws above all shapes).
    pub(crate) fn append_official(&mut self, viewport: [f32; 2]) {
        self.text_vertices.clear();
        self.classic_text_vertices.clear();
        self.game_fonts.clear_text();
        let target = crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        );
        self.official.window.append(target, viewport);
    }

    /// A key while the window shows.
    pub(crate) fn official_key(&mut self, event: &winit::event::KeyEvent) {
        let (shift, control) = self.console.as_ref().map_or((false, false), |console| {
            (console.shift_held(), console.control_held())
        });
        let choice = self.official.window.handle_key(event, shift, control);
        self.official_choice(choice);
    }

    /// The pointer while the window shows.
    pub(crate) fn official_pointer(&mut self, event: sjk_ui::InputEvent) {
        let choice = self.official.window.handle_pointer(event);
        self.official_choice(choice);
    }

    /// Act on the player's choice in the window.
    pub(crate) fn official_choice(&mut self, choice: Option<Choice>) {
        let Some(choice) = choice else {
            return;
        };
        match choice {
            Choice::Close => {
                self.official.window.page = None;
                self.official.asked = false;
            }
            Choice::Later => {
                self.official.window.page = None;
                self.official.asked = false;
                self.official.later = true;
            }
            Choice::Claim { id, rehearsal } => {
                if rehearsal {
                    // Nothing is sent: the rehearsal goes, the next shows.
                    self.official
                        .rehearsed_rewards
                        .retain(|reward| reward.id != id);
                    let mut pending: Vec<(Reward, bool)> = self
                        .official
                        .rehearsed_rewards
                        .iter()
                        .map(|reward| (reward.clone(), true))
                        .collect();
                    pending.extend(
                        crate::player_identity::with_snapshot(|snapshot| {
                            snapshot
                                .rewards
                                .as_ref()
                                .map(|rewards| rewards.pending.clone())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default()
                        .into_iter()
                        .map(|reward| (reward, false)),
                    );
                    self.next_claim(id, &pending);
                } else if crate::player_identity::with_service(|service| service.claim_reward(id))
                    .is_none()
                    && let Some(Page::Claim { busy, message, .. }) =
                        self.official.window.page.as_mut()
                {
                    *busy = false;
                    "Turn the SJK identity on to claim it".clone_into(message);
                }
            }
            Choice::PickImage => self.pick_news_image(),
            Choice::Send(request) => {
                if crate::player_identity::with_service(|service| service.announce(request))
                    .is_none()
                    && let Some(Page::Compose(composer)) = self.official.window.page.as_mut()
                {
                    composer.busy = false;
                    composer.confirm = false;
                    "Turn the SJK identity on to announce".clone_into(&mut composer.message);
                }
            }
        }
        // A window put away gives the cursor back at once, not on the next frame.
        if self.official.window.page.is_none() {
            self.official.window.set_open(false);
        }
        self.sync_cursor_policy();
    }

    /// A command opened the window: the console closes, so the window shows alone and
    /// closing it returns to the game (a line clicked on the chat page keeps the console,
    /// and closing the window returns to the page).
    fn close_console_for_window(&mut self) {
        if let Some(console) = &mut self.console {
            console.set_open(false);
        }
    }

    /// `announcement [number]`.
    pub(crate) fn announcement_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let id = match args.first() {
            Some(number) => Some(
                number
                    .parse::<u64>()
                    .map_err(|_| format!("{ANNOUNCEMENT_COMMAND}: {number} is not a number"))?,
            ),
            None => None,
        };
        let announcement = self.find_announcement(id).ok_or_else(|| match id {
            Some(id) => format!("{ANNOUNCEMENT_COMMAND}: no announcement {id}"),
            None => format!("{ANNOUNCEMENT_COMMAND}: no announcement yet"),
        })?;
        self.open_announcement(announcement);
        self.close_console_for_window();
        self.sync_cursor_policy();
        Ok(Vec::new())
    }

    /// `rewards`.
    pub(crate) fn rewards_command(&mut self) -> Result<Vec<String>, String> {
        self.open_rounds(None);
        self.close_console_for_window();
        self.sync_cursor_policy();
        Ok(Vec::new())
    }

    /// `announce`: the composer, for the creator only (the hub refuses anyone else too).
    pub(crate) fn announce_command(&mut self) -> Result<Vec<String>, String> {
        if !crate::player_identity::is_staff() {
            return Err(STAFF_ONLY.to_owned());
        }
        if !matches!(self.official.window.page, Some(Page::Compose(_))) {
            self.official.open(Page::Compose(Composer::default()), true);
        }
        self.official.asked = true;
        self.close_console_for_window();
        self.sync_cursor_policy();
        Ok(Vec::new())
    }

    /// `debug_announce [news|release|long]`: a made-up announcement's line in the game's
    /// chat and its card, nothing sent, nothing counted as seen.
    pub(crate) fn debug_announce_command(
        &mut self,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let kind = args.first().map_or("", String::as_str);
        let id = REHEARSAL_ID + self.official.rehearsed.len() as u64;
        let announcement =
            rehearsal_announcement(kind, id, crate::medal_popup::rehearsal::unix_now())?;
        self.chat
            .rehearse_official(Mark::Announcement(announcement.clone()), Instant::now());
        self.official.rehearsed.push(announcement.clone());
        self.open_announcement(announcement);
        self.close_console_for_window();
        self.sync_cursor_policy();
        Ok(vec![format!(
            "{DEBUG_ANNOUNCE_COMMAND}: rehearsal {id} in the game's chat; announcement {id} reads it again"
        )])
    }

    /// `debug_reward [list]`: a made-up reward waiting (its claim window on the next menu,
    /// a card in a match) and its round's line in the game's chat; `list` opens the round
    /// list. Nothing is sent.
    pub(crate) fn debug_reward_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let list = match args.first().map(String::as_str) {
            None => false,
            Some("list") => true,
            Some(other) => return Err(format!("{DEBUG_REWARD_COMMAND}: {other} is not list")),
        };
        let id = REHEARSAL_ID + self.official.rehearsed_rounds.len() as u64;
        let (reward, round) = rehearsal_reward(id, crate::medal_popup::rehearsal::unix_now());
        self.chat.rehearse_official(
            Mark::Round(FeedRound {
                id,
                round: round.id,
                at: round.at,
                title: round.title.clone(),
                count: round.people.len() as u32,
            }),
            Instant::now(),
        );
        self.official.rehearsed_rounds.insert(0, round);
        self.official.rehearsed_rewards.push(reward);
        self.official.later = false;
        if list {
            self.open_rounds(Some(id));
            self.close_console_for_window();
        }
        self.sync_cursor_policy();
        Ok(vec![format!(
            "{DEBUG_REWARD_COMMAND}: rehearsal reward {id} waits for the menus; {REWARDS_COMMAND} lists its round"
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_newer_only_with_two_versions_that_read() {
        assert!(older_than("2026.1024.1", "2026.1025.1"));
        assert!(!older_than("2026.1025.1", "2026.1025.1"));
        assert!(!older_than("2026.1026.1", "2026.1025.1"));
        assert!(!older_than("dev", "2026.1025.1"), "a local build");
        assert!(!older_than("2026.1024.1", ""));
    }

    #[test]
    fn requests_wait_for_the_next_frame() {
        let _ = take_requests();
        request(Target::Round(2));
        request(Target::Announcement(3));
        assert_eq!(take_requests(), [Target::Round(2), Target::Announcement(3)]);
        assert!(take_requests().is_empty());
    }

    #[test]
    fn rehearsals_are_made_up_far_above_the_hub() {
        for kind in ["", "news", "release", "long"] {
            let announcement = rehearsal_announcement(kind, REHEARSAL_ID, 1).unwrap();
            assert!(
                sjk_identity::news::check(&sjk_identity::AnnounceRequest {
                    release: announcement.is_release(),
                    title: announcement.title.clone(),
                    text: announcement.text.clone(),
                    version: announcement.version.clone(),
                })
                .is_ok(),
                "{kind} keeps the hub's rules"
            );
        }
        assert!(rehearsal_announcement("other", 1, 1).is_err());
        let (reward, round) = rehearsal_reward(REHEARSAL_ID, 1);
        assert_eq!(reward.round, round.id);
        assert!(round.people.len() > window::items(&reward).len());
    }

    #[test]
    fn the_round_list_opens_on_the_round_asked_for() {
        let round = |id| Round {
            id,
            ..Round::default()
        };
        let Page::Rounds { index, wanted, .. } = rounds_page(vec![round(3), round(2)], Some(2))
        else {
            panic!("the round list");
        };
        assert_eq!((index, wanted), (1, None));
        let Page::Rounds { index, wanted, .. } = rounds_page(Vec::new(), Some(9)) else {
            panic!("the round list");
        };
        assert_eq!((index, wanted), (0, Some(9)), "waits for the rounds");
    }
}
