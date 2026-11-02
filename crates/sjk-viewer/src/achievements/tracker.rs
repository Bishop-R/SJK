//! What the client sees the player do in a match, for the achievements it counts
//! (`achievements.rs`): players defeated (and how, and in a duel), the player's own
//! deaths for streaks, flags captured, the maps and servers played, the time played,
//! duels won flawless or by a hair, the players duelled, long saber throws, thermal
//! detonators sent back and time hidden in the cardboard box; and the toys' own, from
//! what the toys tell it of the moment ([`Toys`]): the lifter of the player's box
//! defeated at once, a player the Hax steers defeating others or falling, the player the
//! Drone watched defeated after it landed, and the boombox's listeners. Only in a live
//! match on a server: never in a demo, a game this client hosts, or while following
//! someone else.
//!
//! Kills come from the obituaries the client already decodes (`EV_OBITUARY`, its
//! attacker, victim and means of death); captures from the player's own
//! `persistant[PERS_CAPTURES]` rising; duels from the player state's private duel
//! (`duelInProgress`, `duelIndex`) or a duel game type. What the player took in a duel is
//! their own health and shield as the snapshots show them ([`Watch`]); a saber throw is
//! a saber kill while the player's saber is in flight (`saberInFlight`) and the victim
//! far away; a thermal detonator's thrower is the player it left ([`Thermal`]), as the
//! game never sends a missile's owner (`r.ownerNum` stays on the server).

use super::{Counter, Record};
use sjk_client::{LegacyClientInfo, ObituaryEvent};
use sjk_protocol::{EntityState, GameState, PlayerState, Snapshot};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// `persistant[PERS_CAPTURES]`.
const PERS_CAPTURES: usize = 14;
/// `PMF_FOLLOW`: the view is someone else's.
const PMF_FOLLOW: u16 = 0x1000;
/// `PM_INTERMISSION`.
const PM_INTERMISSION: u8 = 7;
/// `CS_PLAYERS`.
const CS_PLAYERS: usize = 1131;
/// `GT_DUEL` and `GT_POWERDUEL`; from `GT_TEAM` on, players have teams.
const GT_DUEL: i32 = 3;
const GT_POWERDUEL: i32 = 4;
const GT_TEAM: i32 = 6;
/// Longest gap between two snapshots counted as play (a stall is not play).
const PLAY_STEP_MAX_MS: u64 = 1_000;
/// Most captures one snapshot may add (a jump past it is a reset, not play).
const CAPTURE_STEP_MAX: u32 = 3;
/// Client numbers below this are players (the obituaries' bound).
const PLAYERS: u16 = 64;
/// `ET_PLAYER`, `ET_MISSILE`, `WP_THERMAL`, `EF_DEAD`, `EF_NODRAW`.
const ET_PLAYER: u8 = 1;
const ET_MISSILE: u8 = 3;
const WP_THERMAL: u8 = 12;
const EF_DEAD: u32 = 1 << 1;
const EF_NODRAW: u32 = 1 << 8;

/// How close another player must be for time in the cardboard box to count (game units,
/// from origin to origin: about 28 metres, within sight of the box across a room or a
/// courtyard; 512 until 26/10/2026, which asked players to stand almost on top of it).
pub(crate) const BOX_NEAR: f32 = 1024.0;
/// The shortest saber throw Long Throw takes (game units, from the player to the victim).
/// A swing reaches under 100; a thrown saber flies up to 700 per Saber Throw level
/// (`SABER_MAX_THROW_DISTANCE`).
pub(crate) const SABER_THROW_FAR: f32 = 512.0;
/// `fd.saberAnimLevel`'s styles (`SS_FAST` to `SS_STAFF`, `bg_public.h`): Desann's and
/// Tavion's (4, 5) count towards none.
const SS_FAST: u8 = 1;
const SS_MEDIUM: u8 = 2;
const SS_STRONG: u8 = 3;
const SS_DUAL: u8 = 6;
const SS_STAFF: u8 = 7;
/// Close Call: a duel won with less health than this left.
pub(crate) const CLOSE_CALL_HEALTH: i32 = 5;
/// How near a player a new thermal detonator must be first seen to be theirs (game
/// units, from the player's origin to where it left: their muzzle is about 50 away).
const THROWER_REACH: f32 = 96.0;
/// How fresh a thermal detonator's flight must be when it is first seen to name its
/// thrower (its trajectory's start, against the snapshot's time): an older one was not
/// seen leaving a hand.
const THROW_FRESH_MS: i32 = 250;
/// How long a thermal detonator gone (gone off) still answers for a kill.
const THERMAL_WINDOW: Duration = Duration::from_secs(1);
/// How long a private duel's record waits for its obituary after the duel ends.
const DUEL_LINGER: Duration = Duration::from_secs(2);
/// Surprise!: how soon after the player's box was lifted its lifter must fall to them
/// (the lift reaches the boxed player through the hub, a moment after it was made).
pub(crate) const BOX_AMBUSH: Duration = Duration::from_secs(3);
/// Eye in the Sky: how soon after the Drone landed the player it watched must fall.
pub(crate) const DRONE_STRIKE: Duration = Duration::from_secs(10);

/// `meansOfDeath_t` values (`sjk_game_jka::means_of_death`).
mod means {
    pub(super) const SABER: u8 = 3;
    pub(super) const THERMAL: u8 = 23;
    pub(super) const THERMAL_SPLASH: u8 = 24;
    pub(super) const FORCE_DARK: u8 = 31;
    pub(super) const WATER: u8 = 33;
    pub(super) const SLIME: u8 = 34;
    pub(super) const LAVA: u8 = 35;
    pub(super) const CRUSH: u8 = 36;
    pub(super) const FALLING: u8 = 38;
    pub(super) const TRIGGER_HURT: u8 = 41;
}

/// The kind of weapon a means of death belongs to, for Arsenal: one bit each.
pub(crate) fn weapon_kind(means: u8) -> Option<u32> {
    Some(match means {
        1 => 0,        // stun baton
        2 => 1,        // melee
        3 => 2,        // saber
        4 | 5 => 3,    // bryar pistol
        6 | 7 => 4,    // blaster (and the turret's)
        8..=10 => 5,   // disruptor
        11 => 6,       // bowcaster
        12..=14 => 7,  // repeater
        15 | 16 => 8,  // DEMP 2
        17 | 18 => 9,  // flechette
        19..=22 => 10, // rocket launcher
        23 | 24 => 11, // thermal detonator
        25 | 26 => 12, // trip mine
        27 => 13,      // detonation pack
        29 | 30 => 14, // concussion rifle
        _ => return None,
    })
}

