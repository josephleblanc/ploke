
use std::collections::HashMap;

use super::*;
/// Contains data for the UI to reference the selected cells in the returned value from a query.
#[derive(Default)]
pub struct TableCells {
    pub(crate) selected_cells: Vec<(usize, usize)>, // (row, column) indices
    pub(crate) selection_in_progress: Option<(usize, usize)>, // Starting cell for drag selection
    pub(crate) control_groups: HashMap<usize, Vec<(usize, usize)>>, // Control group number -> cells
}

impl TableCells {
    pub(crate) fn reset_selected_cells(cells: &mut TableCells) {
        cells.selected_cells.clear();
        cells.selection_in_progress = None;
        cells.control_groups.clear();
    }
}


