//! Trades between players in game (`docs/sjk-ui.md`, "Trading"): the frame's side of
//! [`crate::trade_window`]. It reads the player's trades from the identity service each
//! frame, shows the toast over play while one runs and the window over the game menu,
//! and sends what the player chose.
//!
//! A trade starts from a player's card (the game chat's sender card, while the composer
//! frees the pointer, or the game menu's Players page) or the `trade` command. In game
//! the pointer is free only in the menus, so the window shows over the game menu: Esc
//! opens it while a trade runs, and its own Esc closes it and goes back to play; the
//! trade goes on and the toast says so. `trade` opens it again from anywhere.

use crate::player_identity;
use crate::trade_window::{Choice, Item, Said, View};
use std::time::{Duration, Instant};

/// The console command: `trade <player>` asks them, `trade` opens the window, `trade
/// accept|decline|cancel` answers without it.
pub(crate) const COMMAND: &str = "trade";
/// The trade window's size: 0 small, 1 normal (the default), 2 large.
pub(crate) const SIZE_CVAR: &str = "cl_tradeWindowSize";
pub(crate) const HELP: &str = "Trade holocrons and duplicate shaders with a player on this server: trade <player>; trade alone opens the trade window; trade accept, decline or cancel answers";
/// How long the toast tells of an ended trade, or of a refusal with no trade on.
const TOLD_FOR: Duration = Duration::from_secs(6);

/// The trade window, its toast, and what the player did with them.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) window: crate::trade_window::Window,
    pub(crate) toast: crate::trade_window::Toast,
    /// The trade whose window the player closed: only the toast shows for it.
    hidden: Option<i64>,
    /// The last trade seen running, to tell when it ends.
    live_seen: Option<i64>,
    /// A trade that ended while seen, and when this PC saw it end.
    ended: Option<(i64, Instant)>,
    /// The ended trade the player closed the window on.
    ended_closed: Option<i64>,
    /// The last refusal shown and when it came, by the service's step count.
    problem: Option<(u64, String, Instant)>,
    /// The toast's words this frame, when it shows.
    said: Option<Said>,
    /// The toast was drawn last frame (its targets are live).
    toast_shown: bool,
    /// The window opened the game menu (from a command or a card): closing it closes the
    /// menu.
    opened_menu: bool,
    /// The views as last built, and what from: rebuilt only when that changes (Sol's
    /// review, 24/10/2026), their clocks run on between.
    built: Option<Built>,
    /// Counts builds, so the window's copy is replaced only by a new one.
    builds: u64,
    /// The trade and build the window holds a copy of.
    window_copy: Option<(i64, u64)>,
    /// A request from a blocked player, declined without a word (Sol's decision 1).
    quiet: Option<i64>,
    /// The player was in play (a game, no game menu) last frame: the game menu opening
    /// from there is Esc in play, which brings a closed window back.
    was_in_play: bool,
    /// A world shot's trade, shown in the window (true) or the toast, with no hub.
    #[cfg(test)]
    pub(crate) shot: Option<(View, bool)>,
}

impl State {
    /// The state the next map's world takes over (`GpuState::hand_shell_to`): what the
    /// window and the toast already said goes on, so the hub's last refusal is not shown
    /// again (Creyon's screenshot, 26/10/2026). The game menu does not go on (the new
    /// world starts with it closed), so the window no longer counts as having opened it:
    /// else Esc after the map change brought the window back and Close shut the menu the
    /// player opened themselves (Sol's review of #211, 27/10/2026).
    pub(crate) fn handed_over(mut self) -> Self {
        self.opened_menu = false;
        self
    }
}

/// What the views were built from: the trades' read and step counts, the own profile's
/// and the known players' revisions, whether the window's parts (the player's items,
/// the limits) were built, and the minute (the tiles' resale countdowns) while they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Source {
    serial: u64,
    steps: u64,
    me: u64,
    players: u64,
    full: bool,
    minute: i64,
    /// The collection marks' revisions (the hub's, the seen-list's) and the blades worn
    /// (29/10/2026).
    marks: (u64, u64),
    worn: [Option<&'static str>; 2],
}

/// The views built from one [`Source`], kept between builds.
struct Built {
    source: Source,
    /// The running trade's view.
    live: Option<View>,
    /// The ended trade the player saw run, while it is still listed.
    ended: Option<View>,
    /// The hub's clock, for the views' time left.
    clock: sjk_identity::TradesClock,
    /// When the trading day resets (hub time).
    resets_at: Option<i64>,
    /// The service's step count and its last refusal.
    steps: u64,
    problem: String,
}

/// The player's own items that may go in a trade, from their profile: unopened
/// holocrons found by play and not traded within the hub's lock, and one copy of each
/// shader of the trade held twice or more, with its collection marks (`marks`). Those that
/// may go in first (a locked shader waits with the holocrons in their lock), the
/// favourites first among them, then the most worth first; those already offered left
/// out.
fn own_items(
    me: &sjk_identity::Profile,
    list: &sjk_identity::TradesAnswer,
    offered: &[Item],
    hub_now: i64,
    marks: &crate::shader_marks::Marks,
) -> Vec<Item> {
    let rules = &list.rules;
    let lock = rules.resale_lock;
    let mut items: Vec<Item> = me
        .holocrons
        .iter()
        // Holocrons only once the hub trades them (duplicates first).
        .filter(|holocron| rules.holocrons && holocron.is_sealed() && holocron.source == "play")
        .map(|holocron| {
            let value = rules
                .holocron_value
                .get(&holocron.tier)
                .copied()
                .unwrap_or(0);
            let mut item = Item::holocron(holocron.id, &holocron.tier, value);
            // Received in a trade within the hub's lock: shown, waiting its week.
            item.locked_for = holocron
                .traded
                .map(|at| at + lock - hub_now)
                .filter(|left| *left > 0);
            item
        })
        .collect();
    items.extend(
        me.unlocks
            .iter()
            .filter(|unlock| unlock.count >= 2 && unlock.medal.is_none())
            .filter_map(|unlock| {
                let skin = crate::unlockables::find(&unlock.id)?;
                crate::economy::in_trade(skin).then(|| {
                    let value = rules
                        .shader_value
                        .get(skin.tier as usize)
                        .copied()
                        .unwrap_or(0);
                    let mut item = Item::shader(&unlock.id, None, value);
                    item.favourite = marks.favourite(skin.id);
                    item.locked = marks.locked(skin.id);
                    item.new = marks.is_new(skin.id);
                    item.worn = marks.worn(skin.id);
                    item
                })
            }),
    );
    items.retain(|item| !offered.iter().any(|offered| offered.what == item.what));
    // Those that may go in first, the favourites first, most worth first; then those
    // that wait.
    items.sort_by(|a, b| {
        (!a.offerable())
            .cmp(&!b.offerable())
            .then(b.favourite.cmp(&a.favourite))
            .then(b.value.cmp(&a.value))
            .then(a.name.cmp(&b.name))
    });
    items
}