/// How a kill by the player was made, beyond its means of death.
#[derive(Clone, Copy, Debug, Default)]
struct Kill {
    /// It won a duel; with what the player took in it.
    duel: Option<Watch>,
    /// A saber kill while the player's saber flew, from at least [`SABER_THROW_FAR`].
    long_throw: bool,
    /// The victim's own thermal detonator, sent back.
    returned: bool,
    /// The saber style the player fought in (`fd.saberAnimLevel`) when it was made.
    style: u8,
}

/// The count a duel won with the saber in `style` adds to.
fn style_counter(style: u8) -> Option<Counter> {
    Some(match style {
        SS_FAST => Counter::DuelStyleBlue,
        SS_MEDIUM => Counter::DuelStyleYellow,
        SS_STRONG => Counter::DuelStyleRed,
        SS_DUAL => Counter::DuelStyleDual,
        SS_STAFF => Counter::DuelStyleStaff,
        _ => return None,
    })
}

/// What a kill by the player counts towards.
fn count_kill(record: &mut Record, means: u8, kill: Kill, streak: u64) {
    record.add(Counter::Kills, 1);
    record.raise(Counter::BestStreak, streak);
    match means {
        means::SABER => record.add(Counter::SaberKills, 1),
        means::FORCE_DARK => record.add(Counter::DarkSideKills, 1),
        _ if hazard(means) => record.add(Counter::LedgeKills, 1),
        _ => {}
    }
    if let Some(bit) = weapon_kind(means) {
        record.weapons |= 1 << bit;
    }
    if let Some(watch) = kill.duel {
        record.add(Counter::DuelWins, 1);
        if !watch.hurt {
            record.add(Counter::DuelFlawless, 1);
        }
        if watch.health > 0 && watch.health < CLOSE_CALL_HEALTH {
            record.add(Counter::DuelCloseCall, 1);
        }
        // A duel, private (saber only or full Force) or a duel game's, won with the saber.
        if means == means::SABER
            && let Some(counter) = style_counter(kill.style)
        {
            record.add(counter, 1);
        }
    }
    if kill.long_throw {
        record.add(Counter::SaberThrowFar, 1);
    }
    if kill.returned {
        record.add(Counter::ThermalReturn, 1);
    }
}

/// Whether a means of death is a fall, a pit or a hazard (Watch Your Step, Leap of Faith).
fn hazard(means: u8) -> bool {
    matches!(
        means,
        means::WATER
            | means::SLIME
            | means::LAVA
            | means::CRUSH
            | means::FALLING
            | means::TRIGGER_HURT
    )
}

/// What the toys tell the tracker at a snapshot (`GpuState::toy_moments`): what its
/// obituaries and minutes are weighed against for the toys' achievements.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Toys {
    /// The player who lifted the player's box, and when that reached this client.
    pub(crate) lifted_by: Option<(u16, Instant)>,
    /// The player the Hax steers now.
    pub(crate) steering: Option<u16>,
    /// The player the Drone last watched, and when it landed (the peek ended).
    pub(crate) landed: Option<(u16, Instant)>,
    /// The SJK players in reach of the player's boombox while it plays (bits by slot).
    pub(crate) listeners: u64,
}

/// What the player took during a fight, from their own health and shield in the
/// snapshots: whether either fell, and the last health seen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Watch {
    hurt: bool,
    health: i32,
    armor: i32,
}

impl Watch {
    fn of(player: &PlayerState) -> Self {
        Self {
            hurt: false,
            health: player.health(),
            armor: player.armor(),
        }
    }

    /// The next snapshot: health or shield lower is damage, unless it was above the
    /// maximum health, which the game wears down by itself.
    fn see(&mut self, player: &PlayerState) {
        let max = player.max_health().max(1);
        if (player.health() < self.health && self.health <= max)
            || (player.armor() < self.armor && self.armor <= max)
        {
            self.hurt = true;
        }
        self.health = player.health();
        self.armor = player.armor();
    }
}

/// A private duel: the opponent, what the player took since it began, and when it
/// ended (the obituary that ends it may come with the snapshot after).
#[derive(Clone, Copy, Debug)]
struct Duel {
    with: u16,
    watch: Watch,
    ended: Option<Instant>,
}

/// A thermal detonator seen in flight: its entity, the player it left when that was
/// seen ([`THROWER_REACH`], [`THROW_FRESH_MS`]), and when it went (gone off, or out of
/// sight).
#[derive(Clone, Copy, Debug)]
struct Thermal {
    entity: u16,
    thrower: Option<u16>,
    gone: Option<Instant>,
}

fn is_thermal(entity: &EntityState) -> bool {
    entity.entity_type() == ET_MISSILE && entity.weapon() == WP_THERMAL
}

/// A player entity the game draws alive.
fn standing_player(entity: &EntityState) -> bool {
    entity.entity_type() == ET_PLAYER
        && entity.number() < PLAYERS
        && entity.e_flags() & (EF_DEAD | EF_NODRAW) == 0
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Where the player is, to tell one match from the next.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Place {
    server: SocketAddr,
    map: String,
    client: u16,
}

/// The tracker's state between snapshots.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    place: Option<Place>,
    /// Players defeated since the player last fell.
    streak: u64,
    /// The private duel the snapshots show, or showed a moment ago.
    duel: Option<Duel>,
    /// What the player took since they last spawned (a duel game's round).
    life: Option<Watch>,
    /// The player's saber was in flight at the last snapshot.
    saber_flying: bool,
    /// Where the players stood at the last snapshot, by client number.
    positions: Vec<(u16, [f32; 3])>,
    /// Thermal detonators in flight, or gone a moment ago.
    thermals: Vec<Thermal>,
    /// The captures and team the last snapshot showed, to count rises.
    captures: Option<(u32, u8)>,
    /// When the last snapshot that counted as play was seen.
    last_play: Option<Instant>,
    /// When the last snapshot that counted as time in the box was seen.
    last_hide: Option<Instant>,
    /// The player has been in the game (not spectating) on this map.
    joined: bool,
    /// The lift of the player's box, and the Drone's landing, that already counted.
    ambushed: Option<Instant>,
    struck: Option<Instant>,
    /// The player the Hax steers and how many they defeated in this hack.
    hack: Option<(u16, u64)>,
    /// When the last snapshot that counted as time steering, and with the boombox
    /// heard, was seen.
    last_steer: Option<Instant>,
    last_music: Option<Instant>,
}

