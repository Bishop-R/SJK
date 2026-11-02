//! Low-volume VFS ownership diagnostics at session lifecycle boundaries.

use sjk_vfs::VirtualFileSystem;
use std::sync::Arc;

pub(crate) fn log(event: &str, attached: Option<&Arc<VirtualFileSystem>>) {
    let stats = sjk_vfs::lifetime_stats();
    match attached {
        Some(vfs) => crate::log::progress(format_args!(
            "VFS lifetime at {event}: attached generation {}, live generations {}, live PK3 archives {}",
            vfs.generation_id(),
            stats.live_generations,
            stats.live_pk3_archives,
        )),
        None => crate::log::progress(format_args!(
            "VFS lifetime at {event}: no attached generation, live generations {}, live PK3 archives {}",
            stats.live_generations, stats.live_pk3_archives,
        )),
    }
}
