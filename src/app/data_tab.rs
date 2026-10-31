//! The Import/Export tab: CSV, JSON and SQL files, and backing up the
//! database itself.

use super::*;
use crate::transfer::{self, Format};
use std::path::{Path, PathBuf};

/// The file name suggested when exporting, such as "budget-2026-10-30.csv".
fn suggested_name(extension: &str) -> String {
    format!("budget-{}.{extension}", money::today())
}

/// Whether `path` is the open database or one of its WAL files, which a
/// copy must never replace.
fn is_live_database(path: &Path, db_path: &Path) -> bool {
    let canonical = |p: &Path| std::fs::canonicalize(p).ok();
    let Some(target) = canonical(path) else {
        return false;
    };
    ["", "-wal", "-shm"].iter().any(|suffix| {
        let mut live = db_path.as_os_str().to_owned();
        live.push(suffix);
        canonical(Path::new(&live)).is_some_and(|live| live == target)
    })
}

/// Copies the database to `path`. VACUUM INTO only writes to a new or empty
/// file, so the copy goes into an empty temporary file beside `path` and is
/// renamed over it once complete; a failed copy leaves `path` as it was.
fn copy_database(db: &Db, path: &Path) -> anyhow::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new().prefix(".opentrack-copy-").suffix(".db").tempfile_in(dir)?;
    db.back_up(tmp.path())?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

impl BudgetApp {
    /// Ctrl+I: asks for a CSV or JSON file and imports it.
    pub(super) fn import_any(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(t!("data-import-title"))
            .add_filter(t!("data-filter-csv-or-json"), &["csv", "json"])
            .add_filter(t!("data-filter-csv"), &["csv"])
            .add_filter(t!("data-filter-json"), &["json"])
            .pick_file()
        else {
            return self.announce(t!("data-nothing-imported"));
        };
        match Format::of(&path) {
            Some(Format::Json) => self.import_json(path),
            _ => self.import_csv(path),
        }
    }

    /// Ctrl+E: asks where to save, and exports in the format of the file
    /// name chosen.
    pub(super) fn export_any(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(t!("data-export-title"))
            .set_file_name(suggested_name("csv"))
            .add_filter(t!("data-filter-csv"), &["csv"])
            .add_filter(t!("data-filter-json"), &["json"])
            .add_filter(t!("data-filter-sql"), &["sql"])
            .save_file()
        else {
            return self.announce(t!("data-nothing-exported"));
        };
        let format = Format::of(&path).unwrap_or(Format::Csv);
        self.export(format, &path);
    }

    fn choose_export(&mut self, format: Format) {
        let (extension, kind) = match format {
            Format::Csv => ("csv", t!("data-filter-csv")),
            Format::Json => ("json", t!("data-filter-json")),
            Format::Sql => ("sql", t!("data-filter-sql")),
        };
        match rfd::FileDialog::new().set_file_name(suggested_name(extension)).add_filter(kind, &[extension]).save_file() {
            Some(path) => self.export(format, &path),
            None => self.announce(t!("data-nothing-exported")),
        }
    }

    fn export(&mut self, format: Format, path: &Path) {
        log::info!("exporting as {format:?}");
        let file = file_name(path);
        let result = match format {
            Format::Csv => transfer::export_csv(&self.db, path).map(|n| t!("data-exported-csv", count = n, file = &file)),
            Format::Json => transfer::export_json(&self.db, path)
                .map(|(c, n)| t!("data-exported-json", categories = c, count = n, file = &file)),
            Format::Sql => transfer::export_sql(&self.db, path).map(|n| t!("data-exported-sql", count = n, file = &file)),
        };
        match result {
            Ok(message) => self.announce(message),
            Err(e) => self.show_error(t!("error-export", error = format!("{e:#}"))),
        }
    }

    fn choose_import(&mut self, format: Format) {
        let dialog = rfd::FileDialog::new();
        let dialog = match format {
            Format::Json => dialog.add_filter(t!("data-filter-json"), &["json"]),
            _ => dialog.add_filter(t!("data-filter-csv"), &["csv"]),
        };
        match (dialog.pick_file(), format) {
            (Some(path), Format::Json) => self.import_json(path),
            (Some(path), _) => self.import_csv(path),
            (None, _) => self.announce(t!("data-nothing-imported")),
        }
    }