/// One live snapshot as the tracker needs it.
pub(crate) struct Seen<'a> {
    pub(crate) snapshot: &'a Snapshot,
    pub(crate) game_state: &'a GameState,
    pub(crate) server: SocketAddr,
    /// A game this client hosts: nothing counts.
    pub(crate) local: bool,
    /// The obituaries this snapshot brought, oldest first.
    pub(crate) obituaries: &'a [ObituaryEvent],
    /// The player hides in the cardboard box (`cardboard_box.rs`: on, alive, playing).
    pub(crate) boxed: bool,
    /// What the toys tell of the moment.
    pub(crate) toys: Toys,
    pub(crate) now: Instant,
}

impl Tracker {
    /// The session ended: the next one starts afresh.
    pub(crate) fn leave(&mut self) {
        *self = Self::default();
    }

    /// Count what `seen` shows; whether the record changed.
    pub(crate) fn observe(&mut self, seen: &Seen<'_>, record: &mut Record) -> bool {
        // A server on this machine is one the player runs: Create game and `devmap`
        // start `sjk-server` on 127.0.0.1, a new port each time, so each one read as
        // another server (Server Hopper by devmap, #165) and its bots as kills.
        if seen.local || seen.server.ip().is_loopback() {
            self.leave();
            return false;
        }
        let game = seen.game_state;
        let Ok(me) = u16::try_from(game.client_num) else {
            return false;
        };
        let info = game.config_string(0).map(LegacyClientInfo::new);
        let map = info
            .and_then(|info| info.bytes("mapname"))
            .map(|bytes| String::from_utf8_lossy(bytes).to_ascii_lowercase())
            .unwrap_or_default();
        let gametype = info
            .and_then(|info| info.integer("g_gametype"))
            .unwrap_or(0);
        let place = Place {
            server: seen.server,
            map,
            client: me,
        };
        if self.place.as_ref() != Some(&place) {
            // A new map, server or slot: streaks, baselines and the join start again.
            *self = Self {
                place: Some(place.clone()),
                ..Self::default()
            };
        }
        let snapshot = seen.snapshot;
        let player = &snapshot.player;
        let own_view = player.client_num() == me && player.movement_flags() & PMF_FOLLOW == 0;
        let playing = own_view && !player.is_spectator();
        let duel_game = matches!(gametype, GT_DUEL | GT_POWERDUEL);
        let mut changed = false;

        self.watch_fights(player, own_view, playing, snapshot.server_time, seen.now);
        self.watch_thermals(snapshot, me, playing, seen.now);
        let saber_flew = self.saber_flying || (own_view && player.saber_in_flight());

        for event in seen.obituaries {
            if event.target == me {
                self.streak = 0;
                // A duel lost still counts its partner.
                if let Some(duel) = self.duel.take() {
                    changed |= remember_partner(record, game, duel.with);
                } else if duel_game && event.attacker < PLAYERS && event.attacker != me {
                    changed |= remember_partner(record, game, event.attacker);
                }
                continue;
            }
            if event.target >= PLAYERS {
                continue;
            }
            let private = self.duel.filter(|duel| duel.with == event.target);
            if event.attacker != me {
                // The opponent fell to another hand (or the world): the duel is over.
                if private.is_some() {
                    self.duel = None;
                    changed |= remember_partner(record, game, event.target);
                }
                continue;
            }
            if gametype >= GT_TEAM && same_team(game, me, event.target) {
                continue;
            }
            let duel = match private {
                Some(duel) => Some(duel.watch),
                None if duel_game => Some(self.life.unwrap_or_default()),
                None => None,
            };
            if duel.is_some() {
                self.duel = self.duel.filter(|duel| duel.with != event.target);
                remember_partner(record, game, event.target);
            }
            let long_throw = event.means_of_death == means::SABER
                && saber_flew
                && self
                    .position_of(snapshot, event.target)
                    .is_some_and(|at| distance(at, player.origin()) >= SABER_THROW_FAR);
            let returned = matches!(event.means_of_death, means::THERMAL | means::THERMAL_SPLASH)
                && self.sent_back(event.target, me);
            self.streak += 1;
            let kill = Kill {
                duel,
                long_throw,
                returned,
                style: player.saber_style(),
            };
            count_kill(record, event.means_of_death, kill, self.streak);
            changed = true;
        }
        self.saber_flying = own_view && player.saber_in_flight();
        self.positions.clear();
        self.positions.extend(
            snapshot
                .entities
                .iter()
                .filter(|entity| entity.entity_type() == ET_PLAYER && entity.number() < PLAYERS)
                .map(|entity| (entity.number(), entity.trajectory_base())),
        );

        if own_view {
            let captures = player.persistent[PERS_CAPTURES] & 0xffff;
            let team = player.team();
            if let Some((before, before_team)) = self.captures
                && before_team == team
                && captures > before
                && captures - before <= CAPTURE_STEP_MAX
            {
                record.add(Counter::Captures, u64::from(captures - before));
                changed = true;
            }
            self.captures = Some((captures, team));
        }

        let in_match = playing && player.movement_type() != PM_INTERMISSION;
        if in_match && !self.joined {
            self.joined = true;
            changed |= Record::remember(&mut record.maps, &place.map);
            changed |= Record::remember(&mut record.servers, &place.server.to_string());
        }
        if let Some(ms) = step(&mut self.last_play, in_match, seen.now) {
            record.play(ms);
            changed = true;
        }
        let hiding = in_match && seen.boxed && someone_near(snapshot, me, player.origin());
        if let Some(ms) = step(&mut self.last_hide, hiding, seen.now) {
            record.hide(ms);
            changed = true;
        }
        changed |= self.watch_toys(seen, record, me, in_match);
        changed
    }