/// The day's limits as the window lists them: holocrons, Mythical holocrons and
/// duplicates, each received and given. A limit the hub does not send is left out.
fn limits(list: &sjk_identity::TradesAnswer) -> Vec<crate::trade_window::Limit> {
    let (today, rules) = (&list.today, &list.rules);
    let mut rows = Vec::new();
    // The look Creyon chose (23/10/2026): one Holocrons row, one Duplicates row. The
    // hub's one-of-each-tier rule shows only when it holds an item back.
    if rules.holocrons {
        rows.push(crate::trade_window::Limit {
            label: "Holocrons",
            received: (today.holocrons_received, rules.holocrons_received_day),
            given: (today.holocrons_given, rules.holocrons_given_day),
            colour: None,
        });
    }
    rows.push(crate::trade_window::Limit {
        label: "Duplicates",
        received: (today.duplicates_received, rules.duplicates_received_day),
        given: (today.duplicates_given, rules.duplicates_given_day),
        colour: None,
    });
    rows.retain(|row| row.received.1 > 0 || row.given.1 > 0);
    rows
}

/// How many more the player may give today past `offered`: the hub's given limits less
/// what they gave and what they offer now. Unlimited where the hub sends no limit.
fn room(list: &sjk_identity::TradesAnswer, offered: &[Item]) -> crate::trade_window::Room {
    let (today, rules) = (&list.today, &list.rules);
    let holocrons = offered
        .iter()
        .filter(|item| item.holocron_slot.is_some())
        .count() as i64;
    let shaders = offered.len() as i64 - holocrons;
    let left = |most: i64, done: i64, now: i64| {
        if most > 0 {
            most - done - now
        } else {
            i64::MAX
        }
    };
    // One of each tier: what is given today and offered now, by tier; Mythical's own
    // older limit counts too, for a hub without tiers.
    let mut tiers = [i64::MAX; 4];
    for tier in crate::holocrons::TIERS {
        let offered = offered
            .iter()
            .filter(|item| item.holocron_slot == Some(tier.index))
            .count() as i64;
        let done = today.tiers_given.get(tier.id).copied().unwrap_or(0);
        let mut most = left(rules.holocron_tier_day, done, offered);
        if tier.index == 3 {
            most = most.min(left(
                rules.mythical_given_day,
                today.mythical_given,
                offered,
            ));
        }
        if let Some(slot) = tiers.get_mut(tier.index) {
            *slot = most;
        }
    }
    crate::trade_window::Room {
        holocrons: left(rules.holocrons_given_day, today.holocrons_given, holocrons),
        tiers,
        duplicates: left(rules.duplicates_given_day, today.duplicates_given, shaders),
    }
}

/// The other player's name as they wear it, colour codes kept (white after it): the
/// hub's, else the name the game shows for their key here (`players`, the server's
/// presence); else "an SJK player". Reads only what it is given: it runs under the
/// identity snapshot's lock, which must never be taken again here (it froze the game,
/// Creyon 23/10/2026).
fn partner_shown(with: &sjk_identity::TradePartner, players: &[sjk_identity::Presence]) -> String {
    let named = |name: &str| !crate::medals::plain_note(name).trim().is_empty();
    let name = Some(with.name.clone())
        .filter(|name| named(name))
        .or_else(|| {
            players
                .iter()
                .find(|player| player.key_id == with.key_id)
                .map(|player| player.claimed_name.clone())
                .filter(|name| named(name))
        })
        .unwrap_or_else(|| "an SJK player".to_owned());
    format!("{name}^7")
}

/// Their picture's version: the hub's with the trade, else the server's word for their
/// key here (`players`); empty for none. As [`partner_shown`], it takes no lock.
fn partner_avatar(with: &sjk_identity::TradePartner, players: &[sjk_identity::Presence]) -> String {
    if !with.avatar.is_empty() {
        return with.avatar.clone();
    }
    players
        .iter()
        .find(|player| player.key_id == with.key_id)
        .map(|player| player.avatar.clone())
        .unwrap_or_default()
}

/// Ask the player whose SJK key is `key_id` to trade, from a page that is not the game's
/// (the SJK chat page): the window opens once that page closes. Why not, in the
/// console's words: the player's own key, a player blocked on this PC (as every other
/// way to ask, Sol's review of 25/10/2026), or the identity off.
pub(crate) fn ask_by_key(key_id: &str) -> Result<(), &'static str> {
    if player_identity::own_key_id().as_deref() == Some(key_id) {
        return Err("that is you");
    }
    if blocked(key_id, "") {
        return Err("you blocked this player");
    }
    player_identity::with_service(|service| service.trade_ask(key_id))
        .ok_or("the SJK identity is off (cl_identity 1)")?;
    OPEN_REQUEST.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
