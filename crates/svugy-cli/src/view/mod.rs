//! Serve the input's interactive SVG at a local URL and reload it in the browser whenever the input changes.

mod live;
mod server;

use std::ffi::OsStr;
use std::sync::Arc;

use clap::Args;

use crate::failure::Failure;
use crate::flavor::{RenderProfile, RenderTheme, SvgFlavor};
use crate::input::ModelInput;
use crate::output::browser::DefaultBrowser;
use live::{FileWatcher, LiveDiagram};
use server::ViewServer;

/// State what `view` prints, when it re-renders and what the page shows, after view's help.
pub const LIVE_VIEW_HELP: &str = "\
Prints `svugy view: http://127.0.0.1:PORT` on stderr and runs until Ctrl-C.
INPUT must have no errors at start. Every 250 ms view checks INPUT's
modification time; on a change it re-renders INPUT, prints its diagnostics on
stderr as render does, and reloads the page on the level it was focused on. A
save with errors leaves the last good diagram on the page under its
diagnostics.";

/// Serve the input's interactive SVG in the chosen theme on 127.0.0.1 and re-render it on every change, opening the page unless `--no-open` is given.
#[derive(Args)]
pub struct ViewCommand {
    /// Diagram (.md), d2 file or JSON model to watch.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    /// How to draw the diagram.
    #[arg(long, value_enum, default_value_t = RenderTheme::Standard)]
    theme: RenderTheme,
    /// Port on 127.0.0.1 to serve on; 0 lets the OS pick a free one.
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Don't open the URL in the default browser.
    #[arg(long)]
    no_open: bool,
}

impl ViewCommand {
    /// Run `view`: render the input, serve it, open the page, then re-render on every change until interrupted.
    pub fn run(self) -> Result<(), Failure> {
        let flavor = SvgFlavor::new(RenderProfile::Interactive, self.theme);
        let watched_input = self.input.clone();
        let watcher = FileWatcher::new(watched_input, flavor);
        let model = self.input.load_model_printing_diagnostics()?;
        let svg = flavor.render(&model);
        let live_diagram = LiveDiagram::new(svg);
        let live_diagram = Arc::new(live_diagram);
        let served_live_diagram = Arc::clone(&live_diagram);
        let server = ViewServer::bind(self.port, served_live_diagram)?;
        let url = server.url().to_owned();
        eprintln!("svugy view: {url}");
        server.serve_in_background();
        let is_open_requested = !self.no_open;
        if is_open_requested {
            let url_argument = OsStr::new(&url);
            DefaultBrowser::open(url_argument)?;
        }
        watcher.watch_forever(&live_diagram)
    }
}