    /// Count the toys' achievements of `seen`: its obituaries against what the toys tell
    /// ([`Toys`]), and the minutes steering and with the boombox heard.
    fn watch_toys(
        &mut self,
        seen: &Seen<'_>,
        record: &mut Record,
        me: u16,
        in_match: bool,
    ) -> bool {
        let toys = seen.toys;
        let mut changed = false;
        // A hack's tally lasts while the same player is steered.
        self.hack = match (self.hack, toys.steering) {
            (Some((slot, kills)), Some(steered)) if slot == steered => Some((slot, kills)),
            (_, steered) => steered.map(|slot| (slot, 0)),
        };
        for event in seen.obituaries {
            let (killer, victim) = (event.attacker, event.target);
            if victim >= PLAYERS {
                continue;
            }
            // By their own hand, or the world's.
            let own_hand = killer == victim || killer >= PLAYERS;
            if killer == me && victim != me {
                if let Some((lifter, at)) = toys.lifted_by
                    && lifter == victim
                    && seen.now.saturating_duration_since(at) <= BOX_AMBUSH
                    && self.ambushed != Some(at)
                {
                    self.ambushed = Some(at);
                    record.add(Counter::BoxAmbush, 1);
                    changed = true;
                }
                if let Some((watched, at)) = toys.landed
                    && watched == victim
                    && seen.now.saturating_duration_since(at) <= DRONE_STRIKE
                    && self.struck != Some(at)
                {
                    self.struck = Some(at);
                    record.add(Counter::DroneStrikes, 1);
                    changed = true;
                }
            }
            if let Some((steered, kills)) = &mut self.hack {
                if killer == *steered && victim != *steered && victim != me {
                    *kills += 1;
                    record.raise(Counter::HaxBest, *kills);
                    changed = true;
                }
                if victim == *steered && own_hand && hazard(event.means_of_death) {
                    record.add(Counter::HaxFalls, 1);
                    changed = true;
                }
            }
            if victim != me && own_hand && toys.listeners & 1 << victim != 0 {
                record.add(Counter::BoomboxToughCrowd, 1);
                changed = true;
            }
        }
        let steering = in_match && toys.steering.is_some();
        if let Some(ms) = step(&mut self.last_steer, steering, seen.now) {
            record.time(Counter::HaxMinutes, ms);
            changed = true;
        }
        let heard = in_match && toys.listeners != 0;
        if let Some(ms) = step(&mut self.last_music, heard, seen.now) {
            record.time(Counter::BoomboxMinutes, ms);
            changed = true;
        }
        let crowd = u64::from(toys.listeners.count_ones());
        if heard && crowd > record.count(Counter::BoomboxCrowd) {
            record.raise(Counter::BoomboxCrowd, crowd);
            changed = true;
        }
        changed
    }

    /// Follow what the player takes: in the private duel the snapshot shows (its
    /// countdown sets health and shield, it is no damage), and in this life.
    fn watch_fights(
        &mut self,
        player: &PlayerState,
        own_view: bool,
        playing: bool,
        server_time: i32,
        now: Instant,
    ) {
        if own_view && player.duel_in_progress() {
            let with = player.duel_index();
            match &mut self.duel {
                Some(duel) if duel.with == with && duel.ended.is_none() => {
                    if server_time < player.duel_time() {
                        duel.watch = Watch::of(player);
                    } else {
                        duel.watch.see(player);
                    }
                }
                _ => {
                    self.duel = Some(Duel {
                        with,
                        watch: Watch::of(player),
                        ended: None,
                    });
                }
            }
        } else if let Some(duel) = &mut self.duel {
            let ended = *duel.ended.get_or_insert(now);
            if now.saturating_duration_since(ended) > DUEL_LINGER {
                self.duel = None;
            }
        }
        if playing && player.health() > 0 {
            match &mut self.life {
                Some(life) => life.see(player),
                None => self.life = Some(Watch::of(player)),
            }
        } else {
            self.life = None;
        }
    }

    /// Follow the thermal detonators in flight: the ones gone since the last snapshot,
    /// and each new one's thrower.
    fn watch_thermals(&mut self, snapshot: &Snapshot, me: u16, playing: bool, now: Instant) {
        let entities = &snapshot.entities;
        for thermal in &mut self.thermals {
            if thermal.gone.is_none()
                && !entities
                    .iter()
                    .any(|entity| entity.number() == thermal.entity && is_thermal(entity))
            {
                thermal.gone = Some(now);
            }
        }
        self.thermals.retain(|thermal| {
            thermal
                .gone
                .is_none_or(|gone| now.saturating_duration_since(gone) <= THERMAL_WINDOW)
        });
        for entity in entities.iter().filter(|entity| is_thermal(entity)) {
            if self
                .thermals
                .iter()
                .any(|thermal| thermal.entity == entity.number() && thermal.gone.is_none())
            {
                continue;
            }
            let fresh = snapshot
                .server_time
                .saturating_sub(entity.trajectory_time())
                <= THROW_FRESH_MS;
            let thrower = fresh
                .then(|| thrower(snapshot, me, playing, entity.trajectory_base()))
                .flatten();
            self.thermals.push(Thermal {
                entity: entity.number(),
                thrower,
                gone: None,
            });
        }
    }

    /// Whether a thermal detonator `victim` threw went off a moment ago, and none of
    /// the player's own: the one the game credits the player with was the victim's,
    /// pushed or batted back.
    fn sent_back(&mut self, victim: u16, me: u16) -> bool {
        let gone_of = |thermals: &[Thermal], who: u16| {
            thermals
                .iter()
                .any(|thermal| thermal.thrower == Some(who) && thermal.gone.is_some())
        };
        if !gone_of(&self.thermals, victim) || gone_of(&self.thermals, me) {
            return false;
        }
        self.thermals
            .retain(|thermal| !(thermal.thrower == Some(victim) && thermal.gone.is_some()));
        true
    }

    /// Where player `client` is: in this snapshot, else where the last one showed them.
    fn position_of(&self, snapshot: &Snapshot, client: u16) -> Option<[f32; 3]> {
        snapshot
            .entities
            .iter()
            .find(|entity| entity.number() == client && entity.entity_type() == ET_PLAYER)
            .map(EntityState::trajectory_base)
            .or_else(|| {
                self.positions
                    .iter()
                    .find(|(number, _)| *number == client)
                    .map(|(_, at)| *at)
            })
    }
}

/// The time since `last` while `on`, at most [`PLAY_STEP_MAX_MS`] (a stall is not
/// time spent); `last` follows.
fn step(last: &mut Option<Instant>, on: bool, now: Instant) -> Option<u64> {
    if !on {
        *last = None;
        return None;
    }
    let ms = last.map(|last| now.saturating_duration_since(last).as_millis() as u64);
    *last = Some(now);
    ms.filter(|ms| *ms > 0).map(|ms| ms.min(PLAY_STEP_MAX_MS))
}

