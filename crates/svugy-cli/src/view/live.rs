//! Keep the latest rendering of the watched file for readers to wait on, re-rendering it whenever the file changes.

use std::fs;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::failure::Failure;
use crate::flavor::SvgFlavor;
use crate::input::{LoadedInput, ModelInput};

/// Space the checks of the watched file's modification time.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Share the latest good SVG and the diagnostics of a later failed save with every reader, waking them on each publish.
pub struct LiveDiagram {
    state: Mutex<DiagramState>,
    published: Condvar,
}

/// Hold what the publishes left: the latest good SVG and the generation it came with, the diagnostics of a failed save since, and the latest generation.
struct DiagramState {
    svg: String,
    svg_generation: u64,
    diagnostics: Option<String>,
    generation: u64,
}

/// Follow one reader through the publishes, so each wait returns what that reader has not seen.
pub struct Subscription<'a> {
    live_diagram: &'a LiveDiagram,
    seen_generation: Option<u64>,
}

/// Name what a reader has not seen: a newer SVG's generation, and the diagnostics of a failed save that followed it.
pub struct LiveUpdate {
    pub reloaded_generation: Option<u64>,
    pub diagnostics: Option<String>,
}

/// Re-render the watched file into the live diagram whenever its modification time changes.
pub struct FileWatcher {
    input: ModelInput,
    flavor: SvgFlavor,
    last_modified: Option<SystemTime>,
}

impl LiveDiagram {
    /// Create a new live diagram showing `svg` at generation 0, without diagnostics.
    pub fn new(svg: String) -> Self {
        let state = DiagramState {
            svg,
            svg_generation: 0,
            diagnostics: None,
            generation: 0,
        };
        Self {
            state: Mutex::new(state),
            published: Condvar::new(),
        }
    }

    /// Replace the SVG with `svg` at the next generation, clearing the diagnostics, and wake every reader.
    pub fn publish_svg(&self, svg: String) {
        let mut state = self.lock_state();
        state.generation += 1;
        state.svg_generation = state.generation;
        state.svg = svg;
        state.diagnostics = None;
        self.published.notify_all();
    }

    /// Set the diagnostics of a failed save at the next generation, keeping the last good SVG, and wake every reader.
    pub fn publish_diagnostics(&self, diagnostics: String) {
        let mut state = self.lock_state();
        state.generation += 1;
        state.diagnostics = Some(diagnostics);
        self.published.notify_all();
    }

    /// Subscribe a new reader, whose first update is the whole current state.
    pub fn subscribe(&self) -> Subscription<'_> {
        Subscription {
            live_diagram: self,
            seen_generation: None,
        }
    }

    /// Copy the latest good SVG.
    pub fn svg(&self) -> String {
        let state = self.lock_state();
        state.svg.clone()
    }

    /// Lock the state, recovering it from a poisoned lock since every publish leaves it whole.
    fn lock_state(&self) -> MutexGuard<'_, DiagramState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Subscription<'_> {
    /// Wait for a generation this reader has not seen and return what changed; the first call returns at once.
    pub fn next_update(&mut self) -> LiveUpdate {
        let live_diagram = self.live_diagram;
        let seen_generation = self.seen_generation;
        let state = live_diagram.lock_state();
        let state = live_diagram
            .published
            .wait_while(state, |state| Some(state.generation) == seen_generation)
            .unwrap_or_else(PoisonError::into_inner);
        let is_svg_unseen = seen_generation.is_none_or(|seen| state.svg_generation > seen);
        let reloaded_generation = is_svg_unseen.then_some(state.svg_generation);
        let diagnostics = state.diagnostics.clone();
        self.seen_generation = Some(state.generation);
        LiveUpdate {
            reloaded_generation,
            diagnostics,
        }
    }
}

impl FileWatcher {
    /// Create a new watcher of `input` rendering it as `flavor`, its current modification time counting as already rendered.
    pub fn new(input: ModelInput, flavor: SvgFlavor) -> Self {
        let last_modified = Self::modification_time_of(&input);
        Self {
            input,
            flavor,
            last_modified,
        }
    }

    /// Check the file every poll interval, re-rendering it into `live_diagram` after each change of its modification time.
    pub fn watch_forever(mut self, live_diagram: &LiveDiagram) -> ! {
        loop {
            thread::sleep(POLL_INTERVAL);
            let modified = Self::modification_time_of(&self.input);
            let is_unchanged = modified == self.last_modified;
            if is_unchanged {
                continue;
            }
            self.last_modified = modified;
            self.rerender_into(live_diagram);
        }
    }

    /// Load the file, print its diagnostics on stderr as `render` does, and publish its SVG, or its diagnostics when it has errors.
    fn rerender_into(&self, live_diagram: &LiveDiagram) {
        let loaded = self.input.load().inspect_err(Failure::eprint);
        let Ok(LoadedInput { model, report }) = loaded else {
            return;
        };
        report.eprint_diagnostics();
        let Some(model) = model else {
            let diagnostics = report.to_plain_text();
            live_diagram.publish_diagnostics(diagnostics);
            return;
        };
        let first_workflow = model.showing_first_workflow();
        let svg = self.flavor.render(first_workflow);
        live_diagram.publish_svg(svg);
    }

    /// Read the modification time of `input`'s file, `None` while it cannot be read.
    fn modification_time_of(input: &ModelInput) -> Option<SystemTime> {
        let input_path = input.path();
        let metadata = fs::metadata(input_path).ok()?;
        metadata.modified().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::LiveDiagram;

    #[test]
    fn viewer_page_gets_the_whole_state_on_connect_then_only_what_it_missed() {
        let live_diagram = LiveDiagram::new("<svg>v0</svg>".to_owned());
        let mut viewer_page = live_diagram.subscribe();
        let on_connect = viewer_page.next_update();
        assert_eq!(on_connect.reloaded_generation, Some(0));

        live_diagram.publish_svg("<svg>v1</svg>".to_owned());
        live_diagram.publish_diagnostics("no component `nowhere`".to_owned());
        let after_good_then_broken_save = viewer_page.next_update();
        assert_eq!(after_good_then_broken_save.reloaded_generation, Some(1));
        let missed_diagnostics = after_good_then_broken_save.diagnostics.as_deref();
        assert_eq!(missed_diagnostics, Some("no component `nowhere`"));
        let served_svg = live_diagram.svg();
        assert_eq!(
            served_svg, "<svg>v1</svg>",
            "a broken save keeps the last good SVG served"
        );

        let after_fixing_save = thread::scope(|scope| {
            let waiting_page = scope.spawn(|| viewer_page.next_update());
            live_diagram.publish_svg("<svg>v3</svg>".to_owned());
            waiting_page
                .join()
                .expect("the waiting page wakes on the publish")
        });
        assert_eq!(after_fixing_save.reloaded_generation, Some(3));
        assert_eq!(after_fixing_save.diagnostics, None);
    }
}
