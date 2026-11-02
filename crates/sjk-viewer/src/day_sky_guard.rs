//! Keeps the sky's day tint from closing the game at map load, and at every load after
//! (hub report #219).
//!
//! The day tint (`sky_stage::day`, the sky following `r_dayHour` or, with `r_dayClock`,
//! the PC's clock) compiles its own sky pipelines while the graphics load. A compile
//! wgpu rejects ([`crate::gpu_guard::capture`]) is logged and the sky keeps the map's
//! own look. A compile that ends the process instead (a graphics driver going down
//! with it) would end every later load too, so before compiling, `crashes/day-sky.txt`
//! is written, and removed once the compile returns, failed or not. A file still there
//! at the next start means the game closed while compiling: that session draws the
//! map's own sky, logs why and says so once in the console. The file is read and
//! removed once a session, so the tint is tried again at the start after, as
//! `menu_stage/hilt_guard.rs` does for a hilt (#214).

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// The file, in the crash reports' folder.
const FILE: &str = "day-sky.txt";

/// What the console says in a session that leaves the tint out.
pub(crate) const NOTICE: &str = "Sun follows PC clock (r_dayClock): the sky's day and night \
    tint is off this session, as the game closed while loading graphics last time; it is \
    tried again at the next start";

/// Whether the last session closed while compiling the tint, read once a session.
static CRASHED: OnceLock<bool> = OnceLock::new();
/// The console notice is still to be shown.
static NOTICE_DUE: AtomicBool = AtomicBool::new(false);

/// Build the tint's pipelines with `build`, unless the last session closed doing so.
/// `None` (logged) when it is left out or wgpu rejected it: the caller keeps the
/// authored sky.
pub(crate) fn build<T>(device: &wgpu::Device, build: impl FnOnce() -> T) -> Option<T> {
    build_in(crate::crash_report::directory().as_deref(), device, build)
}

fn build_in<T>(
    directory: Option<&Path>,
    device: &wgpu::Device,
    build: impl FnOnce() -> T,
) -> Option<T> {
    if crashed_last_time(directory) {
        return None;
    }
    begin(directory);
    let built = crate::gpu_guard::capture(device, build);
    end(directory);
    match built {
        Ok(value) => Some(value),
        Err(error) => {
            crate::log::progress(format_args!(
                "sky: the day tint's pipelines failed ({error}); drawing the map's own sky"
            ));
            None
        }
    }
}

/// Whether the last session closed while compiling the tint. The first call reads (and
/// removes) what that session left in `directory`, logs it and queues the notice.
fn crashed_last_time(directory: Option<&Path>) -> bool {
    *CRASHED.get_or_init(|| {
        let crashed = directory.is_some_and(take);
        if crashed {
            crate::log::progress(format_args!(
                "sky: the game closed while compiling the day tint last time \
                 (crashes/{FILE}); drawing the map's own sky this session"
            ));
            NOTICE_DUE.store(true, Ordering::Relaxed);
        }
        crashed
    })
}

/// The console notice, once, when this session leaves the tint out.
pub(crate) fn take_notice() -> Option<&'static str> {
    NOTICE_DUE.swap(false, Ordering::Relaxed).then_some(NOTICE)
}

/// Note that the tint is being compiled.
fn begin(directory: Option<&Path>) {
    if let Some(directory) = directory {
        let _ = std::fs::create_dir_all(directory);
        let _ = std::fs::write(directory.join(FILE), "day sky pipelines\n");
    }
}

/// Note that the compile returned.
fn end(directory: Option<&Path>) {
    if let Some(directory) = directory {
        let _ = std::fs::remove_file(directory.join(FILE));
    }
}

/// Whether `directory` holds the file, removing it.
fn take(directory: &Path) -> bool {
    let path = directory.join(FILE);
    let found = path.exists();
    let _ = std::fs::remove_file(&path);
    found
}

impl crate::GpuState {
    /// Say once in the console that the day tint is off this session ([`NOTICE`]).
    pub(crate) fn report_day_sky_guard(&mut self) {
        if let Some(console) = self.console.as_mut()
            && let Some(notice) = take_notice()
        {
            console.push_log(notice);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("sjk-day-sky-guard-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    /// A compile that returned leaves nothing; one the process ended in leaves the file,
    /// which the next start finds once.
    #[test]
    fn a_compile_left_running_is_found_once_and_a_finished_one_not_at_all() {
        let directory = directory("marker");
        begin(Some(&directory));
        assert!(directory.join(FILE).exists(), "written before the compile");
        end(Some(&directory));
        assert!(!take(&directory), "a compile that returned leaves nothing");
        begin(Some(&directory));
        // The process ended here; the next start finds the file, once.
        assert!(take(&directory));
        assert!(!take(&directory));
        // Without a crash folder (tests, before the profile is known) nothing is written.
        begin(None);
        end(None);
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A pipeline wgpu rejects falls back (`None`) instead of closing the game, and the
    /// file goes either way.
    #[test]
    #[ignore = "requires an available GPU; uses no installed game assets"]
    fn a_rejected_compile_falls_back_and_leaves_no_marker() {
        let gpu = pollster::block_on(crate::gpu_context::Context::new(None, None)).unwrap();
        let device = &gpu.device;
        let directory = directory("fallback");
        let rejected = build_in(Some(&directory), device, || {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("SJK day sky guard test"),
                source: wgpu::ShaderSource::Wgsl("fn broken( {".into()),
            })
        });
        assert!(rejected.is_none(), "a rejected program is the authored sky");
        assert!(!directory.join(FILE).exists());
        let built = build_in(Some(&directory), device, || 7);
        assert_eq!(built, Some(7));
        assert!(!directory.join(FILE).exists());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
