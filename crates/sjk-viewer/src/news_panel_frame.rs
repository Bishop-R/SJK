//! The News panel in the frame: when it opens on its own, what it lists, and what the
//! player's choices do (`news_panel.rs`).

use super::{Choice, Entry, Key, Panel, SEEN_CVAR, Sources, entries, flip, switch_on};
use crate::console::changelog::{Digest, Review};
use sjk_identity::Announcement;

/// What the identity knows of the announcements: the key, the hub's list and the
/// rehearsals.
fn announcements(gpu: &crate::GpuState) -> (Option<String>, Vec<Announcement>) {
    let (key_id, mut list) = crate::player_identity::with_snapshot(|snapshot| {
        (
            Some(snapshot.local_key_id.clone()),
            snapshot.announcements.clone(),
        )
    })
    .unwrap_or_default();
    list.extend(gpu.official.rehearsed().iter().cloned());
    (key_id, list)
}

/// The entry of `entries` that shows `announcement`: its own, or its release's.
fn key_of(entries: &[Entry], announcement: &Announcement) -> Option<Key> {
    entries
        .iter()
        .find(|entry| match &entry.key {
            Key::Announcement(id) => *id == announcement.id,
            Key::Release(title) => {
                announcement.is_release() && super::title_version(title) == announcement.version
            }
            Key::Review => false,
        })
        .map(|entry| entry.key.clone())
}

impl crate::GpuState {
    /// The panel's entries: the recent releases (with the unreleased one for a preview),
    /// the announcements, and `reviews` when an update asks for them.
    fn news_entries(&mut self, reviews: &[Review], preview: bool) -> Vec<Entry> {
        let installed = crate::build_info::VERSION;
        let mut digests: Vec<Digest> = Vec::new();
        if preview
            && let Some(newest) = crate::console::changelog::newest_digest()
            && newest.title == "Unreleased"
        {
            digests.push(newest);
        }
        digests.extend(crate::console::changelog::recent_digests(
            installed,
            super::RELEASES,
        ));
        let seen = self
            .console
            .as_ref()
            .and_then(|console| console.text_cvar(SEEN_CVAR).ok())
            .unwrap_or_default()
            .to_owned();
        let new_releases: Vec<String> = if seen == installed {
            Vec::new()
        } else {
            crate::console::changelog::digests(installed, &seen)
                .into_iter()
                .map(|digest| digest.title)
                .collect()
        };
        let (key_id, list) = announcements(self);
        let read = match (key_id, self.console.as_ref()) {
            (Some(key_id), Some(console)) if !key_id.is_empty() => {
                let directory = console.config_directory().to_path_buf();
                self.official.news_read(&directory, &key_id, &list)
            }
            _ => u64::MAX,
        };
        entries(&Sources {
            digests: &digests,
            reviews,
            announcements: &list,
            new_releases: &new_releases,
            read,
            installed,
        })
    }

    /// Read the update marker once: an update offers the panel on its settings to review
    /// or its recap; a new installation, and a build no release lists, offer nothing.
    fn read_update_marker(&mut self) {
        let installed = crate::build_info::VERSION;
        let Some(console) = self.console.as_mut() else {
            return;
        };
        let seen = console.text_cvar(SEEN_CVAR).unwrap_or_default().to_owned();
        if seen == installed {
            return;
        }
        // A player who never saw First setup installed just now: nothing was updated.
        if seen.is_empty() && console.bool_cvar(crate::settings::quick::HIDE_CVAR) != Some(true) {
            console.set_cvar(SEEN_CVAR, installed);
            return;
        }
        let digests = crate::console::changelog::digests(installed, &seen);
        if digests.is_empty() {
            return;
        }
        super::reset_once(console, installed, &seen, crate::console::changelog::resets);
        let reviews = crate::console::changelog::reviews(installed, &seen);
        let entries = self.news_entries(&reviews, false);
        let on = entries
            .iter()
            .find(|entry| matches!(entry.key, Key::Review))
            .map(|entry| entry.key.clone())
            .unwrap_or_else(|| Key::Release(digests[0].title.clone()));
        self.news.offer(entries, Some(&on), false, installed);
    }

    /// An announcement the player has not read: the panel on it, once a run for each.
    fn offer_unread_news(&mut self) {
        let (key_id, list) = announcements(self);
        let Some(key_id) = key_id.filter(|key| !key.is_empty()) else {
            return;
        };
        let Some(directory) = self
            .console
            .as_ref()
            .map(|console| console.config_directory().to_path_buf())
        else {
            return;
        };
        let read = self.official.news_read(&directory, &key_id, &list);
        let newest = list
            .iter()
            .filter(|announcement| announcement.id > read && announcement.id > self.news.offered)
            .max_by_key(|announcement| announcement.id)
            .map(|announcement| announcement.id);
        let Some(id) = newest else {
            return;
        };
        self.news.offered = id;
        let entries = self.news_entries(&[], false);
        let on = list
            .iter()
            .find(|announcement| announcement.id == id)
            .and_then(|announcement| key_of(&entries, announcement));
        self.news.offer(entries, on.as_ref(), false, "");
    }

