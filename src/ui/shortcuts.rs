//! Keyboard shortcuts. "Ctrl" is Cmd (⌘) on macOS.
//!
//! | Shortcut | Action |
//! | --- | --- |
//! | Ctrl+1 … Ctrl+5 | Switch tab |
//! | Ctrl+N | New transaction (Transactions tab, description focused) |
//! | Ctrl+F | Search transactions |
//! | Ctrl+S | Add or save the transaction being edited; on Settings, Save & connect |
//! | Esc | Cancel editing a transaction or renaming a category; close a dialog |

use egui::{Key, KeyboardShortcut, Modifiers};

use super::{App, Tab};

pub(super) const NEW_TRANSACTION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub(super) const SEARCH: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
pub(super) const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

const TAB_KEYS: [Key; 5] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];

/// Ctrl+1 for the first tab, Ctrl+2 for the second, and so on.
pub(super) fn tab(tab: Tab) -> KeyboardShortcut {
    let index = Tab::ALL.iter().position(|t| *t == tab).expect("every tab is in Tab::ALL");
    KeyboardShortcut::new(Modifiers::COMMAND, TAB_KEYS[index])
}

/// Every shortcut with what it does, for the list in Settings.
pub(super) fn describe(ctx: &egui::Context) -> Vec<(String, &'static str)> {
    let first = ctx.format_shortcut(&tab(Tab::ALL[0]));
    let last = ctx.format_shortcut(&tab(Tab::ALL[Tab::ALL.len() - 1]));
    vec![
        (format!("{first} … {last}"), "Switch tab (in the order they are listed)"),
        (ctx.format_shortcut(&NEW_TRANSACTION), "New transaction"),
        (ctx.format_shortcut(&SEARCH), "Search transactions"),
        (ctx.format_shortcut(&SAVE), "Add or save the transaction; on Settings, save and connect"),
        ("Enter".into(), "Add or save, from a transaction or category field"),
        ("Esc".into(), "Cancel editing or renaming; close a dialog"),
    ]
}

impl App {
    /// Act on shortcuts pressed this frame. Runs before the panels are drawn,
    /// so the focus requests it makes land on this frame's widgets.
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // A dialog has the keyboard while it is open (Esc closes it).
        if self.tx.dialog_open() || self.import.is_open() || self.settings.dialog_open() {
            return;
        }
        for t in Tab::ALL {
            if ctx.input_mut(|i| i.consume_shortcut(&tab(t))) {
                self.switch_tab(t);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEW_TRANSACTION)) {
            self.switch_tab(Tab::Transactions);
            self.tx.start_new();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SEARCH)) {
            self.switch_tab(Tab::Transactions);
            self.tx.focus_search();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE)) {
            match self.tab {
                Tab::Settings => self.apply_settings(),
                // Not on an empty form, which would only show an error.
                Tab::Transactions if self.tx.has_input() => self.submit_transaction(),
                _ => {}
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            match self.tab {
                Tab::Transactions => self.tx.cancel_edit(),
                Tab::Categories => self.cats.cancel_rename(),
                _ => {}
            }
        }
    }
}
