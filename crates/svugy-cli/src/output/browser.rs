//! Open a file or URL in the default web browser, not in whatever app the file's type is registered to.

use std::ffi::OsStr;
#[cfg(target_os = "macos")]
use std::path::Path;
use std::process::Command;

use miette::{IntoDiagnostic, WrapErr, miette};

use crate::failure::Failure;

/// Locate macOS's LaunchServices handler table, relative to the home directory.
#[cfg(target_os = "macos")]
const LAUNCH_SERVICES_PLIST: &str =
    "Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist";
/// Name the browser macOS opens links in when the user never picked another.
#[cfg(target_os = "macos")]
const SAFARI_BUNDLE_ID: &str = "com.apple.Safari";

/// Opens files and URLs in the browser the user opens web links in.
pub struct DefaultBrowser;

impl DefaultBrowser {
    /// Open `target`, a file path or a URL, in the default browser, failing when the opener can't run or reports an error.
    pub fn open(target: &OsStr) -> Result<(), Failure> {
        let target_display = target.to_string_lossy();
        let mut opener = Self::opener();
        let opener_program = opener.get_program().to_string_lossy().into_owned();
        let opener_status = opener
            .arg(target)
            .status()
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot run `{opener_program}` to open {target_display}"))
            .map_err(Failure::Io)?;
        let is_opened = opener_status.success();
        if is_opened {
            return Ok(());
        }
        let opener_report = miette!("`{opener_program}` could not open {target_display}");
        Err(Failure::Io(opener_report))
    }

    /// Build `open -b BUNDLE`, BUNDLE being the app macOS opens `https` links in, since plain `open` picks the `.svg` handler.
    #[cfg(target_os = "macos")]
    fn opener() -> Command {
        let browser_bundle_id = Self::https_handler_bundle_id();
        let mut opener = Command::new("open");
        opener.args(["-b", &browser_bundle_id]);
        opener
    }

    /// Build `cmd /C start ""`.
    #[cfg(target_os = "windows")]
    fn opener() -> Command {
        let mut opener = Command::new("cmd");
        opener.args(["/C", "start", ""]);
        opener
    }

    /// Build `xdg-open`.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn opener() -> Command {
        Command::new("xdg-open")
    }

    /// Read the bundle id LaunchServices records for `https` links, or Safari's when none is recorded or the table can't be read.
    #[cfg(target_os = "macos")]
    fn https_handler_bundle_id() -> String {
        let Some(home) = std::env::var_os("HOME") else {
            return SAFARI_BUNDLE_ID.to_owned();
        };
        let plist_path = Path::new(&home).join(LAUNCH_SERVICES_PLIST);
        let conversion = Command::new("plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&plist_path)
            .output();
        let Ok(conversion) = conversion else {
            return SAFARI_BUNDLE_ID.to_owned();
        };
        let handler_table = serde_json::from_slice::<serde_json::Value>(&conversion.stdout);
        let Ok(handler_table) = handler_table else {
            return SAFARI_BUNDLE_ID.to_owned();
        };
        let handlers = handler_table["LSHandlers"].as_array();
        let https_handler = handlers
            .into_iter()
            .flatten()
            .find(|handler| handler["LSHandlerURLScheme"] == "https");
        let bundle_id = https_handler.and_then(|handler| {
            let role_all = handler["LSHandlerRoleAll"].as_str();
            role_all.or_else(|| handler["LSHandlerRoleViewer"].as_str())
        });
        bundle_id.unwrap_or(SAFARI_BUNDLE_ID).to_owned()
    }
}