thread_local! {
    /// Tests: the hub lets this player trade ([`trading_open`]). Per test thread, so a
    /// test that turns it on never shows Trade on another test's card.
    pub(crate) static TRADING_FOR_TEST: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
}

/// A page outside the game asked to open the trade window ([`ask_by_key`]).
static OPEN_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The window's view of `trade`, its clocks aside ([`refresh`]): the player's items
/// (`me`, given only while the window may show), the limits and the room read from
/// `state` at the hub's `hub_now`. Runs under the identity lock, only when what it
/// reads changed.
fn build(
    trade: &sjk_identity::PlayerTrade,
    state: &sjk_identity::service::TradesState,
    me: Option<&sjk_identity::Profile>,
    players: &[sjk_identity::Presence],
    hub_now: i64,
    marks: &crate::shader_marks::Marks,
) -> View {
    let mine: Vec<Item> = trade.mine.items.iter().map(Item::from_wire).collect();
    let theirs = trade.theirs.items.iter().map(Item::from_wire).collect();
    let full = me.is_some();
    let own = me.map_or_else(Vec::new, |me| {
        own_items(me, &state.list, &mine, hub_now, marks)
    });
    let shown = partner_shown(&trade.with, players);
    View {
        partner: crate::medals::plain_note(&shown),
        partner_shown: shown,
        partner_key: trade.with.key_id.clone(),
        partner_avatar: partner_avatar(&trade.with, players),
        room: room(&state.list, &mine),
        mine,
        theirs,
        own,
        items_max: state.list.rules.items_max.max(1),
        seconds_left: 0,
        left_exact: 0.0,
        ask_secs: u64::try_from(state.list.rules.ask_secs).unwrap_or(30),
        confirm_in: None,
        limits: if full {
            limits(&state.list)
        } else {
            Vec::new()
        },
        holocrons: state.list.rules.holocrons,
        resets_in: None,
        confirming_gift: false,
        marks_kept: marks.kept,
        favourites_only: false,
        problem: String::new(),
        trade: trade.clone(),
    }
}

/// A view's clocks at `now`: the trade's time left, the confirmation's wait and the
/// trading day's reset, from the hub's `clock`.
fn refresh(
    view: &mut View,
    clock: &sjk_identity::TradesClock,
    resets_at: Option<i64>,
    now: Instant,
) {
    let trade = &view.trade;
    view.seconds_left = trade.expires.map_or(0, |at| clock.seconds_until(at, now));
    view.left_exact = trade
        .expires
        .map_or(0.0, |at| clock.seconds_until_exact(at, now));
    view.confirm_in = trade
        .confirm_from
        .filter(|_| trade.mine.ready && trade.theirs.ready)
        .map(|at| clock.seconds_until(at, now));
    view.resets_in = resets_at.map(|at| clock.seconds_until(at, now));
}

/// Whether the player blocked the player with `key_id` (wearing `name`) on this PC: their
/// requests are declined without a word, and no card offers to trade with them (Sol's
/// decision 1, 24/10/2026).
pub(crate) fn blocked(key_id: &str, name: &str) -> bool {
    crate::player_mutes::is_muted(Some(key_id), name)
}

/// Whether the game menu just opened from play (Esc in play), which brings a closed trade
/// window back: `was_in_play` last frame, the game menu up in a game now. Never in the
/// main menu, where only the toast's Open trade does.
fn esc_in_play(was_in_play: bool, game_menu: bool, in_game: bool) -> bool {
    was_in_play && game_menu && in_game
}

