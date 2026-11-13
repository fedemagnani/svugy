//! Export `render`'s static SVG of one workflow as PNG: the whole workflow, one disclosure layer, or every layer.

use std::path::{Path, PathBuf};

use clap::Args;
use miette::miette;
use resvg::tiny_skia::{Pixmap, Transform};
use svugy_model::model::ShownWorkflow;

use crate::failure::Failure;
use crate::flavor::RenderTheme;
use crate::output::OutputTarget;
use crate::workflow_choice::WORKFLOW_CHOICE_GROUP;

/// Rasterize the PNG at this many pixels per SVG unit, sharp on high-density screens.
const PIXELS_PER_SVG_UNIT: f32 = 2.0;

/// Pick whether `render` exports a PNG instead of an SVG, and which disclosure layers it exports.
#[derive(Args)]
pub struct PngExport {
    /// Write a PNG of one workflow's static SVG instead of an SVG, so it needs --workflow or --all-workflows; --theme applies.
    #[arg(long, conflicts_with = "profile", requires = WORKFLOW_CHOICE_GROUP)]
    png: bool,
    /// With --png, export only layer N: 0 shows the top-level elements, each next layer opens one more depth of containers.
    #[arg(
        long,
        value_name = "N",
        requires = "png",
        conflicts_with = "all_depths"
    )]
    depth: Option<usize>,
    /// With --png, export every layer, layer N to <OUTPUT stem>-depth-N.png beside OUTPUT; needs -o.
    #[arg(long, requires_all = ["png", "output"], conflicts_with = "open")]
    all_depths: bool,
}

impl PngExport {
    /// Write the PNG of the shown workflow of `shown` drawn in `theme` to `output`, or one PNG per layer beside it with --all-depths.
    pub fn write(
        &self,
        shown: ShownWorkflow<'_>,
        theme: RenderTheme,
        output: &OutputTarget,
    ) -> Result<(), Failure> {
        let is_whole_workflow = self.depth.is_none() && !self.all_depths;
        if is_whole_workflow {
            let svg = theme.render_static(shown);
            let png = Self::rasterize(&svg);
            return output.write(png);
        }
        let layer_svgs = theme.render_every_static_layer(shown);
        if let Some(depth) = self.depth {
            let Some(layer_svg) = layer_svgs.get(depth) else {
                let deepest_depth = layer_svgs.len() - 1;
                let out_of_range = miette!(
                    "--depth {depth} is out of range: the workflow has layers 0 to {deepest_depth}"
                );
                return Err(Failure::Usage(out_of_range));
            };
            let png = Self::rasterize(layer_svg);
            return output.write(png);
        }
        let output_path = output.path().expect("clap requires -o with --all-depths");
        for (depth, layer_svg) in layer_svgs.iter().enumerate() {
            let layer_path = Self::layer_path(output_path, depth);
            let png = Self::rasterize(layer_svg);
            OutputTarget::write_file(&layer_path, png)?;
        }
        Ok(())
    }

    /// Return whether `--png` was given.
    pub fn is_requested(&self) -> bool {
        self.png
    }

    /// Rasterize `svg` at `PIXELS_PER_SVG_UNIT` and encode it as PNG.
    fn rasterize(svg: &str) -> Vec<u8> {
        let options = usvg::Options::default();
        let tree = usvg::Tree::from_str(svg, &options).expect("svugy writes SVG that usvg parses");
        let svg_size = tree.size().to_int_size();
        let png_size = svg_size
            .scale_by(PIXELS_PER_SVG_UNIT)
            .expect("a scaled static SVG keeps a non-zero size");
        let mut pixmap = Pixmap::new(png_size.width(), png_size.height())
            .expect("a static SVG frames a non-empty canvas");
        let scale = Transform::from_scale(PIXELS_PER_SVG_UNIT, PIXELS_PER_SVG_UNIT);
        let mut pixmap_view = pixmap.as_mut();
        resvg::render(&tree, scale, &mut pixmap_view);
        pixmap
            .encode_png()
            .expect("an in-memory pixmap encodes as PNG")
    }

    /// Name layer `depth`'s file beside `output_path`: `<stem>-depth-<depth>.png`.
    fn layer_path(output_path: &Path, depth: usize) -> PathBuf {
        let output_file_stem = output_path.file_stem().unwrap_or_default();
        let output_stem = output_file_stem.to_string_lossy();
        let layer_file_name = format!("{output_stem}-depth-{depth}.png");
        output_path.with_file_name(layer_file_name)
    }
}
