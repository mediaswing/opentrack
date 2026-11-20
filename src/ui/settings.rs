//! The System Settings tab. Everything here is stored in the database as soon as it
//! is changed.

use chrono::NaiveDate;
use egui::{Key, Ui};

use super::widgets::{dropdown, full_button, heading, text_field};
use super::{App, Appearance, appearance};
use crate::locale::LocaleFormat;

impl App {
    pub(super) fn settings_tab(&mut self, ui: &mut Ui) {
        heading(ui, "System Settings", 2);

        heading(ui, "Appearance", 3);
        let names: Vec<String> = Appearance::ALL.iter().map(|a| format!("{}: {}", a.label(), a.description())).collect();
        let mut index = Appearance::ALL.iter().position(|a| *a == self.appearance).unwrap_or(0);
        if dropdown(ui, "appearance", "Colours and text", &names, &mut index).0 {
            self.appearance = Appearance::ALL[index];
            appearance::apply(ui.ctx(), self.appearance);
            self.save_setting("appearance", Some(self.appearance.key()));
            self.set_info(format!("Using the {} appearance.", self.appearance.label()));
        }

        heading(ui, "Dates and Money", 3);
        ui.label(self.formats_sentence());
        let field = text_field(
            ui,
            "Locale, such as en_GB or de_DE; leave empty to follow this computer",
            "locale",
            &mut self.locale_input,
        );
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if entered {
            field.request_focus();
        }
        if full_button(ui, "use_locale", "Use this locale", None).clicked() || entered {
            self.use_locale();
        }

        self.updates_section(ui);

        heading(ui, "Your Data", 3);
        let database = match &self.paths.database {
            Some(path) => format!("Your spending is kept in {}.", path.display()),
            None => "Your spending couldn't be opened, so nothing you add will be kept after the app closes.".to_owned(),
        };
        ui.label(database);
        let log = match &self.paths.log {
            Some(path) => format!(
                "The debug log is {}. It records what the app does, but not what you bought or how much you spent.",
                path.display()
            ),
            None => "There is no debug log.".to_owned(),
        };
        ui.label(log);
    }

    /// "Dates look like 18/11/2026, and money like £1,234.56."
    fn formats_sentence(&self) -> String {
        let example = NaiveDate::from_ymd_opt(2026, 11, 18).expect("a real date");
        format!(
            "Dates look like {}, and money like {}.",
            self.fmt.short_date(example),
            self.fmt.money(123_456)
        )
    }

    /// Switches to the locale typed, or back to the computer's own when the box
    /// is empty, and remembers the choice.
    pub(super) fn use_locale(&mut self) {
        let name = self.locale_input.trim().to_owned();
        let wanted = (!name.is_empty()).then_some(name.as_str());
        match LocaleFormat::detect(wanted) {
            Ok(fmt) => {
                log::info!("locale changed to {}", fmt.describe());
                self.fmt = fmt;
                self.locale_input = name.clone();
                self.save_setting("locale", wanted);
                let whose = if name.is_empty() { "this computer's" } else { "the" };
                self.set_info(format!("Using {whose} {} locale. {}", self.fmt.locale, self.formats_sentence()));
            }
            Err(msg) => self.set_error(msg),
        }
    }
}
