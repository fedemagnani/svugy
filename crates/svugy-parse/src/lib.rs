//! Parse diagrams written in Markdown, d2 or JSON into the svugy model, through one diagnostic harness, and print a model back as Markdown or JSON.

pub mod d2;
pub mod diagnostic;
pub mod json;
pub mod markdown;

mod prose;
