use std::cmp::{Ordering, Reverse};

use super::barycenter::BarycenterEntry;

/// Sort nodes by barycenter, ties by prior position (`bias_right` flips the tie order), and put nodes without one back at their prior positions.
pub(super) fn sort(entries: Vec<BarycenterEntry>, bias_right: bool) -> Vec<String> {
    let (mut sortable, mut unsortable): (Vec<BarycenterEntry>, Vec<BarycenterEntry>) = entries
        .into_iter()
        .partition(|entry| entry.barycenter.is_some());
    unsortable.sort_by_key(|entry| Reverse(entry.i));
    sortable.sort_by(|a, b| compare_with_bias(a, b, bias_right));

    let mut vs = Vec::new();
    let mut vs_index = consume_unsortable(&mut vs, &mut unsortable, 0);
    for entry in sortable {
        vs_index += 1;
        vs.push(entry.v);
        vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);
    }
    vs
}

/// Append the unsortable nodes whose prior position the sorted prefix has reached.
fn consume_unsortable(
    vs: &mut Vec<String>,
    unsortable: &mut Vec<BarycenterEntry>,
    mut index: usize,
) -> usize {
    while let Some(last) = unsortable.pop_if(|last| last.i <= index) {
        vs.push(last.v);
        index += 1;
    }
    index
}

/// Compare by barycenter, then by prior position descending, or ascending with `bias_right`; never equal, as in dagre-js.
fn compare_with_bias(a: &BarycenterEntry, b: &BarycenterEntry, bias_right: bool) -> Ordering {
    let a_barycenter = a.barycenter.unwrap_or(0.0);
    let b_barycenter = b.barycenter.unwrap_or(0.0);
    if a_barycenter < b_barycenter {
        return Ordering::Less;
    }
    if a_barycenter > b_barycenter {
        return Ordering::Greater;
    }
    let is_a_after_b = if bias_right { a.i < b.i } else { a.i > b.i };
    if is_a_after_b {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}
