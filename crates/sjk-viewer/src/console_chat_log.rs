//! Opt-in received game-chat persistence (off by default, as in stock clients). Only chat events perform I/O.
use super::*;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Archived game-chat logging mode: 0 off, 1 append, 2 append and synchronize.
pub(crate) const CVAR: &str = "cl_logChat";

/// Owned log handle and sticky failure latch; retry after disabling logging.
#[derive(Default)]
pub(super) struct ChatLog {
    file: Option<File>,
    failed: bool,
    /// Disabling between messages clears a prior failure on the next event too.
    reset: Arc<AtomicBool>,
}

/// TaystJK cl_main.cpp:3456 and cl_cgame.cpp:563-580, 664-675.
pub(super) fn register(cvars: &mut CvarRegistry) -> Result<ChatLog, sjk_shell::CvarError> {
    cvars.register(CvarDefinition::new(
        CVAR,
        0_i64,
        CvarFlags::ARCHIVE,
        "Append received chat to chatlogs/chat.log; 2 synchronizes each entry",
    ))?;
    let log = ChatLog::default();
    let reset = Arc::clone(&log.reset);
    cvars.on_change(CVAR, move |change| {
        if matches!(change.current, CvarValue::Integer(0)) {
            reset.store(true, Ordering::Relaxed);
        }
    })?;
    Ok(log)
}

impl ViewerConsole {
    /// Log live chat before display filtering, with no audio or network side effects.
    pub(crate) fn log_chat(&mut self, kind: &sjk_client::ServerEventKind, text: &str) {
        use sjk_client::ServerEventKind::{Chat, TeamChat};
        if !matches!(kind, Chat | TeamChat) {
            return;
        }
        let mode = self.integer_cvar(CVAR).unwrap_or(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Err(error) = self
            .chat_log
            .append(&self.config_directory, mode, timestamp, text)
        {
            self.push_log(format!("Chat logging disabled after write error: {error}"));
        }
    }
}

impl ChatLog {
    fn append(
        &mut self,
        directory: &std::path::Path,
        mode: i64,
        time: u64,
        text: &str,
    ) -> std::io::Result<()> {
        if self.reset.swap(false, Ordering::Relaxed) || mode == 0 {
            self.file = None;
            self.failed = false;
        }
        if mode == 0 {
            return Ok(());
        }
        if self.failed {
            return Ok(());
        }
        let result = self.write(directory, mode, time, text);
        if result.is_err() {
            self.failed = true;
            self.file = None;
        }
        result
    }

    fn write(
        &mut self,
        directory: &std::path::Path,
        mode: i64,
        time: u64,
        text: &str,
    ) -> std::io::Result<()> {
        if self.file.is_none() {
            let directory = directory.join("chatlogs");
            std::fs::create_dir_all(&directory)?;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(directory.join("chat.log"))?,
            );
        }
        // Bounded UTF-8-safe cleaning; no per-message string or texture allocations.
        let mut cleaned = [0_u8; 4096];
        let length = clean(text.as_bytes(), &mut cleaned);
        let file = self.file.as_mut().expect("opened above");
        write!(file, "[{time}] ")?;
        file.write_all(&cleaned[..length])?;
        file.write_all(b"\n")?;
        if mode == 2 {
            file.sync_data()?;
        }
        Ok(())
    }
}