/// Whether another player stands within [`BOX_NEAR`] of `origin`.
fn someone_near(snapshot: &Snapshot, me: u16, origin: [f32; 3]) -> bool {
    snapshot.entities.iter().any(|entity| {
        standing_player(entity)
            && entity.number() != me
            && distance(entity.trajectory_base(), origin) <= BOX_NEAR
    })
}

/// The player a thermal detonator leaving `from` left: the nearest within
/// [`THROWER_REACH`], the player among them while they play.
fn thrower(snapshot: &Snapshot, me: u16, playing: bool, from: [f32; 3]) -> Option<u16> {
    let own = playing.then(|| (me, distance(snapshot.player.origin(), from)));
    snapshot
        .entities
        .iter()
        .filter(|entity| standing_player(entity) && entity.number() != me)
        .map(|entity| (entity.number(), distance(entity.trajectory_base(), from)))
        .chain(own)
        .filter(|(_, far)| *far <= THROWER_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(number, _)| number)
}

/// Remember `client`, a player duelled, by the name the server shows (colours and
/// punctuation left out, `sjk_identity::normal_form`); whether they were new.
fn remember_partner(record: &mut Record, game: &GameState, client: u16) -> bool {
    let Some(name) = crate::player_identity::shown_name(game, usize::from(client)) else {
        return false;
    };
    Record::remember(&mut record.partners, &sjk_identity::normal_form(&name))
}

