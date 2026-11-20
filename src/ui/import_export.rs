//! The Import/Export tab: adding spending from a CSV or JSON file, and saving
//! spending as CSV or JSON, or as a report in PDF or Word format. Files are
//! chosen in the operating system's own dialogs, which screen readers know.
//! Nothing is imported until the person has agreed to what the file holds.

use std::path::Path;

use chrono::Months;
use egui::Ui;

use super::widgets::{dropdown, full_button, heading};
use super::{App, file_and_folder, plural, today};
use crate::report::{self, Period};
use crate::transfer::{self, DataFormat, Imported};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportFormat {
    Csv,
    Json,
    Pdf,
    Docx,
}

impl ExportFormat {
    pub(super) const ALL: [ExportFormat; 4] = [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Pdf, ExportFormat::Docx];

    fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV, for spreadsheets (.csv)",
            ExportFormat::Json => "JSON data (.json)",
            ExportFormat::Pdf => "PDF report (.pdf)",
            ExportFormat::Docx => "Word report (.docx)",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Docx => "docx",
        }
    }

    /// What the save dialog calls files of this kind.
    fn kind(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV files",
            ExportFormat::Json => "JSON files",
            ExportFormat::Pdf => "PDF documents",
            ExportFormat::Docx => "Word documents",
        }
    }
}

/// "Gifts", "Gifts and Pets", "Gifts, Pets and Travel".
fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Writes beside the file, then puts it in place, so a file already there is
/// only replaced once the new one is complete.
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "it has no file name"))?;
    let partial = path.with_file_name(format!(".{}.part", name.to_string_lossy()));
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&partial);
    })
}

