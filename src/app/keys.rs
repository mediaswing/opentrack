//! Keyboard shortcuts: taking them before egui sees them, and doing what they
//! ask.

use super::*;

impl BudgetApp {
    /// Takes shortcut key presses out of the input before egui sees it, so
    /// egui's own handling (Escape dropping focus, arrows moving focus)
    /// doesn't also happen. The commands run in `run_shortcuts`.
    pub(super) fn take_shortcuts(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let focused = ctx.memory(|m| m.focused());
        let cx = KeyContext {
            // Read from memory: this runs between frames, when egui's
            // per-frame record of open popups isn't kept.
            popup_open: egui::Popup::is_any_open(ctx),
            focused_tab: focused.and_then(|id| self.tab_buttons.iter().position(|b| *b == id)),
            tab_count: Tab::ALL.len(),
        };
        for command in self.keymap.take(&mut raw_input.events, cx, |_| true) {
            match command {
                Command::CloseList => egui::Popup::close_all(ctx),
                _ => self.pending.push(command),
            }
        }
    }

    pub(super) fn run_shortcuts(&mut self, ctx: &egui::Context) {
        for command in std::mem::take(&mut self.pending) {
            match command {
                Command::Tab(i) => {
                    if let Some(tab) = Tab::ALL.get(i) {
                        self.switch_tab(*tab);
                    }
                }
                Command::Run(action) => self.run_action(ctx, action),
                Command::Cancel => self.cancel(),
                // Handled in `take_shortcuts`.
                Command::CloseList => {}
            }
        }
    }

    fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        let count = Tab::ALL.len();
        let current = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        match action {
            Action::New => {
                if self.tab == Tab::Categories { self.new_category() } else { self.new_entry() }
            }
            Action::Save => match self.tab {
                Tab::Spending => self.save_entry(),
                Tab::Categories if self.adding_category(ctx) => self.add_category(),
                Tab::Categories => self.save_category(),
                _ => self.announce(t!("keys-nothing-to-save")),
            },
            Action::Edit => self.edit_selected(),
            Action::Delete => match self.tab {
                Tab::Spending => self.delete_selected_entry(),
                Tab::Categories => self.delete_category(),
                _ => self.announce(t!("keys-nothing-to-delete")),
            },
            Action::Find => self.find_entries(),
            Action::PreviousMonth => self.change_period(self.period.previous()),
            Action::NextMonth => self.change_period(self.period.next()),
            Action::ThisMonth => self.change_period(Period::Month(Month::current())),
            Action::Summary => {
                let text = self.summary_text();
                self.announce(text);
            }
            Action::Import => self.import_any(),
            Action::Export => self.export_any(),
            Action::NextTab => self.switch_tab(Tab::ALL[(current + 1) % count]),
            Action::PreviousTab => self.switch_tab(Tab::ALL[(current + count - 1) % count]),
            // Handled as `Command::Tab`.
            Action::GoToTab => {}
            Action::Help => self.show_shortcuts(),
        }
    }

    /// Escape: stops editing an entry or empties the form, on the Spending
    /// tab. Elsewhere, focus stays where it is.
    fn cancel(&mut self) {
        if self.tab == Tab::Spending && (self.spending.is_editing() || self.spending.has_typing()) {
            self.cancel_entry();
        }
    }

    /// Shows other dates on the Spending and Summary tabs, and says what was
    /// spent in them.
    pub(super) fn change_period(&mut self, period: Period) {
        self.period = period;
        self.write_period_dates();
        self.reload_entries();
        self.reload_summary();
        let spent = self.money(self.summary.spent);
        let label = self.region.period_label(period);
        self.announce(t!("keys-month-changed", month = label, count = self.summary.count, spent = spent));
    }

    /// Lists the shortcuts in a dialog, which screen readers read out when
    /// it opens.
    pub(super) fn show_shortcuts(&mut self) {
        rfd::MessageDialog::new()
            .set_title(t!("shortcuts-title"))
            .set_description(self.keymap.reference())
            .set_buttons(rfd::MessageButtons::Ok)
            .set_level(rfd::MessageLevel::Info)
            .show();
    }
}