impl crate::GpuState {
    /// Once a frame: read the player's trades, choose what the window and the toast show.
    ///
    /// The views are built only when what they come from changes (a new list or step, the
    /// own profile, the players known, the window becoming possible), under one short
    /// identity lock; between builds only their clocks run on (Sol's review, 24/10/2026).
    pub(crate) fn update_player_trades(&mut self) {
        let now = Instant::now();
        #[cfg(test)]
        if let Some((view, window)) = self.trades.shot.clone() {
            self.trades.said = (!window).then(|| Said::of(&view));
            self.trades.window.set_view(window.then_some(view));
            return;
        }
        if OPEN_REQUEST.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.open_trade_window();
        }
        // Esc in play (the game menu opening from play) opens the trade window again. In
        // the main menu, it stays closed until the toast's Open trade, also for a request
        // that came during play (Sol's review, 25/10/2026: clearing it every frame in play
        // let such a request open over the main menu).
        let in_game = self.live_session.is_some();
        if esc_in_play(self.trades.was_in_play, self.game_menu, in_game) {
            self.trades.hidden = None;
        }
        self.trades.was_in_play = in_game && !self.game_menu;
        if let Some(slot) = self.chat.take_trade_request()
            && let Err(why) = self.trade_with_slot(slot)
            && let Some(console) = self.console.as_mut()
        {
            console.push_log(format!("^3Trade: {why}."));
        }
        if !self.read_player_trades(now) {
            let trades = &mut self.trades;
            trades.built = None;
            trades.window_copy = None;
            trades.window.set_view(None);
            trades.said = None;
            return;
        }
        let trades = &mut self.trades;
        let Some(built) = trades.built.as_mut() else {
            return;
        };
        for view in built.live.iter_mut().chain(built.ended.iter_mut()) {
            refresh(view, &built.clock, built.resets_at, now);
        }
        // A request from a blocked player is declined without a word (Sol's decision 1).
        if let Some(view) = built.live.as_ref()
            && view.trade.is_asked()
            && !view.trade.asker
            && blocked(&view.partner_key, &view.partner)
            && trades.quiet != Some(view.trade.id)
        {
            trades.quiet = Some(view.trade.id);
            let id = view.trade.id;
            player_identity::with_service(|service| {
                service.trade_step(id, sjk_identity::TradeStep::Decline);
            });
        }
        let quiet = trades.quiet;
        let built = trades.built.as_ref().expect("built above");
        let live = built
            .live
            .as_ref()
            .filter(|view| Some(view.trade.id) != quiet);
        let ended = built
            .ended
            .as_ref()
            .filter(|view| Some(view.trade.id) != quiet);
        // A new refusal is shown for a moment.
        if !built.problem.is_empty()
            && trades
                .problem
                .as_ref()
                .is_none_or(|(seen, _, _)| *seen != built.steps)
        {
            if let Some(console) = self.console.as_mut() {
                console.push_log(format!("^3Trade: {}", built.problem));
            }
            trades.problem = Some((built.steps, built.problem.clone(), now));
        }
        let fresh_problem = trades
            .problem
            .as_ref()
            .filter(|(_, _, at)| now.duration_since(*at) < TOLD_FOR)
            .map(|(_, words, _)| words.as_str())
            .unwrap_or_default();
        match live {
            Some(view) => {
                if trades.live_seen != Some(view.trade.id) {
                    // A new trade. One the player started opens its window; a request from
                    // another player shows only the toast until Open trade or Esc (Sol's
                    // review: it took over the menus and Enter accepted it).
                    trades.hidden = (!view.trade.asker).then_some(view.trade.id);
                }
                trades.live_seen = Some(view.trade.id);
            }
            None => {
                if let (Some(seen), Some(view)) = (trades.live_seen, ended)
                    && view.trade.id == seen
                {
                    trades.ended = Some((seen, now));
                    trades.live_seen = None;
                }
            }
        }
        let ended_recent = trades
            .ended
            .filter(|(_, at)| now.duration_since(*at) < TOLD_FOR)
            .map(|(id, _)| id);
        let window_view = match (live, ended) {
            (Some(view), _) => Some(view),
            // An ended trade stays in the window while it is open, until Close.
            (None, Some(view))
                if trades.window.is_open() && trades.ended_closed != Some(view.trade.id) =>
            {
                Some(view)
            }
            _ => None,
        };
        // The window keeps its copy and runs its clocks; a new build replaces it.
        match window_view {
            Some(view) if trades.window_copy == Some((view.trade.id, trades.builds)) => {
                trades.window.update_view(|shown| {
                    shown.seconds_left = view.seconds_left;
                    shown.left_exact = view.left_exact;
                    shown.confirm_in = view.confirm_in;
                    shown.resets_in = view.resets_in;
                    if shown.problem != fresh_problem {
                        fresh_problem.clone_into(&mut shown.problem);
                    }
                });
            }
            Some(view) => {
                let mut copy = view.clone();
                fresh_problem.clone_into(&mut copy.problem);
                trades.window_copy = Some((view.trade.id, trades.builds));
                trades.window.set_view(Some(copy));
            }
            None => {
                trades.window_copy = None;
                trades.window.set_view(None);
            }
        }
        trades.said = match window_view {
            Some(view) if view.trade.is_live() => Some(Said::of(view)),
            _ => None,
        }
        .or_else(|| {
            let view = ended.filter(|view| Some(view.trade.id) == ended_recent)?;
            Some(Said::of(view))
        })
        .or_else(|| {
            (!fresh_problem.is_empty()).then(|| Said {
                headline: "Trade".to_owned(),
                line: fresh_problem.to_owned(),
                clock: None,
                share: None,
                key: ("", ""),
                initial: '!',
                who: (String::new(), String::new()),
                clickable: false,
            })
        });
    }

    /// Read the player's trades from the identity service, under one lock, and build their
    /// views if what they come from changed since the last build. False when there are
    /// none to read (no identity, or no list yet).
    fn read_player_trades(&mut self, now: Instant) -> bool {
        let full = self.trade_window_may_show();
        // The blades worn, for the items' marks.
        let worn_settings = self.console.as_ref().map(|console| {
            let text = |name| console.text_cvar(name).unwrap_or_default().to_owned();
            (
                text(crate::unlockables::SABER_SKIN_CVAR),
                text(crate::looks::SECOND_CVAR),
            )
        });
        let seen_revision = crate::shader_marks::revision();
        let trades = &self.trades;
        let seen = [trades.live_seen, trades.ended.map(|(id, _)| id)];
        let last = trades.built.as_ref().map(|built| built.source);
        let read = player_identity::with_snapshot(|snapshot| {
            let state = snapshot.trades.as_ref()?;
            let hub_now = state.hub_now(now);
            let owned = snapshot
                .me
                .as_ref()
                .map_or(&[][..], |me| me.unlocks.as_slice());
            let worn = worn_settings
                .as_ref()
                .map_or([None, None], |(first, second)| {
                    crate::shader_marks::worn_of(first, second, owned)
                });
            let source = Source {
                serial: state.serial,
                steps: state.steps,
                me: snapshot.me_revision,
                players: snapshot.revision,
                full,
                minute: if full { hub_now.div_euclid(60) } else { 0 },
                marks: (snapshot.marks_revision, seen_revision),
                worn,
            };
            if last == Some(source) {
                return Some(None);
            }
            let me = snapshot.me.as_ref().filter(|_| full);
            let kept = snapshot
                .economy
                .as_ref()
                .is_some_and(|economy| economy.can(sjk_identity::MARKS_ABILITY));
            let debug = owned.iter().any(crate::debug_collectibles::added);
            let marks = crate::shader_marks::with_seen(|seen| {
                crate::shader_marks::Marks::of(
                    snapshot.marks.as_ref(),
                    kept,
                    owned,
                    seen,
                    debug,
                    worn,
                )
            });
            let live = state
                .live()
                .map(|trade| build(trade, state, me, &snapshot.players, hub_now, &marks));
            let ended = seen
                .iter()
                .flatten()
                .find_map(|id| {
                    state
                        .list
                        .trades
                        .iter()
                        .find(|trade| !trade.is_live() && trade.id == *id)
                })
                .map(|trade| build(trade, state, None, &snapshot.players, hub_now, &marks));
            Some(Some(Built {
                source,
                live,
                ended,
                clock: state.clock(),
                resets_at: state.list.today.resets_at,
                steps: state.steps,
                problem: crate::terms::say_owned(state.problem.clone()),
            }))
        })
        .flatten();
        match read {
            None => false,
            Some(None) => true,
            Some(Some(built)) => {
                self.trades.built = Some(built);
                self.trades.builds += 1;
                true
            }
        }
    }

    /// Whether the trade window could show now (the game menu in a game, or the main
    /// menu): only then are its parts (the player's items, the limits) built.
    fn trade_window_may_show(&self) -> bool {
        #[cfg(test)]
        if self.trades.shot.is_some() {
            return true;
        }
        (self.game_menu && self.live_session.is_some()) || self.main_menu_up()
    }

    /// Whether the window shows this frame: a trade to show, the player in the game menu
    /// (the pointer free) with nothing else over it, and not closed for this trade.
    pub(crate) fn prepare_trade_window(&mut self, covered: bool) -> bool {
        let console_open = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::is_open);
        let hidden = self.trades.hidden.is_some()
            && self.trades.hidden == self.trades.live_seen
            && self.trades.live_seen.is_some();
        #[cfg(test)]
        let shot = self.trades.shot.is_some();
        #[cfg(not(test))]
        let shot = false;
        let open = self.trades.window.has_view()
            && !hidden
            && ((self.game_menu && self.live_session.is_some()) || self.main_menu_up() || shot)
            && !covered
            && !console_open
            && !self.text_dialog.is_open()
            && self.world_load_task.is_none()
            && !self.pending_map_reload;
        self.trades.window.set_open(open);
        if open {
            let size = self
                .console
                .as_ref()
                .and_then(|console| console.integer_cvar(SIZE_CVAR))
                .unwrap_or(1);
            self.trades
                .window
                .set_size(crate::trade_window::Size::from_value(size));
        }
        open
    }

    /// Draw the window. It shades everything, so every font batch appended before it is
    /// dropped (text draws above all shapes).
    pub(crate) fn append_trade_window(&mut self, viewport: [f32; 2]) {
        self.text_vertices.clear();
        self.classic_text_vertices.clear();
        self.game_fonts.clear_text();
        let target = crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        );
        self.trades.window.append(target, viewport);
    }

    /// Draw the toast over play when it has something to say and the window is closed;
    /// false when it does not show. `covered` is a card or the console over the frame.
    pub(crate) fn append_trade_toast(&mut self, viewport: [f32; 2], covered: bool) -> bool {
        self.trades.toast_shown = false;
        let Some(said) = self.trades.said.clone() else {
            return false;
        };
        #[cfg(test)]
        let shot = self.trades.shot.is_some();
        #[cfg(not(test))]
        let shot = false;
        if covered
            || self.trades.window.is_open()
            || (self.live_session.is_none() && !self.main_menu_up() && !shot)
        {
            return false;
        }
        let mut said = said;
        said.clickable = self.trade_toast_clickable();
        let target = crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        );
        self.trades.toast.append(&said, target, viewport);
        self.trades.toast_shown = true;
        true
    }

    /// Whether the toast answers the pointer: it shows a running trade and the pointer is
    /// free (the game menu, or the chat's composer, is open).
    fn trade_toast_clickable(&self) -> bool {
        self.trades.live_seen.is_some()
            && (self.game_menu || self.chat.is_typing() || self.main_menu_up())
    }

    /// The main menu shows (no game): trades go on there too, from the SJK chat page.
    fn main_menu_up(&self) -> bool {
        self.live_session.is_none()
            && self
                .client_menu
                .as_ref()
                .is_some_and(crate::menu::ClientMenu::is_visible)
    }

    /// A pointer event while the toast shows: a click on it opens the trade window. True
    /// when the toast took it.
    pub(crate) fn trade_toast_pointer(&mut self, event: sjk_ui::InputEvent) -> bool {
        if !self.trades.toast_shown || !self.trade_toast_clickable() {
            return false;
        }
        if self.trades.toast.handle_pointer(event) {
            self.open_trade_window();
            return true;
        }
        // Over the toast, nothing under it takes the event.
        self.trades.toast.hovered()
            && matches!(
                event,
                sjk_ui::InputEvent::PointerPress { .. } | sjk_ui::InputEvent::PointerRelease { .. }
            )
    }

    /// A key while the window shows: a chat key (`messagemode` to `messagemode6`) opens
    /// its chat, the window staying open under the composer (#215). True when it did.
    pub(crate) fn trade_window_chat_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if event.state != winit::event::ElementState::Pressed || event.repeat {
            return false;
        }
        let Some(action) = self
            .console
            .as_ref()
            .and_then(|console| console.bound_chat_action(event))
        else {
            return false;
        };
        self.apply_input_action(Some(action));
        true
    }

    /// Whether the chat draws over the trade window this frame (after it, so the window's
    /// shade does not drop its text): in a game, with the window shown (#215).
    pub(crate) fn chat_over_trade_window(&self, trade_shown: bool) -> bool {
        trade_shown && self.live_session.is_some()
    }

    /// Act on what the player chose in the window.
    pub(crate) fn trade_window_choice(&mut self, choice: Option<Choice>) {
        match choice {
            None => {}
            Some(Choice::Step(id, step)) => {
                player_identity::with_service(|service| service.trade_step(id, step));
            }
            Some(Choice::Close) => {
                match self.trades.live_seen {
                    Some(id) => self.trades.hidden = Some(id),
                    None => self.trades.ended_closed = self.trades.ended.map(|(id, _)| id),
                }
                self.trades.window.set_view(None);
                // The window opened the menu (a card or a command): back to play. Else the
                // menu the player opened with Esc shows; its Esc goes back to play, and the
                // toast's Open trade (or Esc in play) brings the window back.
                if std::mem::take(&mut self.trades.opened_menu) && self.game_menu {
                    self.game_menu = false;
                    self.game_menu_page = crate::ingame_menu::Page::Main;
                    self.game_menu_row = 0;
                    self.in_game_menu.stop_chat_typing();
                }
                self.sync_cursor_policy();
            }
        }
    }

    /// Show the window over the game menu, opening the menu for the pointer.
    fn open_trade_window(&mut self) {
        self.trades.hidden = None;
        if self.chat.is_typing() {
            self.chat.close_composer();
        }
        if self.live_session.is_some() && !self.game_menu {
            self.release_pointer();
            self.game_menu = true;
            self.game_menu_page = crate::ingame_menu::Page::Main;
            self.game_menu_row = 0;
            self.trades.opened_menu = true;
        }
        self.sync_cursor_policy();
    }

    /// Ask the player in `slot` to trade; why not, in the player's words.
    pub(crate) fn trade_with_slot(&mut self, slot: u16) -> Result<(), String> {
        let session = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
            .ok_or_else(|| "only on an online server".to_owned())?;
        let game = session.game_state();
        if u16::try_from(game.client_num).ok() == Some(slot) {
            return Err("that is you".to_owned());
        }
        let shown = u8::try_from(slot)
            .ok()
            .and_then(|slot| {
                let name = player_identity::shown_name(game, usize::from(slot))?;
                Some((slot, name))
            })
            .ok_or_else(|| "no such player".to_owned())?;
        let mark = player_identity::hub_mark(shown.0, &shown.1)
            .ok_or_else(|| "they are not an SJK player the hub knows here".to_owned())?;
        // As the cards and the Players page: no request to a blocked player (Sol's
        // review, 25/10/2026).
        if blocked(&mark.key_id, &shown.1) {
            return Err("you blocked this player".to_owned());
        }
        player_identity::with_service(|service| service.trade_ask(&mark.key_id))
            .ok_or_else(|| "the SJK identity is off (cl_identity 1)".to_owned())?;
        self.open_trade_window();
        Ok(())
    }

    /// The `trade` command.
    pub(crate) fn trade_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let wanted = args.join(" ");
        let wanted = wanted.trim();
        let live = player_identity::with_snapshot(|snapshot| {
            let state = snapshot.trades.as_ref()?;
            state
                .live()
                .map(|trade| (trade.id, trade.asker, trade.is_asked()))
        })
        .flatten();
        let step = match (wanted, live) {
            ("", Some(_)) => {
                if let Some(console) = self.console.as_mut() {
                    console.set_open(false);
                }
                self.open_trade_window();
                return Ok(Vec::new());
            }
            ("", None) => return Err(format!("{COMMAND}: no trade on; usage: {COMMAND} <player>")),
            ("accept", Some((id, false, true))) => Some((id, sjk_identity::TradeStep::Accept)),
            ("decline", Some((id, false, true))) => Some((id, sjk_identity::TradeStep::Decline)),
            ("cancel", Some((id, ..))) => Some((id, sjk_identity::TradeStep::Cancel)),
            ("accept" | "decline" | "cancel", _) => {
                return Err(format!("{COMMAND}: no trade request to answer"));
            }
            _ => None,
        };
        if let Some((id, step)) = step {
            let accepted = step == sjk_identity::TradeStep::Accept;
            player_identity::with_service(|service| service.trade_step(id, step));
            if accepted {
                if let Some(console) = self.console.as_mut() {
                    console.set_open(false);
                }
                self.open_trade_window();
            }
            return Ok(Vec::new());
        }
        let session = self
            .live_session
            .as_ref()
            .ok_or_else(|| format!("{COMMAND}: only on a server"))?;
        let mut roster = sjk_client::ChatRoster::default();
        roster.update(session.game_state());
        let slot = match wanted.parse::<u16>() {
            Ok(slot) => slot,
            Err(_) => match roster.lookup(wanted) {
                sjk_client::PlayerLookup::Found(slot) => slot,
                sjk_client::PlayerLookup::NotFound => {
                    return Err(format!("{COMMAND}: no player matches \"{wanted}\""));
                }
                sjk_client::PlayerLookup::Ambiguous(_) => {
                    return Err(format!(
                        "{COMMAND}: \"{wanted}\" matches several players; use a number"
                    ));
                }
            },
        };
        if let Some(console) = self.console.as_mut() {
            console.set_open(false);
        }
        self.trade_with_slot(slot)
            .map(|()| Vec::new())
            .map_err(|why| format!("{COMMAND}: {why}"))
    }
}