    fn import_csv(&mut self, path: PathBuf) {
        log::info!("importing CSV");
        match transfer::import_csv(&mut self.db, &path) {
            Ok(done) => {
                self.reload();
                let mut message = t!("data-imported-csv", count = done.entries, file = file_name(&path));
                if done.new_categories > 0 {
                    message.push(' ');
                    message.push_str(&t!("data-imported-new-categories", count = done.new_categories));
                }
                self.announce(message);
            }
            Err(e) => self.show_error(t!("error-import-csv", file = file_name(&path), error = format!("{e:#}"))),
        }
    }

    fn import_json(&mut self, path: PathBuf) {
        let (replace_label, add_label) = (t!("data-replace"), t!("data-add"));
        let choice = rfd::MessageDialog::new()
            .set_title(t!("data-import-backup-title"))
            .set_description(t!("data-import-backup-question"))
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(replace_label.clone(), add_label.clone(), t!("cancel")))
            .set_level(rfd::MessageLevel::Warning)
            .show();
        let replace = match choice {
            rfd::MessageDialogResult::Custom(c) if c == replace_label => true,
            rfd::MessageDialogResult::Custom(c) if c == add_label => false,
            rfd::MessageDialogResult::Yes => true,
            rfd::MessageDialogResult::No => false,
            _ => return self.announce(t!("data-nothing-imported")),
        };
        log::info!("importing JSON, replacing: {replace}");
        match transfer::import_json(&mut self.db, &path, replace) {
            Ok((categories, n)) => {
                self.spending.clear_form(&self.region);
                self.category_form = CategoryForm::default();
                self.reload();
                let file = file_name(&path);
                self.announce(if replace {
                    t!("data-replaced", categories = categories, count = n, file = file)
                } else {
                    t!("data-added", count = n, categories = categories, file = file)
                });
            }
            Err(e) => self.show_error(t!("error-import-json", file = file_name(&path), error = format!("{e:#}"))),
        }
    }

    fn back_up_database(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("budget-{}.db", money::today()))
            .add_filter(t!("data-filter-db"), &["db", "sqlite"])
            .save_file()
        else {
            return self.announce(t!("data-no-copy"));
        };
        if is_live_database(&path, &self.db_path) {
            return self.show_error(t!("data-live-database"));
        }
        match copy_database(&self.db, &path) {
            Ok(()) => self.announce(t!("data-copied", file = file_name(&path))),
            Err(e) => self.show_error(t!("error-copy-database", error = format!("{e:#}"))),
        }
    }

    pub(super) fn data_tab(&mut self, ui: &mut Ui) {
        heading(ui, &t!("data-export-heading"));
        note(ui, &t!("data-export-note"));
        let csv = format!("{}{}", t!("data-export-csv"), self.keymap.hint(Action::Export));
        match button_row(ui, &[(&csv, true), (&t!("data-export-json"), true)]) {
            Some(0) => self.choose_export(Format::Csv),
            Some(_) => self.choose_export(Format::Json),
            None => {}
        }

        heading(ui, &t!("data-import-heading"));
        note(ui, &t!("data-import-note"));
        let import = format!("{}{}", t!("data-import-csv"), self.keymap.hint(Action::Import));
        match button_row(ui, &[(&import, true), (&t!("data-import-json"), true)]) {
            Some(0) => self.choose_import(Format::Csv),
            Some(_) => self.choose_import(Format::Json),
            None => {}
        }

        heading(ui, &t!("data-database-heading"));
        read_only_field(ui, &t!("data-database-location"), &self.db_path.display().to_string());
        if full_button(ui, &t!("data-copy-button"), true).clicked() {
            self.back_up_database();
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copying_replaces_a_file_but_never_the_live_database() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("budget.db");
        let db = Db::open(&live).unwrap();
        assert!(is_live_database(&live, &live));
        assert!(is_live_database(&dir.path().join(".").join("budget.db"), &live));

        let copy = dir.path().join("copy.db");
        std::fs::write(&copy, "old").unwrap();
        assert!(!is_live_database(&copy, &live));
        copy_database(&db, &copy).unwrap();
        assert_eq!(Db::open(&copy).unwrap().categories().unwrap().len(), 8);
        assert!(!dir.path().join("copy.db.old").exists());
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(!names.iter().any(|n| n.to_string_lossy().starts_with(".opentrack-copy-")), "{names:?}");
    }
}
