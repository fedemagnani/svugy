#![doc = include_str!("../README.md")]

mod acyclic;
mod graph;
mod layout;
mod normalize;
mod order;
mod position;
mod rank;
mod util;

/// Space a left-to-right layered layout: `node_gap` between neighbours within a rank, `rank_gap` between consecutive ranks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftToRightLayout {
    /// Gap between neighbours within one rank.
    pub node_gap: f64,
    /// Gap between consecutive ranks.
    pub rank_gap: f64,
}

/// Size a node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Extent along x.
    pub width: f64,
    /// Extent along y.
    pub height: f64,
}

/// Locate a node's centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Position along x, which ranks advance along.
    pub x: f64,
    /// Position along y, which nodes within a rank spread along.
    pub y: f64,
}
