//! Whether the app runs inside a Flatpak sandbox, where it cannot replace itself and the
//! packaging (not this program) owns how it is installed.

use std::path::Path;

/// True under Flatpak. Flatpak sets `FLATPAK_ID` for the app and mounts `/.flatpak-info` in every
/// sandbox; either is enough.
pub fn is_flatpak() -> bool {
    cfg!(target_os = "linux")
        && from_signals(
            std::env::var_os("FLATPAK_ID").as_deref(),
            Path::new("/.flatpak-info"),
        )
}

/// This fork has no hosted Flatpak remote or release feed; local bundles must be rebuilt.
pub const FLATPAK_UPDATE_NOTE: &str = "FakingView is a local build. Rebuild and reinstall its Flatpak bundle to update it; no hosted update repository is configured.";

fn from_signals(flatpak_id: Option<&std::ffi::OsStr>, info_file: &Path) -> bool {
    flatpak_id.is_some_and(|id| !id.is_empty()) || info_file.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn the_app_id_variable_means_flatpak() {
        let missing = Path::new("/definitely/not/here");
        assert!(from_signals(
            Some(OsStr::new("local.fakingview.desktop")),
            missing
        ));
    }

    #[test]
    fn an_empty_or_missing_variable_does_not() {
        let missing = Path::new("/definitely/not/here");
        assert!(!from_signals(None, missing));
        assert!(!from_signals(Some(OsStr::new("")), missing));
    }

    #[test]
    fn the_flatpak_note_explains_local_updates_without_claiming_hosting() {
        assert!(FLATPAK_UPDATE_NOTE.contains("Rebuild and reinstall"));
        assert!(FLATPAK_UPDATE_NOTE.contains("no hosted update repository"));
    }

    #[test]
    fn the_info_file_means_flatpak_without_the_variable() {
        assert!(from_signals(None, Path::new(env!("CARGO_MANIFEST_DIR"))));
    }
}