    /// Offer the panel on an update or unread news, then say whether it shows this
    /// frame: asked for, anywhere; else on the main menu or the game menu with nothing
    /// else over it. `covered` is another card or the console over the frame.
    pub(crate) fn prepare_news_panel(&mut self, covered: bool) -> bool {
        if self.news.take_check() {
            self.read_update_marker();
        }
        let menu_visible = self
            .client_menu
            .as_ref()
            .is_some_and(crate::menu::ClientMenu::is_visible);
        let main_menu = self
            .client_menu
            .as_ref()
            .is_some_and(|menu| menu.is_visible() && menu.on_main_menu());
        let place = main_menu || (self.game_menu && !menu_visible);
        let console_open = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::is_open);
        let away = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::window_away);
        let busy = self.text_dialog.is_open()
            || self.world_load_task.is_some()
            || self.world_install_task.is_some()
            || self.pending_map_reload;
        if !self.news.wants() && place && !covered && !console_open && !away && !busy {
            self.offer_unread_news();
        }
        let asked = self.news.asked();
        let open =
            self.news.wants() && !busy && !away && (asked || (place && !covered && !console_open));
        let was_open = self.news.is_open();
        self.news.set_open(open);
        if open != was_open {
            if open {
                self.gameplay_input.release_keys();
            }
            self.sync_cursor_policy();
        }
        open
    }

    /// Draw the panel. It darkens everything, so every font batch appended before it is
    /// dropped (text draws above all shapes).
    pub(crate) fn append_news_panel(&mut self, viewport: [f32; 2]) {
        self.text_vertices.clear();
        self.classic_text_vertices.clear();
        self.game_fonts.clear_text();
        let target = crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        );
        self.news.prepare(&target.inline_measures());
        if let Some(console) = self.console.as_ref() {
            self.news.sync_switches(|cvar| switch_on(console, cvar));
        }
        self.news.append(target, viewport);
    }

    /// A key while the panel shows.
    pub(crate) fn news_key(&mut self, event: &winit::event::KeyEvent) {
        let choice = self.news.handle_key(event);
        self.news_choice(choice);
    }

    /// The pointer while the panel shows.
    pub(crate) fn news_pointer(&mut self, event: sjk_ui::InputEvent) {
        let choice = self.news.handle_pointer(event);
        self.news_choice(choice);
    }

    /// Act on the player's choice: a switch flips and the panel stays; anything else
    /// closes it, remembering the update and the announcements as read, then opens the
    /// changelog, installs the update, or follows a chip or a link.
    pub(crate) fn news_choice(&mut self, choice: Option<Choice>) {
        let Some(choice) = choice else {
            return;
        };
        if let Choice::Switch(cvar) = &choice {
            if let Some(console) = self.console.as_mut() {
                flip(console, cvar);
            }
            return;
        }
        let version = std::mem::take(&mut self.news.version);
        if let Some(highest) = self.news.highest_listed() {
            self.official.mark_news_read(highest);
        }
        self.news.put_away();
        self.news.set_open(false);
        if let Some(console) = self.console.as_mut() {
            if !version.is_empty() {
                console.set_cvar(SEEN_CVAR, &version);
            }
            match &choice {
                Choice::Changelog => console.open_changelog(),
                Choice::Chip(text) => console.follow_chip(text),
                Choice::Link(target) => match crate::ui_links::follow(target) {
                    Some(crate::ui_links::Follow::Settings(query)) => {
                        console.search_settings(&query);
                    }
                    Some(crate::ui_links::Follow::Command(command)) => {
                        let _ = console.queue_command(command);
                    }
                    None => {}
                },
                Choice::Close | Choice::Update | Choice::Switch(_) => {}
            }
        }
        if choice == Choice::Update {
            if crate::update::available().is_some() {
                if crate::update::click_notice() == crate::update::NoticeClick::Restart {
                    self.quit_requested = true;
                }
            } else if let Some(console) = self.console.as_mut() {
                console.open_update_panel();
            }
        }
        self.sync_cursor_policy();
    }

    /// Open the panel as the player asked, on entry `on` (else the newest); a command
    /// closes the console first.
    pub(crate) fn open_news(&mut self, on: Option<Key>, preview: bool) {
        let reviews = if preview {
            crate::console::changelog::newest_reviews()
        } else {
            Vec::new()
        };
        let entries = self.news_entries(&reviews, preview);
        let on = on.or_else(|| {
            preview.then(|| {
                entries
                    .iter()
                    .find(|entry| matches!(entry.key, Key::Release(_)))
                    .map(|entry| entry.key.clone())
            })?
        });
        self.news.offer(entries, on.as_ref(), true, "");
        if let Some(console) = &mut self.console {
            console.set_open(false);
        }
        self.sync_cursor_policy();
    }

    /// The News panel on `announcement`: its own entry, or its release's.
    pub(crate) fn open_news_on(&mut self, announcement: &Announcement) {
        let entries = self.news_entries(&[], false);
        let key = key_of(&entries, announcement);
        self.news.offer(entries, key.as_ref(), true, "");
        self.sync_cursor_policy();
    }

    /// `whatsnew`: the newest release's recap, "Unreleased" included, as a preview that
    /// writes nothing; its settings to review switch.
    pub(crate) fn preview_whats_new(&mut self) {
        self.open_news(None, true);
    }

    /// `news`.
    pub(crate) fn news_command(&mut self) -> Result<Vec<String>, String> {
        self.open_news(None, false);
        Ok(Vec::new())
    }

    /// Open Settings on the setting a chip or a link named, once; it returns to the game
    /// menu in a match, else to the main menu.
    pub(crate) fn open_chip_settings(&mut self) {
        let Some(query) = self
            .console
            .as_mut()
            .and_then(crate::console::ViewerConsole::take_settings_search)
        else {
            return;
        };
        let target = if self.live_session.is_some() {
            crate::player_menu::ReturnTarget::InGame
        } else {
            crate::player_menu::ReturnTarget::MainMenu
        };
        if let (Some(menu), Some(console)) = (&mut self.client_menu, &self.console) {
            menu.open_settings_search(console, &query, target);
            self.game_menu = false;
        }
        self.sync_cursor_policy();
    }
}

impl Panel {
    /// The update's version this panel would write on Close (tests).
    #[cfg(test)]
    pub(crate) fn version(&self) -> &str {
        &self.version
    }
}