/// Whether players `a` and `b` are on the same team, as the server publishes it.
fn same_team(game: &GameState, a: u16, b: u16) -> bool {
    let team = |client: u16| {
        game.config_string(CS_PLAYERS + usize::from(client))
            .and_then(|bytes| LegacyClientInfo::new(bytes).integer("t"))
    };
    matches!((team(a), team(b)), (Some(x), Some(y)) if x == y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_protocol::PlayerState;
    use std::time::Duration;

    fn snapshot(player: PlayerState) -> Snapshot {
        Snapshot {
            message_sequence: 0,
            reliable_acknowledge: 0,
            server_commands: Vec::new(),
            server_time: 0,
            delta_from: None,
            flags: 0,
            area_mask: Vec::new(),
            player,
            vehicle_player: None,
            entities: Vec::new(),
            consumed_bits: 0,
        }
    }

    fn set_duel(player: &mut PlayerState, on: bool, with: u16) {
        player.set_raw_field(119, u32::from(on));
        player.set_raw_field(44, u32::from(with));
    }

    fn game(map: &str, gametype: i32, me: i32, teams: &[(usize, i32)]) -> GameState {
        let mut game = GameState::empty_local(me);
        game.replace_config_string(
            0,
            format!("\\mapname\\{map}\\g_gametype\\{gametype}").into_bytes(),
        )
        .unwrap();
        for (client, team) in teams {
            game.replace_config_string(
                CS_PLAYERS + client,
                format!("n\\P{client}\\t\\{team}").into_bytes(),
            )
            .unwrap();
        }
        game
    }

    fn kill(attacker: u16, target: u16, means: u8) -> ObituaryEvent {
        ObituaryEvent {
            target,
            attacker,
            means_of_death: means,
            message: "",
            attacker_message: None,
            local_fragged: false,
            local_was_killed: false,
            server_time: 0,
            attacker_force: 0,
        }
    }

    struct Bench {
        tracker: Tracker,
        record: Record,
        game: GameState,
        snapshot: Snapshot,
        now: Instant,
        server: SocketAddr,
        local: bool,
        boxed: bool,
        toys: Toys,
    }

    impl Bench {
        fn new(game: GameState) -> Self {
            let mut player = PlayerState::default();
            player.set_client_num(game.client_num as u16);
            player.persistent[3] = 1; // red: in the game
            let snapshot = snapshot(player);
            Self {
                tracker: Tracker::default(),
                record: Record::default(),
                game,
                snapshot,
                now: Instant::now(),
                server: "1.2.3.4:29070".parse().unwrap(),
                local: false,
                boxed: false,
                toys: Toys::default(),
            }
        }

        fn step(&mut self, obituaries: &[ObituaryEvent]) -> bool {
            self.now += Duration::from_millis(50);
            let seen = Seen {
                snapshot: &self.snapshot,
                game_state: &self.game,
                server: self.server,
                local: self.local,
                obituaries,
                boxed: self.boxed,
                toys: self.toys,
                now: self.now,
            };
            self.tracker.observe(&seen, &mut self.record)
        }
    }

    #[test]
    fn kills_count_by_how_and_streaks_end_with_a_death() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.step(&[kill(2, 5, means::SABER), kill(2, 6, means::FORCE_DARK)]);
        bench.step(&[kill(2, 7, means::FALLING), kill(9, 8, means::SABER)]);
        assert_eq!(bench.record.count(Counter::Kills), 3);
        assert_eq!(bench.record.count(Counter::SaberKills), 1);
        assert_eq!(bench.record.count(Counter::DarkSideKills), 1);
        assert_eq!(bench.record.count(Counter::LedgeKills), 1);
        assert_eq!(bench.record.count(Counter::BestStreak), 3);
        // Falling ends the streak; the best stays.
        bench.step(&[kill(5, 2, means::SABER), kill(2, 5, 12)]);
        assert_eq!(bench.record.count(Counter::BestStreak), 3);
        assert_eq!(bench.tracker.streak, 1);
        // A suicide is not a kill and ends the streak too.
        bench.step(&[kill(2, 2, means::FALLING)]);
        assert_eq!(bench.record.count(Counter::Kills), 4);
        assert_eq!(bench.tracker.streak, 0);
        assert_eq!(
            bench.record.count(Counter::Weapons),
            2,
            "saber and repeater; lightning and falls are no weapon"
        );
        assert_eq!(bench.record.count(Counter::DuelWins), 0);
    }

    #[test]
    fn teammates_do_not_count_in_team_games() {
        let mut bench = Bench::new(game("mp/ctf1", 8, 2, &[(2, 1), (5, 1), (6, 2)]));
        bench.step(&[kill(2, 5, means::SABER), kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::Kills), 1);
    }

    #[test]
    fn duels_are_won_in_a_private_duel_or_a_duel_game() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        set_duel(&mut bench.snapshot.player, true, 5);
        bench.step(&[]);
        // The duel ends in the same snapshot as the kill.
        set_duel(&mut bench.snapshot.player, false, 0);
        bench.step(&[kill(2, 5, means::SABER)]);
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelWins), 1);
        let mut duel = Bench::new(game("mp/duel6", GT_DUEL, 2, &[]));
        duel.step(&[kill(2, 4, means::SABER)]);
        assert_eq!(duel.record.count(Counter::DuelWins), 1);
    }

    /// Dante's idea (#ideas, 29/10/2026): duels won with the saber count towards the style
    /// the player fought in, and only duels, private or a duel game's.
    #[test]
    fn duels_won_with_the_saber_count_towards_the_style_fought_in() {
        let style_wins = |record: &Record| {
            [
                Counter::DuelStyleBlue,
                Counter::DuelStyleYellow,
                Counter::DuelStyleRed,
                Counter::DuelStyleDual,
                Counter::DuelStyleStaff,
            ]
            .map(|counter| record.count(counter))
        };
        let mut duel = Bench::new(game("mp/duel6", GT_DUEL, 2, &[]));
        for (target, style) in [(4, SS_FAST), (5, SS_STRONG), (6, SS_STAFF), (7, SS_DUAL)] {
            duel.snapshot.player.set_raw_field(23, u32::from(style));
            duel.step(&[kill(2, target, means::SABER)]);
        }
        // Desann's style, and a duel won with another weapon, count towards none.
        duel.snapshot.player.set_raw_field(23, 4);
        duel.step(&[kill(2, 8, means::SABER)]);
        duel.snapshot.player.set_raw_field(23, u32::from(SS_MEDIUM));
        duel.step(&[kill(2, 9, 2)]); // MOD_MELEE
        assert_eq!(style_wins(&duel.record), [1, 0, 1, 1, 1]);
        // Out of a duel the style counts nothing.
        let mut ffa = Bench::new(game("mp/ffa3", 0, 2, &[]));
        ffa.snapshot.player.set_raw_field(23, u32::from(SS_STRONG));
        ffa.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(style_wins(&ffa.record), [0; 5]);
        // A private duel counts, whatever Force it allows.
        set_duel(&mut ffa.snapshot.player, true, 6);
        ffa.step(&[]);
        ffa.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(style_wins(&ffa.record), [0, 0, 1, 0, 0]);
    }

    #[test]
    fn captures_count_their_rise_only() {
        let mut bench = Bench::new(game("mp/ctf1", 8, 2, &[]));
        bench.snapshot.player.persistent[PERS_CAPTURES] = 4;
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::Captures), 0, "the baseline");
        bench.snapshot.player.persistent[PERS_CAPTURES] = 5;
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::Captures), 1);
        // A team change re-baselines; a reset to 0 and a jump count nothing.
        bench.snapshot.player.persistent[3] = 2;
        bench.snapshot.player.persistent[PERS_CAPTURES] = 6;
        bench.step(&[]);
        bench.snapshot.player.persistent[PERS_CAPTURES] = 0;
        bench.step(&[]);
        bench.snapshot.player.persistent[PERS_CAPTURES] = 50;
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::Captures), 1);
    }

    #[test]
    fn maps_servers_and_time_count_while_playing() {
        let mut bench = Bench::new(game("mp/FFA3", 0, 2, &[]));
        for _ in 0..21 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.count(Counter::Maps), 1);
        assert_eq!(bench.record.count(Counter::Servers), 1);
        assert!(bench.record.maps.contains("mp/ffa3"));
        assert_eq!(bench.record.play_ms, 1_000);
        // Spectating and a stall add no time; following someone counts nothing.
        bench.snapshot.player.persistent[3] = 3;
        bench.step(&[]);
        bench.snapshot.player.persistent[3] = 1;
        bench.now += Duration::from_secs(30);
        bench.step(&[]);
        bench.step(&[]);
        assert_eq!(bench.record.play_ms, 1_050);
        bench.game = game("mp/ffa1", 0, 2, &[]);
        bench.snapshot.player.set_client_num(7);
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::Maps), 1, "watching someone");
        bench.snapshot.player.set_client_num(2);
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::Maps), 2);
    }

    #[test]
    fn nothing_counts_in_a_local_game() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.local = true;
        assert!(!bench.step(&[kill(2, 5, means::SABER)]));
        assert_eq!(bench.record, Record::default());
    }

    #[test]
    fn nothing_counts_on_a_server_on_this_machine() {
        // Each devmap starts sjk-server on another loopback port (#165).
        for port in [55942, 55943] {
            let mut bench = Bench::new(game("mp/ctf2", 0, 2, &[]));
            bench.server = format!("127.0.0.1:{port}").parse().unwrap();
            for _ in 0..21 {
                assert!(!bench.step(&[kill(2, 5, means::SABER)]));
            }
            assert_eq!(bench.record, Record::default());
        }
    }

    /// A player entity `number` standing at `at`.
    fn player_at(number: u16, at: [f32; 3]) -> EntityState {
        let mut entity = EntityState::zero(number, &sjk_protocol::LEGACY_ENTITY_FIELDS);
        entity.set_raw_field(8, u32::from(ET_PLAYER));
        for (field, axis) in [2, 1, 4].into_iter().zip(at) {
            entity.set_raw_field(field, axis.to_bits());
        }
        entity
    }

    /// A thermal detonator `number` that left `from` at `time`.
    fn thermal_at(number: u16, from: [f32; 3], time: i32) -> EntityState {
        let mut entity = player_at(number, from);
        entity.set_raw_field(8, u32::from(ET_MISSILE));
        entity.set_raw_field(14, u32::from(WP_THERMAL));
        entity.set_raw_field(0, time as u32);
        entity
    }

    fn set_health(player: &mut PlayerState, health: i32, armor: i32) {
        player.stats[0] = health as u32;
        player.stats[5] = armor as u32;
        player.stats[8] = 100;
    }

    #[test]
    fn a_duel_won_untouched_is_flawless_and_one_won_by_a_hair_a_close_call() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[(5, 0), (6, 0)]));
        // The countdown may set health and shield: no damage.
        bench.snapshot.server_time = 1_000;
        bench.snapshot.player.set_raw_field(118, 3_000);
        set_health(&mut bench.snapshot.player, 100, 25);
        set_duel(&mut bench.snapshot.player, true, 5);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 100, 0);
        bench.step(&[]);
        bench.snapshot.server_time = 4_000;
        bench.step(&[]);
        // The duel ends with the kill, the winner's health restored.
        set_duel(&mut bench.snapshot.player, false, 0);
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelWins), 1);
        assert_eq!(bench.record.count(Counter::DuelFlawless), 1);
        assert_eq!(bench.record.count(Counter::DuelCloseCall), 0);
        // A second duel, hurt down to 3 health before the kill.
        set_duel(&mut bench.snapshot.player, true, 6);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 40, 0);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 3, 0);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 100, 0);
        set_duel(&mut bench.snapshot.player, false, 0);
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelWins), 2);
        assert_eq!(bench.record.count(Counter::DuelFlawless), 1, "hurt");
        assert_eq!(bench.record.count(Counter::DuelCloseCall), 1);
        // A kill out of a duel is neither.
        bench.step(&[kill(2, 7, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelFlawless), 1);
        assert_eq!(bench.record.count(Counter::DuelCloseCall), 1);
    }

    #[test]
    fn health_worn_down_above_the_maximum_is_no_damage() {
        let mut bench = Bench::new(game("mp/duel6", GT_DUEL, 2, &[(4, 0)]));
        set_health(&mut bench.snapshot.player, 120, 0);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 110, 0);
        bench.step(&[]);
        bench.step(&[kill(2, 4, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelFlawless), 1);
        // A new life starts clean; a hit then spoils it.
        set_health(&mut bench.snapshot.player, 0, 0);
        bench.step(&[kill(4, 2, means::SABER)]);
        set_health(&mut bench.snapshot.player, 100, 0);
        bench.step(&[]);
        set_health(&mut bench.snapshot.player, 90, 0);
        bench.step(&[]);
        bench.step(&[kill(2, 4, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelWins), 2);
        assert_eq!(bench.record.count(Counter::DuelFlawless), 1);
    }

    #[test]
    fn duel_partners_count_each_name_once_won_or_lost() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[(5, 0), (6, 0), (7, 0)]));
        bench
            .game
            .replace_config_string(CS_PLAYERS + 7, b"n\\^1P^75\\t\\0".to_vec())
            .unwrap();
        set_duel(&mut bench.snapshot.player, true, 5);
        bench.step(&[]);
        set_duel(&mut bench.snapshot.player, false, 0);
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelPartners), 1);
        // Lost to P6: still a partner.
        set_duel(&mut bench.snapshot.player, true, 6);
        bench.step(&[]);
        bench.step(&[kill(6, 2, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelPartners), 2);
        // Client 7 shows as "P5" in colours: the same name, not a new partner.
        set_duel(&mut bench.snapshot.player, true, 7);
        bench.step(&[]);
        set_duel(&mut bench.snapshot.player, false, 0);
        bench.step(&[kill(2, 7, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelPartners), 2);
        assert!(bench.record.partners.contains("p5"));
        // A kill outside a duel is no partner; a duel game's every fight is.
        bench.step(&[kill(2, 8, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DuelPartners), 2);
        let mut duel = Bench::new(game("mp/duel6", GT_DUEL, 2, &[(4, 0), (9, 0)]));
        duel.step(&[kill(2, 4, means::SABER)]);
        duel.step(&[kill(9, 2, means::SABER)]);
        assert_eq!(duel.record.count(Counter::DuelPartners), 2);
    }

    #[test]
    fn a_saber_kill_far_away_with_the_saber_in_flight_is_a_long_throw() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.snapshot.entities = vec![
            player_at(5, [SABER_THROW_FAR + 10.0, 0.0, 0.0]),
            player_at(6, [40.0, 0.0, 0.0]),
            player_at(7, [SABER_THROW_FAR + 50.0, 0.0, 0.0]),
        ];
        // Far, but swung: the saber is in hand.
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::SaberThrowFar), 0);
        // In flight: far counts, near does not.
        bench.snapshot.player.set_raw_field(88, 1);
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::SaberThrowFar), 0);
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::SaberThrowFar), 1);
        // Back in hand in the kill's snapshot, and the victim out of sight: the last
        // snapshot's flight and place count.
        bench.snapshot.player.set_raw_field(88, 0);
        bench.snapshot.entities.clear();
        bench.step(&[kill(2, 7, means::SABER)]);
        assert_eq!(bench.record.count(Counter::SaberThrowFar), 2);
        // Another weapon in flight is no throw.
        bench.snapshot.player.set_raw_field(88, 1);
        bench.snapshot.entities = vec![player_at(5, [900.0, 0.0, 0.0])];
        bench.step(&[kill(2, 5, 12)]);
        assert_eq!(bench.record.count(Counter::SaberThrowFar), 2);
    }

    #[test]
    fn a_thermal_detonator_sent_back_to_its_thrower_is_returned() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.snapshot.server_time = 10_000;
        let victim = [600.0, 0.0, 0.0];
        // Player 5 throws one: first seen fresh, next to them.
        bench.snapshot.entities = vec![
            player_at(5, victim),
            thermal_at(200, [640.0, 0.0, 20.0], 9_950),
        ];
        bench.step(&[]);
        assert_eq!(bench.tracker.thermals[0].thrower, Some(5));
        // The player pushes it back: it flies anew from near the player, still 5's.
        bench.snapshot.entities[1] = thermal_at(200, [40.0, 0.0, 20.0], 10_050);
        bench.snapshot.server_time = 10_100;
        bench.step(&[]);
        assert_eq!(bench.tracker.thermals[0].thrower, Some(5));
        // It goes off by 5, who falls to it, credited to the player.
        bench.snapshot.entities.truncate(1);
        bench.step(&[kill(2, 5, means::THERMAL_SPLASH)]);
        assert_eq!(bench.record.count(Counter::ThermalReturn), 1);
        assert_eq!(bench.record.count(Counter::Kills), 1);
        // The player's own thermal detonator is no return.
        bench.snapshot.server_time = 20_000;
        bench.snapshot.entities = vec![
            player_at(5, victim),
            thermal_at(201, [30.0, 0.0, 20.0], 19_990),
        ];
        bench.step(&[]);
        assert_eq!(bench.tracker.thermals[0].thrower, Some(2));
        bench.snapshot.entities.truncate(1);
        bench.step(&[kill(2, 5, means::THERMAL)]);
        assert_eq!(bench.record.count(Counter::ThermalReturn), 1);
        // One first seen long after it left names no thrower.
        bench.snapshot.server_time = 30_000;
        bench.snapshot.entities = vec![
            player_at(5, victim),
            thermal_at(202, [610.0, 0.0, 0.0], 27_000),
        ];
        bench.step(&[]);
        let late = bench
            .tracker
            .thermals
            .iter()
            .find(|t| t.entity == 202)
            .unwrap();
        assert_eq!(late.thrower, None);
        bench.snapshot.entities.truncate(1);
        bench.step(&[kill(2, 5, means::THERMAL)]);
        assert_eq!(bench.record.count(Counter::ThermalReturn), 1);
        // One gone more than a second before the kill no longer answers for it.
        bench.snapshot.server_time = 40_000;
        bench.snapshot.entities = vec![
            player_at(5, victim),
            thermal_at(203, [620.0, 0.0, 0.0], 39_990),
        ];
        bench.step(&[]);
        bench.snapshot.entities.truncate(1);
        bench.step(&[]);
        bench.now += THERMAL_WINDOW + Duration::from_millis(100);
        bench.step(&[kill(2, 5, means::THERMAL)]);
        assert_eq!(bench.record.count(Counter::ThermalReturn), 1);
    }

    #[test]
    fn the_lifter_of_the_box_and_the_player_the_drone_watched_count_once_soon_after() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        let lifted = bench.now;
        bench.toys.lifted_by = Some((5, lifted));
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::BoxAmbush), 0, "not the lifter");
        bench.step(&[kill(2, 5, means::SABER)]);
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::BoxAmbush), 1, "once a lift");
        bench.toys.lifted_by = Some((5, bench.now));
        for _ in 0..61 {
            bench.step(&[]);
        }
        bench.step(&[kill(2, 5, means::SABER)]);
        assert_eq!(bench.record.count(Counter::BoxAmbush), 1, "too late");
        bench.toys.landed = Some((6, bench.now));
        for _ in 0..180 {
            bench.step(&[]);
        }
        bench.step(&[kill(5, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DroneStrikes), 0, "another hand");
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DroneStrikes), 1);
        bench.toys.landed = Some((6, bench.now));
        for _ in 0..201 {
            bench.step(&[]);
        }
        bench.step(&[kill(2, 6, means::SABER)]);
        assert_eq!(bench.record.count(Counter::DroneStrikes), 1, "past 10 s");
    }

    #[test]
    fn a_player_steered_counts_their_kills_in_one_hack_their_falls_and_the_minutes() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.toys.steering = Some(7);
        bench.step(&[kill(7, 8, means::SABER)]);
        bench.step(&[kill(7, 2, means::SABER), kill(7, 9, means::SABER)]);
        assert_eq!(
            bench.record.count(Counter::HaxBest),
            2,
            "the hacker is no prey"
        );
        bench.toys.steering = None;
        bench.step(&[kill(7, 8, means::SABER)]);
        bench.toys.steering = Some(7);
        bench.step(&[kill(7, 8, means::SABER)]);
        assert_eq!(
            bench.record.count(Counter::HaxBest),
            2,
            "a new hack starts at 0"
        );
        bench.step(&[kill(1022, 7, means::FALLING)]);
        bench.step(&[kill(7, 7, means::SABER)]);
        assert_eq!(
            bench.record.count(Counter::HaxFalls),
            1,
            "a fall, not any death"
        );
        for _ in 0..1_200 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.count(Counter::HaxMinutes), 1);
        assert_eq!(
            bench.record.count(Counter::Kills),
            0,
            "none of them the player's"
        );
    }

    #[test]
    fn the_boombox_counts_its_minutes_its_crowd_and_a_listener_who_quits() {
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.toys.listeners = 1 << 5 | 1 << 6 | 1 << 9;
        for _ in 0..1_201 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.count(Counter::BoomboxMinutes), 1);
        assert_eq!(bench.record.count(Counter::BoomboxCrowd), 3);
        bench.step(&[kill(5, 6, means::SABER), kill(4, 4, means::FALLING)]);
        assert_eq!(bench.record.count(Counter::BoomboxToughCrowd), 0);
        bench.step(&[kill(1022, 9, means::FALLING)]);
        assert_eq!(bench.record.count(Counter::BoomboxToughCrowd), 1);
        bench.toys.listeners = 1 << 5;
        bench.step(&[]);
        assert_eq!(bench.record.count(Counter::BoomboxCrowd), 3, "a best");
    }

    #[test]
    fn time_in_the_box_counts_only_with_another_player_close_by() {
        assert_eq!(BOX_NEAR, 1024.0, "about 28 metres");
        let mut bench = Bench::new(game("mp/ffa3", 0, 2, &[]));
        bench.boxed = true;
        bench.snapshot.entities = vec![player_at(5, [BOX_NEAR + 50.0, 0.0, 0.0])];
        for _ in 0..10 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.box_ms, 0, "nobody close by");
        // 900 units away: beyond the old 512, within the 1024 of 26/10/2026.
        bench.snapshot.entities = vec![player_at(5, [900.0, 0.0, 0.0])];
        for _ in 0..21 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.box_ms, 1_000);
        // A dead player close by does not count, nor time out of the box.
        bench.snapshot.entities[0].set_raw_field(19, EF_DEAD);
        bench.step(&[]);
        bench.step(&[]);
        assert_eq!(bench.record.box_ms, 1_000);
        bench.snapshot.entities[0].set_raw_field(19, 0);
        bench.boxed = false;
        bench.step(&[]);
        bench.step(&[]);
        assert_eq!(bench.record.box_ms, 1_000);
        // Whole minutes go to the count.
        bench.boxed = true;
        bench.step(&[]);
        for _ in 0..1_200 {
            bench.step(&[]);
        }
        assert_eq!(bench.record.count(Counter::BoxMinutes), 1);
        assert_eq!(bench.record.box_ms, 1_000);
    }

    #[test]
    fn every_weapon_has_its_own_kind() {
        let kinds: std::collections::BTreeSet<u32> = (0..=42).filter_map(weapon_kind).collect();
        assert_eq!(kinds.len(), 15);
        assert_eq!(weapon_kind(means::FALLING), None);
        assert_eq!(weapon_kind(means::FORCE_DARK), None);
    }
}