/// A file's contents, unless it is far bigger than spending could be.
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > transfer::MAX_FILE_BYTES {
        return Err(format!("it is larger than {} MB, too large to be a spending file", transfer::MAX_FILE_BYTES / 1024 / 1024));
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

impl App {
    pub(super) fn import_export_tab(&mut self, ui: &mut Ui) {
        heading(ui, "Import/Export", 2);

        heading(ui, "Import", 3);
        ui.label(
            "Adds the spending in a CSV or JSON file, such as one exported here, to the spending already here. A CSV \
             file's first line names its columns: Date, Name, Amount and Category. You are asked before anything is \
             saved, and categories that aren't here yet are added.",
        );
        if full_button(ui, "import", "Import from a file", None).clicked() {
            self.import_file();
        }

        heading(ui, "Export", 3);
        let scopes = ["All spending".to_owned(), format!("Spending in {}", self.fmt.month_year(self.month))];
        dropdown(ui, "export_scope", "What to export", &scopes, &mut self.export_scope);
        let formats = ExportFormat::ALL.map(|f| f.label().to_owned());
        dropdown(ui, "export_format", "Format", &formats, &mut self.export_format);
        ui.label("CSV and JSON files can be imported again. PDF and Word reports total the spending by category and list every entry.");
        if full_button(ui, "export", "Export to a file", None).clicked() {
            self.export_file();
        }
    }

    /// Asks for a file, reads and checks all of it, says what it holds, and
    /// saves it all if the person agrees. A file with a problem saves nothing.
    pub(super) fn import_file(&mut self) {
        let Some(path) = self.dialogs.open_file("Import spending", "CSV or JSON files", &["csv", "json", "txt"]) else {
            return self.set_info("Nothing was imported.");
        };
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let Some(format) = DataFormat::of(&path) else {
            return self.set_error(format!("Nothing was imported. {name} isn't a CSV or JSON file: its name must end in .csv or .json."));
        };
        let imported = match read_file(&path).and_then(|bytes| transfer::read(format, &bytes, &self.fmt, today())) {
            Ok(imported) => imported,
            Err(problem) => {
                log::info!("an import from a {format:?} file was refused");
                return self.set_error(format!("Nothing was imported from {name}. {problem}"));
            }
        };
        let new_categories = match self.db.missing_categories(imported.category_names()) {
            Ok(names) => names,
            Err(e) => return self.set_error(format!("Nothing was imported: your categories could not be read: {e}.")),
        };
        let Some(question) = self.import_question(&name, &imported, &new_categories) else {
            return self.set_info(format!("Nothing was imported. Everything in {name} is here already."));
        };
        if !self.dialogs.confirm("Import spending", &question) {
            return self.set_info("Nothing was imported.");
        }
        match self.db.import(&imported.categories, &imported.entries) {
            Ok(added) => {
                log::info!("imported {} entries and {} categories from a {format:?} file", imported.entries.len(), added.len());
                self.refresh();
                let mut said = format!("Imported {} from {name}.", plural(imported.entries.len(), "entry", "entries"));
                if !added.is_empty() {
                    said.push_str(&format!(" Added {}: {}.", plural(added.len(), "category", "categories"), and_list(&added)));
                }
                self.set_info(said);
            }
            Err(e) => {
                log::error!("could not import: {e}");
                self.set_error(format!("Nothing was imported from {name}: {e}."));
            }
        }
    }

    /// What importing would add, as a question, or nothing if it would add nothing.
    fn import_question(&self, name: &str, imported: &Imported, new_categories: &[String]) -> Option<String> {
        let categories = match new_categories.len() {
            0 => String::new(),
            1 => format!("the category {}", new_categories[0]),
            n => format!("{n} categories: {}", and_list(new_categories)),
        };
        let entries = &imported.entries;
        let (Some(first), Some(last)) = (entries.iter().map(|e| e.date).min(), entries.iter().map(|e| e.date).max()) else {
            return (!new_categories.is_empty()).then(|| format!("Add {categories}, from {name}?"));
        };
        let dates = if first == last {
            format!("dated {}", self.fmt.long_date(first))
        } else {
            format!("dated {} to {}", self.fmt.long_date(first), self.fmt.long_date(last))
        };
        let total = self.fmt.money(entries.iter().map(|e| e.pence).sum());
        let mut question = format!("Import {} from {name}, {dates}, totalling {total}?", plural(entries.len(), "entry", "entries"));
        if !categories.is_empty() {
            question.push_str(&format!(" This also adds {categories}."));
        }
        Some(question)
    }

    /// Saves the chosen spending in the chosen format, where the person says.
    pub(super) fn export_file(&mut self) {
        let format = ExportFormat::ALL[self.export_format.min(ExportFormat::ALL.len() - 1)];
        let all = self.export_scope == 0;
        let entries = if all { self.db.all_spending() } else { self.db.spending_between(self.month, self.month + Months::new(1)) };
        let entries = match entries {
            Ok(entries) => entries,
            Err(e) => {
                log::error!("could not read spending to export: {e}");
                return self.set_error(format!("Your spending could not be read: {e}."));
            }
        };
        if entries.is_empty() {
            let what = if all { "no spending recorded".to_owned() } else { format!("no spending in {}", self.fmt.month_year(self.month)) };
            return self.set_error(format!("There is {what}, so there is nothing to export."));
        }
        let today = today();
        let kind = if matches!(format, ExportFormat::Pdf | ExportFormat::Docx) { "report" } else { "spending" };
        let covers = if all { format!("all-{}", today.format("%Y-%m-%d")) } else { self.month.format("%Y-%m").to_string() };
        let suggested = format!("opentrack-{kind}-{covers}.{}", format.extension());
        let Some(mut path) = self.dialogs.save_file("Export spending", &suggested, format.kind(), &[format.extension()]) else {
            return self.set_info("Nothing was exported.");
        };
        if path.extension().is_none() {
            path.set_extension(format.extension());
        }
        let bytes = match format {
            ExportFormat::Csv => transfer::to_csv(&entries),
            ExportFormat::Json => {
                let categories: Vec<String> = self.categories.iter().map(|c| c.short.clone()).collect();
                transfer::to_json(&entries, &categories, today)
            }
            ExportFormat::Pdf | ExportFormat::Docx => {
                let period = if all { Period::All } else { Period::Month(self.month) };
                let built = report::build(&entries, period, &self.fmt, today);
                if format == ExportFormat::Pdf { report::pdf(&built, today) } else { report::docx(&built, today) }
            }
        };
        match write_file(&path, &bytes) {
            Ok(()) => {
                log::info!("exported {} entries as {format:?}", entries.len());
                self.set_info(format!("Exported {} to {}.", plural(entries.len(), "entry", "entries"), file_and_folder(&path)));
            }
            Err(e) => {
                log::error!("could not export: {e}");
                self.set_error(format!("{} could not be saved: {e}.", file_and_folder(&path)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_read_naturally() {
        let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(and_list(&names(&["Gifts"])), "Gifts");
        assert_eq!(and_list(&names(&["Gifts", "Pets"])), "Gifts and Pets");
        assert_eq!(and_list(&names(&["Gifts", "Pets", "Travel"])), "Gifts, Pets and Travel");
    }

    #[test]
    fn a_file_is_replaced_only_when_the_new_one_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.csv");
        std::fs::write(&path, "old").unwrap();
        write_file(&path, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "no partial file is left behind");
        assert!(write_file(&dir.path().join("missing/out.csv"), b"x").is_err());
    }
}
