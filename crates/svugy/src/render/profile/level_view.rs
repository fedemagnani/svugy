//! Collect every level's view in document coordinates, the rectangle the runtime frames when the level is entered.

use std::collections::BTreeMap;
use std::iter;

use kurbo::Rect;
use svugy_model::path::ElementPath;

use crate::geometry::Group;

/// Maps every level to its document view: the root's is its own view, the document's, each interior's has every enclosing frame applied.
pub(crate) struct LevelViews {
    views_by_level: BTreeMap<ElementPath, Rect>,
}

impl LevelViews {
    /// Collect the views of `root` and of every interior nested in it.
    pub(crate) fn collect(root: &Group) -> Self {
        let root_path = ElementPath::root();
        let root_view = (root_path, root.view);
        let root_views = iter::once(root_view);
        let nested_views = root.nested_interiors().map(|interior| {
            let container = interior.container.clone();
            let outer_view = interior.outer_view();
            (container, outer_view)
        });
        let views_by_level = root_views.chain(nested_views).collect();
        Self { views_by_level }
    }

    /// Return the document view of `level`.
    pub(crate) fn view_of(&self, level: &ElementPath) -> Rect {
        self.views_by_level[level]
    }
}