fn clean(text: &[u8], output: &mut [u8]) -> usize {
    let (mut source, mut written) = (0, 0);
    while source < text.len() && written < output.len() {
        if text[source] == b'^' && text.get(source + 1).is_some_and(u8::is_ascii_digit) {
            source += 2;
            continue;
        }
        let byte = text[source];
        source += 1;
        if byte < 32 || byte == 127 {
            continue;
        }
        output[written] = byte;
        written += 1;
    }
    while std::str::from_utf8(&output[..written]).is_err() && written > 0 {
        written -= 1;
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_client::ServerEventKind;

    #[test]
    fn an_opted_in_profile_logs_received_game_chat_and_appends_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("config.cfg");
        let path = directory.path().join("chatlogs/chat.log");
        {
            let fresh = tempfile::tempdir().unwrap();
            let mut console = ViewerConsole::new(fresh.path().join("config.cfg")).unwrap();
            assert_eq!(console.integer_cvar(CVAR), Some(0), "off by default");
            console.log_chat(&ServerEventKind::Chat, "Player: unlogged");
            assert!(
                !fresh.path().join("chatlogs").exists(),
                "nothing logged until opted in"
            );
        }
        std::fs::write(&config, "seta cl_logChat \"1\"\n").unwrap();
        {
            let mut console = ViewerConsole::new(config.clone()).unwrap();
            console.log_chat(&ServerEventKind::Print, "not a chat message");
            assert!(!path.exists(), "no log is created for other server text");
            console.log_chat(&ServerEventKind::Chat, "^1Player: hello\n");
            console.log_chat(&ServerEventKind::TeamChat, "^2Teammate: ready");
        }
        let mut console = ViewerConsole::new(config).unwrap();
        console.log_chat(&ServerEventKind::Chat, "Player: back");
        let saved = std::fs::read_to_string(path).unwrap();
        let lines: Vec<_> = saved.lines().collect();
        assert_eq!(lines.len(), 3);
        for (line, text) in lines
            .iter()
            .zip(["Player: hello", "Teammate: ready", "Player: back"])
        {
            let (time, body) = line.split_once("] ").unwrap();
            assert!(time.strip_prefix('[').unwrap().parse::<u64>().is_ok());
            assert_eq!(body, text);
        }
    }

    #[test]
    fn a_saved_opt_out_stays_off_and_the_switch_persists() {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("config.cfg");
        std::fs::write(&config, "seta cl_logChat \"0\"\n").unwrap();
        let mut console = ViewerConsole::new(config.clone()).unwrap();
        assert_eq!(console.integer_cvar(CVAR), Some(0));
        console.log_chat(&ServerEventKind::Chat, "off");
        assert!(!directory.path().join("chatlogs").exists());
        assert!(console.set_cvar(CVAR, "1"));
        console.log_chat(&ServerEventKind::Chat, "on");
        assert!(console.set_cvar(CVAR, "0"));
        console.log_chat(&ServerEventKind::Chat, "off again");
        drop(console);
        let console = ViewerConsole::new(config).unwrap();
        assert_eq!(console.integer_cvar(CVAR), Some(0));
        let saved = std::fs::read_to_string(directory.path().join("chatlogs/chat.log")).unwrap();
        assert_eq!(saved.lines().count(), 1);
        assert!(saved.ends_with("] on\n"));
    }

    #[test]
    fn a_write_failure_can_be_retried_after_off_then_on_without_intervening_chat() {
        let directory = tempfile::tempdir().unwrap();
        let obstruction = directory.path().join("chatlogs");
        std::fs::write(&obstruction, "a file blocks the log directory").unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert!(console.set_cvar(CVAR, "1"));
        console.log_chat(&ServerEventKind::Chat, "first failure");
        assert!(console.chat_log.failed);
        std::fs::remove_file(obstruction).unwrap();
        console.log_chat(&ServerEventKind::Chat, "latched failure");
        assert!(!directory.path().join("chatlogs").exists());
        assert!(console.set_cvar(CVAR, "0"));
        assert!(console.set_cvar(CVAR, "1"));
        console.log_chat(&ServerEventKind::Chat, "recovered");
        assert!(!console.chat_log.failed);
        let saved = std::fs::read_to_string(directory.path().join("chatlogs/chat.log")).unwrap();
        assert!(saved.ends_with("] recovered\n"));
    }

    #[test]
    fn synchronized_mode_and_bounded_unicode_cleaning_keep_single_lines() {
        let directory = tempfile::tempdir().unwrap();
        let mut log = ChatLog::default();
        log.append(directory.path(), 2, 42, "^3Al\u{ef}ce:\n hi\t!\u{7f}")
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.path().join("chatlogs/chat.log")).unwrap(),
            "[42] Al\u{ef}ce: hi!\n"
        );
        let mut output = [0; 7];
        let length = clean("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}".as_bytes(), &mut output);
        assert_eq!(
            std::str::from_utf8(&output[..length]).unwrap(),
            "\u{e9}\u{e9}\u{e9}"
        );
        log.append(directory.path(), 0, 43, "disabled").unwrap();
        assert!(log.file.is_none());
    }
}
