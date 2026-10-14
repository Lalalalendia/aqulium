use super::selection::Selection;

/// Tracks active Input Method Editor (IME) composition state (`preedit`).
///
/// While an IME composition is active:
/// - The text that was selected when composition started is remembered in `replaced_text`.
/// - Updates via `ime_preedit` mutate `active_range` in the `RopeBuffer` so the user
///   sees live composition inside the bounded GPUI viewport, without pushing intermediate
///   keystrokes onto the `UndoStack`.
/// - `ime_cancel` restores `replaced_text` and `initial_selection`.
/// - `ime_commit` finalizes the composed text as a single atomic `UndoEntry` and syncs
///   it to `DocumentHub`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeState {
    pub active_range: Option<(usize, usize)>,
    pub preedit_text: String,
    pub cursor_in_preedit: Option<(usize, usize)>,
    pub replaced_text: String,
    pub initial_selection: Selection,
}

impl ImeState {
    pub fn is_composing(&self) -> bool {
        self.active_range.is_some()
    }

    pub fn clear(&mut self) {
        self.active_range = None;
        self.preedit_text.clear();
        self.cursor_in_preedit = None;
        self.replaced_text.clear();
    }
}