/// Whether the SJK hub trades holocrons yet (duplicates come first): what Trade's hints
/// say it swaps.
pub(crate) fn trading_holocrons() -> bool {
    player_identity::with_snapshot(|snapshot| {
        snapshot
            .trades
            .as_ref()
            .is_some_and(|state| state.list.rules.holocrons)
    })
    .unwrap_or(false)
}

/// Whether players may trade on the SJK hub now (for this player), so a card offers
/// Trade only when it can work.
pub(crate) fn trading_open() -> bool {
    #[cfg(test)]
    if TRADING_FOR_TEST.get() {
        return true;
    }
    player_identity::with_snapshot(|snapshot| {
        let staff = snapshot.me.as_ref().is_some_and(|me| me.staff);
        snapshot
            .trades
            .as_ref()
            .is_some_and(|state| state.open(staff))
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At a map change the refusal already shown stays shown, and the window no longer
    /// owns a game menu the new world does not have (Sol's review of #211, 27/10/2026).
    #[test]
    fn a_map_change_keeps_the_shown_refusal_and_drops_the_menu_it_opened() {
        let mut state = State {
            opened_menu: true,
            ..State::default()
        };
        state.problem = Some((
            4,
            "Two accounts on one connection cannot trade with each other.".to_owned(),
            Instant::now(),
        ));
        let next = state.handed_over();
        assert!(
            !next.opened_menu,
            "the new world's menu is the player's own"
        );
        assert_eq!(
            next.problem.as_ref().map(|(steps, _, _)| *steps),
            Some(4),
            "the refusal counts as shown"
        );
    }

    /// The trade window (a request, an open trade with both sides in each of the three
    /// sizes, both ready, a gift's question) and the toast over play, over duel6 at
    /// 1080p: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer trade_shots --
    /// --ignored --nocapture`. The Trade buttons are in `duel6_ingame_players`
    /// (`players-trade`) and `duel6_sjk_chat_page` (`card-trade`).
    #[test]
    #[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
    fn trade_shots() {
        crate::world_shot::on_big_stack(|| {
            let menu = crate::menu::ClientMenu::new(true, String::new());
            let cvars = [
                ("ui_menuStyle", "sjk"),
                (crate::settings::quick::HIDE_CVAR, "1"),
            ];
            let Some((mut gpu, _profile)) =
                crate::world_shot::open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &cvars)
            else {
                return;
            };
            let _ = crate::world_shot::frame(&mut gpu, 10);
            let open = crate::trade_window::tests::open_view();
            let mut asked = open.clone();
            asked.trade.state = "asked".to_owned();
            asked.trade.asker = false;
            asked.seconds_left = 24;
            let mut ready = open.clone();
            ready.trade.mine.ready = true;
            ready.trade.theirs.ready = true;
            ready.confirm_in = Some(0);
            ready.trade.uneven = true;
            let mut gift = open.clone();
            gift.theirs.clear();
            gift.trade.theirs.items.clear();
            gift.trade.theirs.value = 0;
            for (view, window, size, name) in [
                (asked.clone(), false, "1", "trade-toast"),
                (asked, true, "1", "trade-request"),
                (open.clone(), true, "0", "trade-window-small"),
                (open.clone(), true, "1", "trade-window"),
                (open, true, "2", "trade-window-large"),
                (ready, true, "1", "trade-both-ready"),
                (gift.clone(), true, "1", "trade-gift-question"),
            ] {
                if let Some(console) = gpu.console.as_mut() {
                    console.set_cvar(SIZE_CVAR, size);
                }
                if name == "trade-gift-question" {
                    gpu.trades
                        .window
                        .ask_gift_for_shot(gift.trade.id, gift.trade.rev);
                }
                gpu.trades.shot = Some((view, window));
                gpu.update_player_trades();
                let path = crate::world_shot::shoot(&mut gpu, 6, name);
                println!("{}", path.display());
            }
        });
    }

    /// The size setting reads as a number, so Small and Large take effect (its default
    /// was the string "1", which `integer_cvar` never read: the window stayed Normal).
    #[test]
    fn the_window_size_setting_is_read() {
        let directory = tempfile::tempdir().unwrap();
        let mut console =
            crate::console::ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert_eq!(console.integer_cvar(SIZE_CVAR), Some(1));
        for (value, size) in [
            ("0", crate::trade_window::Size::Small),
            ("2", crate::trade_window::Size::Large),
        ] {
            console.set_cvar(SIZE_CVAR, value);
            let read = console.integer_cvar(SIZE_CVAR).unwrap();
            assert_eq!(crate::trade_window::Size::from_value(read), size);
        }
    }

    /// A view's clocks run from the hub's clock as read, without the list.
    #[test]
    fn a_view_keeps_its_clocks_between_builds() {
        let read_at = Instant::now();
        let clock = sjk_identity::TradesClock {
            hub_time: 1_000,
            read_at,
        };
        let mut view = crate::trade_window::tests::open_view();
        view.trade.expires = Some(1_100);
        view.trade.confirm_from = Some(1_010);
        view.trade.mine.ready = true;
        view.trade.theirs.ready = true;
        refresh(
            &mut view,
            &clock,
            Some(4_600),
            read_at + Duration::from_secs(5),
        );
        assert_eq!(view.seconds_left, 95);
        assert_eq!(view.confirm_in, Some(5));
        assert_eq!(view.resets_in, Some(3_595));
        assert!((view.left_exact - 94.5).abs() < 0.01);
        // Not both ready: no confirmation clock.
        view.trade.theirs.ready = false;
        refresh(&mut view, &clock, None, read_at);
        assert_eq!((view.confirm_in, view.resets_in), (None, None));
    }

    /// Only the game menu opening from play brings a closed window back: staying in the
    /// game menu, play itself and the main menu do not, so a request that came during play
    /// never opens over the main menu by itself (Sol's review, 25/10/2026).
    #[test]
    fn only_esc_in_play_reopens_the_window() {
        assert!(esc_in_play(true, true, true), "Esc in play");
        assert!(!esc_in_play(true, false, true), "still in play");
        assert!(!esc_in_play(false, true, true), "already in the game menu");
        assert!(!esc_in_play(true, false, false), "left for the main menu");
        assert!(!esc_in_play(false, false, false), "in the main menu");
    }

    /// Sol's decision 1: a blocked player's requests are declined without a word.
    #[test]
    fn blocked_follows_the_block_list() {
        // A key no other test blocks: the block list is the process's own.
        let key = "b10c4ed0b10c4ed2";
        assert!(!blocked(key, "Nib"));
        crate::player_mutes::set_muted(Some(key), "Nib", true);
        let was = blocked(key, "^1Nib");
        // No way to ask leaves them out: by their key, from the SJK chat page, too.
        let asked = ask_by_key(key);
        crate::player_mutes::set_muted(Some(key), "Nib", false);
        assert!(was);
        assert_eq!(asked, Err("you blocked this player"));
    }

    /// The window's parts (the player's items, the limits) are built only for a window
    /// that may show; the toast's view does without (Sol's review, 24/10/2026).
    #[test]
    fn the_toasts_view_skips_the_windows_parts() {
        let state = sjk_identity::service::TradesState {
            serial: 1,
            list: list(),
            read_at: Instant::now(),
            hub_time: 604_905,
            problem: String::new(),
            steps: 0,
        };
        let trade = crate::trade_window::tests::open_view().trade;
        let none = crate::shader_marks::Marks::default();
        let toast = build(&trade, &state, None, &[], 604_905, &none);
        assert!(toast.own.is_empty() && toast.limits.is_empty());
        let me = profile();
        let window = build(&trade, &state, Some(&me), &[], 604_905, &none);
        assert!(!window.own.is_empty() && !window.limits.is_empty());
        // The name is read once and kept both ways.
        assert_eq!(
            window.partner,
            crate::medals::plain_note(&window.partner_shown)
        );
    }

    fn profile() -> sjk_identity::Profile {
        serde_json::from_value(serde_json::json!({
            "key_id": "aa", "key": "", "name": "Sol", "bio": "", "verified": true, "created": 0,
            "holocrons": [
                {"id": 1, "tier": "rare", "source": "play"},
                {"id": 2, "tier": "mythical", "source": "play"},
                {"id": 3, "tier": "rare", "source": "staff"},
                {"id": 4, "tier": "rare", "source": "play", "opened": 5},
                {"id": 5, "tier": "legendary", "source": "play", "traded": 990},
                {"id": 6, "tier": "uncommon", "source": "play", "traded": 100}
            ],
            "unlocks": [
                {"id": "saber_tidal", "granted": 1, "count": 2},
                {"id": "saber_circuit", "granted": 1, "count": 1},
                {"id": "saber_aurora", "granted": 1, "count": 3},
                {"id": "saber_sakura", "granted": 1, "count": 2}
            ]
        }))
        .unwrap()
    }

    fn list() -> sjk_identity::TradesAnswer {
        serde_json::from_value(serde_json::json!({
            "trading": "everyone",
            "today": {"trades": 2, "holocrons_received": 1},
            "rules": {"items_max": 6, "trades_day": 10, "holocrons_received_day": 4,
                      "mythical_received_day": 1, "resale_lock": 604800,
                      "holocron_value": {"uncommon": 4, "rare": 8, "mythical": 51},
                      "shader_value": [5, 10, 25, 60, 150]}
        }))
        .unwrap()
    }

    #[test]
    fn only_unopened_found_holocrons_and_spare_copies_are_offered() {
        let none = crate::shader_marks::Marks::default();
        let items = own_items(&profile(), &list(), &[], 604_905, &none);
        let whats: Vec<_> = items.iter().map(|item| item.what.clone()).collect();
        // The Mythical first (most worth); the staff gift, the opened one and the one
        // traded within the week stay out; a single copy is no duplicate.
        assert_eq!(whats[0], crate::trade_window::What::Holocron(2));
        assert!(whats.contains(&crate::trade_window::What::Holocron(1)));
        assert!(whats.contains(&crate::trade_window::What::Holocron(6)));
        for kept in [3, 4] {
            assert!(
                !whats.contains(&crate::trade_window::What::Holocron(kept)),
                "{kept}"
            );
        }
        // The one received in a trade within the week is shown, waiting, after the rest.
        let waiting = items
            .iter()
            .find(|item| item.what == crate::trade_window::What::Holocron(5))
            .expect("shown");
        assert_eq!(waiting.locked_for, Some(990 + 604_800 - 604_905));
        assert_eq!(items.last().map(|item| &item.what), Some(&waiting.what));
        assert!(!whats.contains(&crate::trade_window::What::Shader("saber_circuit".into())));
        let tidal = crate::unlockables::find("saber_tidal").unwrap();
        assert_eq!(
            whats.contains(&crate::trade_window::What::Shader("saber_tidal".into())),
            crate::economy::in_trade(tidal)
        );
        // What is already offered is not offered twice.
        let offered = [Item::holocron(2, "mythical", 51)];
        let items = own_items(&profile(), &list(), &offered, 604_905, &none);
        assert!(items.iter().all(|item| item.what != offered[0].what));
    }

    /// Collection marks (29/10/2026): a shader's marks reach its item; a favourite leads
    /// what may go in, a locked one waits at the end with the holocrons in their lock.
    #[test]
    fn marked_shaders_are_ordered_and_a_locked_one_waits() {
        let marks = crate::shader_marks::Marks::shown(
            &["saber_sakura"],
            &["saber_aurora"],
            &["saber_sakura"],
            Some("saber_sakura"),
        );
        let items = own_items(&profile(), &list(), &[], 604_905, &marks);
        let shader = |id: &str| crate::trade_window::What::Shader(id.into());
        assert_eq!(items[0].what, shader("saber_sakura"), "the favourite first");
        assert!(items[0].favourite && items[0].new && items[0].worn && !items[0].locked);
        let aurora = items
            .iter()
            .position(|item| item.what == shader("saber_aurora"))
            .expect("listed");
        assert!(items[aurora].locked && !items[aurora].offerable());
        let first_waiting = items.iter().position(|item| !item.offerable()).unwrap();
        assert!(aurora >= first_waiting, "with those that wait");
        assert!(items[first_waiting..].iter().all(|item| !item.offerable()));
    }

    #[test]
    fn the_limits_read_received_and_given_and_room_follows_the_offer() {
        let lines = limits(&list());
        assert_eq!(lines[0].label, "Holocrons");
        assert_eq!(lines[0].received, (1, 4));
        assert!(
            lines.iter().all(|line| line.label != "Mythical"),
            "within the 4"
        );
        let mut list = list();
        list.rules.holocrons_given_day = 4;
        list.rules.mythical_given_day = 1;
        list.rules.duplicates_given_day = 10;
        list.today.holocrons_given = 2;
        let offered = [
            Item::holocron(1, "rare", 8),
            Item::holocron(2, "mythical", 51),
        ];
        let room = room(&list, &offered);
        assert_eq!((room.holocrons, room.tiers[3], room.duplicates), (0, 0, 10));
        assert!(!room.takes(&Item::holocron(9, "uncommon", 4)));
        // No limit sent: nothing held back.
        let open = super::room(&super::tests::list(), &[]);
        assert!(open.takes(&Item::holocron(9, "mythical", 51)));
    }
}
