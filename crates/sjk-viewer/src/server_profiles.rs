//! The game module each server ran when this client last joined it, for a join whose
//! `getinfo` goes unanswered (#222). The userinfo a join connects with is fixed before
//! the gamestate says which mod the server runs, and a JA+ server reads the plugin keys
//! (`cjp_client`) from that connect userinfo only; a lost `getinfo` used to connect as
//! base JKA, leaving JA+ katas, kicks and `+button12` off until the next map.

use sjk_client::CompatProfile;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

/// `getinfo` packets sent before a join gives up on the answer.
pub(super) const GETINFO_TRIES: usize = 3;
/// How long each waits for its answer.
pub(super) const GETINFO_WAIT: Duration = Duration::from_millis(400);
/// The wait before joining again with the right keys: OpenJK's `sv_reconnectlimit`
/// (3 s by default, `SV_DirectConnect`) refuses a client of the same address sooner.
pub(super) const RECONNECT_WAIT: Duration = Duration::from_millis(3_500);

static KNOWN: Mutex<Option<HashMap<SocketAddr, CompatProfile>>> = Mutex::new(None);

/// What `server` ran when last joined in this run of the client.
pub(super) fn known(server: SocketAddr) -> Option<CompatProfile> {
    KNOWN
        .lock()
        .ok()?
        .as_ref()
        .and_then(|known| known.get(&server).cloned())
}

/// Remember what `server`'s gamestate says it runs.
pub(super) fn remember(server: SocketAddr, profile: &CompatProfile) {
    if let Ok(mut known) = KNOWN.lock() {
        known
            .get_or_insert_with(HashMap::new)
            .insert(server, profile.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_joined_servers_mod_is_remembered_per_address() {
        let server: SocketAddr = "192.0.2.7:29070".parse().unwrap();
        let other: SocketAddr = "192.0.2.7:29071".parse().unwrap();
        assert_eq!(known(server), None);
        let japlus = CompatProfile::JaPlus { version: None };
        remember(server, &japlus);
        assert_eq!(known(server), Some(japlus));
        assert_eq!(known(other), None);
        remember(server, &CompatProfile::BaseJka);
        assert_eq!(known(server), Some(CompatProfile::BaseJka));
    }
}
