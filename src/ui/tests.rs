//! Drives the whole window the way a keyboard and screen reader user would:
//! pressing keys, then looking at the accessibility tree AccessKit gives to
//! screen readers. Nothing here uses the pointer.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::{Datelike, Locale, Months};
use egui::accesskit::{Live, Role};
use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

use super::*;

/// Stands in for the native dialogs: gives a set answer, and remembers asks.
#[derive(Clone, Default)]
struct Script {
    confirm: Rc<RefCell<bool>>,
    /// The file chosen in an open or save dialog; none is cancelling.
    file: Rc<RefCell<Option<PathBuf>>>,
    asked: Rc<RefCell<Vec<String>>>,
}

impl Dialogs for Script {
    fn confirm(&self, title: &str, question: &str) -> bool {
        self.asked.borrow_mut().push(format!("{title}: {question}"));
        *self.confirm.borrow()
    }

    fn open_file(&self, title: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf> {
        self.asked.borrow_mut().push(format!("{title}: {kind} {extensions:?}"));
        self.file.borrow().clone()
    }

    fn save_file(&self, title: &str, name: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf> {
        self.asked.borrow_mut().push(format!("{title}: {name} {kind} {extensions:?}"));
        self.file.borrow().clone()
    }
}

/// The app is made during the first frame, so that it can use the harness's context.
type Window = Harness<'static, Option<App>>;

/// British formats, whatever the computer running the tests uses.
fn en_gb() -> LocaleFormat {
    LocaleFormat::new(Locale::en_GB, "test")
}

fn open(db: Db) -> (Window, Script) {
    let script = Script::default();
    let dialogs = script.clone();
    let mut parts = Some((db, dialogs));
    let mut harness = Harness::builder().with_size(egui::vec2(1000.0, 1400.0)).build_ui_state(
        move |ui, app: &mut Option<App>| {
            let app = app.get_or_insert_with(|| {
                let (db, dialogs) = parts.take().expect("made once");
                App::new(ui.ctx(), db, en_gb(), Box::new(dialogs), Paths::default(), String::new())
            });
            app.show(ui);
        },
        None,
    );
    harness.run_steps(3);
    (harness, script)
}

fn window() -> (Window, Script) {
    open(Db::open_in_memory().unwrap())
}

fn app(h: &Window) -> &App {
    h.state().as_ref().unwrap()
}

fn app_mut(h: &mut Window) -> &mut App {
    h.state_mut().as_mut().unwrap()
}

fn press(h: &mut Window, key: Key) {
    h.key_press(key);
    h.run_steps(3);
}

fn press_with(h: &mut Window, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.run_steps(3);
}

fn command(h: &mut Window, key: Key) {
    press_with(h, Modifiers::COMMAND, key);
}

/// Escape, as the real window's input hook handles it.
fn escape(h: &mut Window) {
    let mut input = egui::RawInput {
        events: vec![
            Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
            Event::Key { key: Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
        ],
        ..Default::default()
    };
    app_mut(h).take_escape(&mut input);
    assert!(input.events.is_empty(), "egui must not see Escape, or it would drop the focus");
    h.run_steps(3);
}

fn type_text(h: &mut Window, text: &str) {
    h.event(Event::Text(text.to_owned()));
    h.run_steps(2);
}

/// What a screen reader would say the focused control is called.
fn focused(h: &Window) -> Option<String> {
    let node = h.root().children_recursive().find(|n| n.accesskit_node().is_focused())?;
    Some(node.accesskit_node().label().unwrap_or_else(|| "(no name)".to_owned()))
}

/// Presses Tab until the focus comes back round, giving each stop's name.
fn tab_stops(h: &mut Window, modifiers: Modifiers) -> Vec<String> {
    let mut stops = Vec::new();
    for _ in 0..60 {
        press_with(h, modifiers, Key::Tab);
        let Some(name) = focused(h) else { continue };
        if stops.first() == Some(&name) {
            return stops;
        }
        stops.push(name);
    }
    panic!("the focus never came back round: {stops:?}");
}

fn status(h: &Window) -> String {
    h.root()
        .children_recursive()
        .find(|n| n.accesskit_node().live() != Live::Off)
        .and_then(|n| n.accesskit_node().value().or(n.accesskit_node().label()))
        .expect("the status bar is a live region")
}

fn status_live(h: &Window) -> Live {
    h.root().children_recursive().find(|n| n.accesskit_node().live() != Live::Off).unwrap().accesskit_node().live()
}

fn node_by_label<'a>(h: &'a Window, label: &str) -> egui_kittest::Node<'a> {
    h.root()
        .children_recursive()
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.contains(label)))
        .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
}

fn has_label(h: &Window, label: &str) -> bool {
    h.root().children_recursive().any(|n| n.accesskit_node().label().is_some_and(|l| l.contains(label)))
}

fn focus_on(h: &mut Window, label: &str) {
    for _ in 0..60 {
        if focused(h).is_some_and(|f| f.contains(label)) {
            return;
        }
        press(h, Key::Tab);
    }
    panic!("the keyboard can't reach {label:?}");
}

/// Types into the box labelled `label`, replacing nothing (they start empty).
fn fill(h: &mut Window, label: &str, text: &str) {
    focus_on(h, label);
    type_text(h, text);
}

fn add_tea(h: &mut Window) {
    fill(h, "What you bought", "Tea");
    fill(h, "Amount", "2.50");
    command(h, Key::S);
}

#[test]
fn the_keyboard_reaches_every_control_on_spending_in_order() {
    let db = Db::open_in_memory().unwrap();
    db.add_spending("Cinema", today(), 1250, 22).unwrap();
    let (mut h, _) = open(db);
    assert_eq!(focused(&h).as_deref(), Some("Spending"), "the focus starts on the Spending tab");
    let stops = tab_stops(&mut h, Modifiers::NONE);
    let expected = [
        "Categories",
        "Import/Export",
        "System Settings",
        "What you bought",
        "Amount, for example 12.50",
        "Date, such as",
        "Category",
        "Save spending",
        "Previous month",
        "This month",
        "Next month",
        "Spending in",
        "Entry to delete",
        "Delete this entry",
        "Summary",
        "Spending",
    ];
    assert_eq!(stops.len(), expected.len(), "{stops:?}");
    for (stop, wanted) in stops.iter().zip(expected) {
        assert!(stop.contains(wanted), "{stop:?} should be {wanted:?}: {stops:?}");
    }
    h.check_accessibility();
}

#[test]
fn tabs_are_tabs_and_say_which_is_selected() {
    let (mut h, _) = window();
    let tabs: Vec<_> = h.root().children_recursive().filter(|n| n.accesskit_node().role() == Role::Tab).collect();
    let names: Vec<String> = tabs.iter().map(|t| t.accesskit_node().label().unwrap()).collect();
    assert_eq!(names, ["Summary", "Spending", "Categories", "Import/Export", "System Settings"]);
    assert!(tabs[1].accesskit_node().is_selected().unwrap());
    command(&mut h, Key::Num1);
    assert_eq!(app(&h).tab, Tab::Summary);
    assert_eq!(focused(&h).as_deref(), Some("Summary"), "the focus goes to the tab");
    assert!(node_by_label(&h, "Summary").accesskit_node().is_selected().unwrap());
}

#[test]
fn spending_is_added_from_the_keyboard() {
    let (mut h, _) = window();
    fill(&mut h, "What you bought", "Tea");
    fill(&mut h, "Amount", "2.50");
    // Category stays on the first, Entertainment; choose Food and Drink.
    focus_on(&mut h, "Category");
    press(&mut h, Key::ArrowDown);
    command(&mut h, Key::S);
    let said = status(&h);
    assert!(said.starts_with("Saved. Entry 1: Tea, £2.50, on "), "{said}");
    assert!(said.ends_with(", in Food and Drink."), "{said}");
    assert_eq!(status_live(&h), Live::Polite);
    // Ready for the next one.
    assert_eq!(app(&h).name_input, "");
    assert_eq!(app(&h).amount_input, "");
    assert_eq!(focused(&h).as_deref(), Some("What you bought"));
    // And it is in this month's list, on the same tab.
    assert!(node_by_label(&h, "Spending in").accesskit_node().value().unwrap().contains("Entry 1: Tea, £2.50"));
}

#[test]
fn enter_in_a_box_saves_too() {
    let (mut h, _) = window();
    fill(&mut h, "What you bought", "Bus fare");
    fill(&mut h, "Amount", "1,80");
    press(&mut h, Key::Enter);
    assert!(status(&h).starts_with("Saved. Entry 1: Bus fare, £1.80"), "{}", status(&h));
}

#[test]
fn a_mistake_is_announced_at_once_and_the_focus_goes_to_it() {
    let (mut h, _) = window();
    command(&mut h, Key::S);
    assert_eq!(status(&h), "Type what you bought first.");
    assert_eq!(status_live(&h), Live::Assertive, "mistakes interrupt");
    assert_eq!(focused(&h).as_deref(), Some("What you bought"));

    type_text(&mut h, "Tea");
    command(&mut h, Key::S);
    assert_eq!(status(&h), "Type how much it was.");
    assert!(focused(&h).unwrap().starts_with("Amount"));

    type_text(&mut h, "lots");
    command(&mut h, Key::S);
    assert!(status(&h).contains("\"lots\" is not an amount"), "{}", status(&h));
    assert!(focused(&h).unwrap().starts_with("Amount"));
    assert_eq!(app(&h).amount_input, "lots", "what was typed is kept to fix");

    app_mut(&mut h).amount_input = "2".to_owned();
    fill(&mut h, "Date", "soon");
    command(&mut h, Key::S);
    assert!(status(&h).contains("\"soon\" is not a date"), "{}", status(&h));
    assert!(focused(&h).unwrap().starts_with("Date"));
}

#[test]
fn a_repeated_message_is_still_announced() {
    let (mut h, _) = window();
    command(&mut h, Key::S);
    let first = status(&h);
    command(&mut h, Key::S);
    assert_ne!(status(&h), first, "screen readers ignore text that doesn't change");
    assert_eq!(status(&h).trim_end_matches('\u{a0}'), first);
    press(&mut h, Key::F2);
    assert_eq!(status(&h), first, "F2 says it again by changing it back");
}

#[test]
fn months_can_be_changed_and_are_announced() {
    let (mut h, _) = window();
    command(&mut h, Key::Num2);
    let this = app(&h).month;
    focus_on(&mut h, "Previous month");
    press(&mut h, Key::Space);
    let previous = this - Months::new(1);
    assert_eq!(app(&h).month, previous);
    let name = en_gb().month_year(previous);
    assert_eq!(status(&h), format!("Showing {name}: No spending recorded."));
    assert_eq!(focused(&h).as_deref(), Some("Previous month"), "the focus stays put");
    press_with(&mut h, Modifiers::COMMAND, Key::PageDown);
    assert_eq!(app(&h).month, this);
    // The Summary tab shows the same month.
    command(&mut h, Key::Num1);
    command(&mut h, Key::PageUp);
    assert_eq!(app(&h).month, previous);
    focus_on(&mut h, "This month");
    press(&mut h, Key::Enter);
    assert_eq!(app(&h).month.month(), today().month());
}

#[test]
fn the_month_buttons_share_a_line() {
    let (mut h, _) = window();
    command(&mut h, Key::Num2);
    let rects: Vec<_> =
        ["Previous month", "This month", "Next month"].iter().map(|l| node_by_label(&h, l).rect()).collect();
    assert!(rects.iter().all(|r| (r.top() - rects[0].top()).abs() < 1.0), "{rects:?}");
}

#[test]
fn save_spending_is_beside_the_category() {
    let (h, _) = window();
    let category = node_by_label(&h, "Category").rect();
    let save = node_by_label(&h, "Save spending").rect();
    assert!((category.top() - save.top()).abs() < 1.0, "{category:?} {save:?}");
    assert!(save.left() > category.right(), "the button is to the right");
    assert_eq!(node_by_label(&h, "Category").accesskit_node().role(), Role::ComboBox, "still named by its label");
}

#[test]
fn in_a_narrow_window_save_spending_goes_below_the_category() {
    let (mut h, _) = window();
    // Text this large leaves no room beside the longest category.
    h.ctx.set_zoom_factor(3.0);
    h.run_steps(3);
    let category = node_by_label(&h, "Category").rect();
    let save = node_by_label(&h, "Save spending").rect();
    assert!(save.top() >= category.bottom(), "{category:?} {save:?}");
    focus_on(&mut h, "Category");
    press(&mut h, Key::Tab);
    assert_eq!(focused(&h).as_deref(), Some("Save spending"));
}

#[test]
fn an_entry_is_deleted_only_when_confirmed() {
    let db = Db::open_in_memory().unwrap();
    db.add_spending("Cinema", today(), 1250, 22).unwrap();
    db.add_spending("Pizza", today(), 900, 23).unwrap();
    let (mut h, script) = open(db);
    command(&mut h, Key::Num2);
    focus_on(&mut h, "Entry to delete");
    press(&mut h, Key::ArrowDown);
    focus_on(&mut h, "Delete this entry");
    press(&mut h, Key::Space);
    assert!(script.asked.borrow()[0].contains("Entry 2: Pizza, £9.00"), "{:?}", script.asked.borrow());
    assert_eq!(status(&h), "Nothing was deleted.");
    assert_eq!(app(&h).entries.len(), 2);

    *script.confirm.borrow_mut() = true;
    press(&mut h, Key::Space);
    assert!(status(&h).starts_with("Deleted entry 2. "), "{}", status(&h));
    assert_eq!(app(&h).entries.len(), 1);
    assert_eq!(focused(&h).as_deref(), Some("Delete this entry"));

    // Deleting the last one takes the button away, so the focus goes to the tab.
    press(&mut h, Key::Space);
    assert!(app(&h).entries.is_empty());
    assert!(!has_label(&h, "Delete this entry"));
    assert_eq!(focused(&h).as_deref(), Some("Spending"));
}

#[test]
fn the_summary_gives_each_category_with_its_share() {
    let db = Db::open_in_memory().unwrap();
    db.add_spending("Cinema", today(), 1250, 22).unwrap();
    db.add_spending("Pizza", today(), 900, 23).unwrap();
    let (mut h, _) = open(db);
    command(&mut h, Key::Num1);
    let text = node_by_label(&h, "by category").accesskit_node().value().unwrap();
    assert_eq!(
        text,
        "Entertainment: £12.50, from 1 entry, 58 percent of the total.\nFood and Drink: £9.00, from 1 entry, 42 percent of the total."
    );
    assert!(has_label(&h, "") && h.root().children_recursive().any(|n| n
        .accesskit_node()
        .value()
        .is_some_and(|v| v == "2 entries, totalling £21.50.")));
}

#[test]
fn a_category_is_added_and_can_be_chosen() {
    let (mut h, _) = window();
    command(&mut h, Key::Num3);
    fill(&mut h, "Name of the new category", "Gifts");
    press(&mut h, Key::Enter);
    assert_eq!(status(&h), "Added the category Gifts. You can choose it on the Spending tab.");
    assert_eq!(focused(&h).as_deref(), Some("Name of the new category, up to 32 characters"));
    assert!(node_by_label(&h, "Categories, one on each line").accesskit_node().value().unwrap().ends_with("Gifts"));

    type_text(&mut h, "gifts");
    command(&mut h, Key::S);
    assert_eq!(status(&h), "There is already a category called gifts.");
    assert_eq!(status_live(&h), Live::Assertive);

    command(&mut h, Key::Num2);
    focus_on(&mut h, "Category");
    press(&mut h, Key::End);
    assert_eq!(app(&h).categories[app(&h).category_index].short, "Gifts");
}

#[test]
fn the_appearance_is_changed_and_remembered() {
    let (mut h, _) = window();
    command(&mut h, Key::Num5);
    focus_on(&mut h, "Colours and text");
    press(&mut h, Key::ArrowDown);
    assert_eq!(app(&h).appearance, Appearance::HighContrast);
    assert_eq!(status(&h), "Using the High Contrast appearance.");
    assert_eq!(app(&h).db.setting("appearance").unwrap().as_deref(), Some("high_contrast"));
    assert!(h.ctx.global_style().visuals.dark_mode);
}

#[test]
fn the_locale_is_changed_and_remembered() {
    let (mut h, _) = window();
    command(&mut h, Key::Num5);
    fill(&mut h, "Locale", "de_DE");
    press(&mut h, Key::Enter);
    assert_eq!(status(&h), "Using the de_DE locale. Dates look like 18.11.2026, and money like 1.234,56 €.");
    assert_eq!(app(&h).db.setting("locale").unwrap().as_deref(), Some("de_DE"));
    assert!(focused(&h).unwrap().starts_with("Locale"), "Enter keeps the focus in the box");
    command(&mut h, Key::Num2);
    assert!(has_label(&h, "Amount, for example 12,50"), "the Spending tab follows it");

    command(&mut h, Key::Num5);
    focus_on(&mut h, "Locale");
    type_text(&mut h, "x");
    command(&mut h, Key::S);
    assert!(status(&h).starts_with("Unknown locale \"de_DEx\""), "{}", status(&h));
    assert_eq!(app(&h).fmt.locale, Locale::de_DE, "a mistake changes nothing");
}

#[test]
fn f1_shows_the_shortcuts_and_puts_the_focus_back() {
    let (mut h, _) = window();
    focus_on(&mut h, "Amount");
    press(&mut h, Key::F1);
    assert_eq!(focused(&h).as_deref(), Some("Keyboard shortcuts, one on each line"));
    escape(&mut h);
    assert!(focused(&h).unwrap().starts_with("Amount"));
    press(&mut h, Key::F1);
    press(&mut h, Key::F1);
    assert!(focused(&h).unwrap().starts_with("Amount"));
}

#[test]
fn ctrl_n_goes_to_the_first_box() {
    let (mut h, _) = window();
    command(&mut h, Key::Num3);
    command(&mut h, Key::N);
    assert_eq!(app(&h).tab, Tab::Spending);
    assert_eq!(focused(&h).as_deref(), Some("What you bought"));
}

#[test]
fn the_shortcuts_match_the_readme() {
    let (h, _) = window();
    let readme = include_str!("../../README.md");
    for (_, action) in shortcuts::describe(&h.ctx) {
        assert!(action.starts_with(char::is_uppercase), "{action}");
        assert!(readme.contains(&format!("| {action} |")), "the README's Keyboard section is missing {action:?}");
    }
}

#[test]
fn every_tab_passes_the_accessible_name_check() {
    let db = Db::open_in_memory().unwrap();
    db.add_spending("Cinema", today(), 1250, 22).unwrap();
    let (mut h, _) = open(db);
    for key in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5] {
        command(&mut h, key);
        h.check_accessibility();
        for stop in tab_stops(&mut h, Modifiers::NONE) {
            assert_ne!(stop, "(no name)");
            assert!(!stop.trim().is_empty());
        }
    }
}

#[test]
fn the_tab_icons_are_in_the_fonts() {
    let (h, _) = window();
    let font = egui::FontId::proportional(15.0);
    for tab in Tab::ALL {
        let icon = tab.label().chars().next().unwrap();
        assert!(h.ctx.clone().fonts_mut(|f| f.has_glyph(&font, icon)), "{tab:?}'s icon {icon:?} has no glyph");
    }
}

#[test]
fn a_problem_at_startup_is_announced() {
    let script = Script::default();
    let mut h = Harness::builder().build_ui_state(
        move |ui, app: &mut Option<App>| {
            let app = app.get_or_insert_with(|| {
                let db = Db::open_in_memory().unwrap();
                App::new(ui.ctx(), db, en_gb(), Box::new(script.clone()), Paths::default(), "It went wrong.".into())
            });
            app.show(ui);
        },
        None,
    );
    h.run_steps(3);
    assert_eq!(status(&h), "It went wrong.");
    assert_eq!(status_live(&h), Live::Assertive);
}

#[test]
fn adding_more_spending_keeps_the_list_up_to_date() {
    let (mut h, _) = window();
    add_tea(&mut h);
    add_tea(&mut h);
    assert_eq!(app(&h).entries.len(), 2);
}

// ----- import and export -----------------------------------------------------

/// Chooses an export format and what to export, then presses Export.
fn export(h: &mut Window, scope_downs: usize, format_downs: usize) {
    command(h, Key::Num4);
    focus_on(h, "What to export");
    press(h, Key::Home);
    for _ in 0..scope_downs {
        press(h, Key::ArrowDown);
    }
    focus_on(h, "Format");
    press(h, Key::Home);
    for _ in 0..format_downs {
        press(h, Key::ArrowDown);
    }
    focus_on(h, "Export to a file");
    press(h, Key::Space);
}

#[test]
fn the_keyboard_reaches_every_control_on_import_export() {
    let (mut h, _) = window();
    command(&mut h, Key::Num4);
    assert_eq!(focused(&h).as_deref(), Some("Import/Export"));
    let stops = tab_stops(&mut h, Modifiers::NONE);
    let expected =
        ["System Settings", "Import from a file", "What to export", "Format", "Export to a file", "Summary", "Spending", "Categories", "Import/Export"];
    assert_eq!(stops, expected);
    h.check_accessibility();
}

#[test]
fn spending_is_exported_in_each_format_and_imported_back() {
    let db = Db::open_in_memory().unwrap();
    db.add_spending("Cinema", today(), 1250, 22).unwrap();
    db.add_spending("Old tea", today() - Months::new(2), 250, 23).unwrap();
    db.add_category("Unused").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (mut h, script) = open(db);
    *script.confirm.borrow_mut() = true;

    for (format, name) in [(0, "all.csv"), (1, "all.json"), (2, "all.pdf"), (3, "all")] {
        *script.file.borrow_mut() = Some(dir.path().join(name));
        export(&mut h, 0, format);
        let said = status(&h);
        assert!(said.starts_with("Exported 2 entries to "), "{said}");
        assert_eq!(status_live(&h), Live::Polite);
    }
    let asked = script.asked.borrow().clone();
    let all = format!("opentrack-spending-all-{}.csv", today().format("%Y-%m-%d"));
    assert!(asked[0].starts_with(&format!("Export spending: {all} CSV files [\"csv\"]")), "{asked:?}");
    assert!(asked[2].contains("opentrack-report-all-"), "{asked:?}");
    // The extension is added if the name has none.
    assert!(dir.path().join("all.docx").is_file());
    assert!(std::fs::read(dir.path().join("all.pdf")).unwrap().starts_with(b"%PDF-"));
    assert!(std::fs::read_to_string(dir.path().join("all.csv")).unwrap().contains(",Cinema,12.50,Entertainment"));

    // Only the month on show.
    *script.file.borrow_mut() = Some(dir.path().join("month.csv"));
    export(&mut h, 1, 0);
    assert!(status(&h).starts_with("Exported 1 entry to month.csv in "), "{}", status(&h));

    // Cancelling saves nothing.
    *script.file.borrow_mut() = None;
    export(&mut h, 0, 0);
    assert_eq!(status(&h), "Nothing was exported.");

    // Into a new database: the JSON brings the unused category too.
    let (mut h, script) = window();
    *script.file.borrow_mut() = Some(dir.path().join("all.json"));
    command(&mut h, Key::Num4);
    focus_on(&mut h, "Import from a file");
    press(&mut h, Key::Space);
    let question = script.asked.borrow().last().unwrap().clone();
    assert!(question.starts_with("Import spending: Import 2 entries from all.json, dated "), "{question}");
    assert!(question.ends_with(", totalling £15.00? This also adds the category Unused."), "{question}");
    assert_eq!(status(&h), "Nothing was imported.", "no answer is no");
    assert!(app(&h).db.all_spending().unwrap().is_empty());

    *script.confirm.borrow_mut() = true;
    press(&mut h, Key::Space);
    assert_eq!(status(&h), "Imported 2 entries from all.json. Added 1 category: Unused.");
    assert_eq!(focused(&h).as_deref(), Some("Import from a file"), "the focus stays put");
    assert_eq!(app(&h).entries.len(), 1, "this month's list is up to date");
    assert!(app(&h).categories.iter().any(|c| c.short == "Unused"));

    // The same again, as CSV, adds the entries a second time; nothing is new.
    *script.file.borrow_mut() = Some(dir.path().join("all.csv"));
    press(&mut h, Key::Space);
    assert_eq!(status(&h), "Imported 2 entries from all.csv.");
    assert_eq!(app(&h).db.all_spending().unwrap().len(), 4);
}

#[test]
fn a_file_with_a_problem_imports_nothing_and_says_where() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bank.csv");
    std::fs::write(&path, "Date,Name,Amount,Category\n2026-11-18,Tea,2.50,Food\n2026-11-19,Cake,lots,Food\n").unwrap();
    let (mut h, script) = window();
    *script.confirm.borrow_mut() = true;
    *script.file.borrow_mut() = Some(path);
    command(&mut h, Key::Num4);
    focus_on(&mut h, "Import from a file");
    press(&mut h, Key::Space);
    assert_eq!(
        status(&h),
        "Nothing was imported from bank.csv. Line 3 has an amount, \"lots\", that isn't a number such as 12.50."
    );
    assert_eq!(status_live(&h), Live::Assertive);
    assert!(app(&h).db.all_spending().unwrap().is_empty());
    assert_eq!(script.asked.borrow().len(), 1, "nothing is asked about a file with a problem");

    *script.file.borrow_mut() = Some(dir.path().join("notes.pdf"));
    press(&mut h, Key::Space);
    assert!(status(&h).contains("notes.pdf isn't a CSV or JSON file"), "{}", status(&h));
    *script.file.borrow_mut() = Some(dir.path().join("missing.csv"));
    press(&mut h, Key::Space);
    assert!(status(&h).starts_with("Nothing was imported from missing.csv. "), "{}", status(&h));
}

#[test]
fn exporting_nothing_says_so() {
    let (mut h, script) = window();
    export(&mut h, 0, 2);
    assert_eq!(status(&h), "There is no spending recorded, so there is nothing to export.");
    assert_eq!(status_live(&h), Live::Assertive);
    assert!(script.asked.borrow().is_empty(), "no dialog opens");
}

#[test]
fn the_log_does_not_record_what_was_bought() {
    // The status bar says what was bought and for how much; the log mustn't.
    let source = include_str!("mod.rs");
    let announce = &source[source.find("fn announce").unwrap()..];
    let announce = &announce[..announce.find("\n    }").unwrap()];
    assert!(!announce.contains("{text}"), "{announce}");
}

// ----- update checks ---------------------------------------------------------
// None of these reach GitHub: they hand the window the answer a check would give.

fn version_999() -> crate::update::Version {
    crate::update::Version::parse("999.0.0").unwrap()
}

/// The answer arrives from the checking thread during a frame, as it does when
/// the app runs, so the web browser can be asked to open a page. Returns the
/// page it was asked to open, if any.
fn update_arrives(h: &mut Window, result: Result<Option<crate::update::Version>, String>, requested: bool) -> Option<String> {
    app(h).update_tx.send((result, requested)).unwrap();
    h.step();
    let opened = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
        _ => None,
    });
    h.run_steps(2);
    opened
}

#[test]
fn a_new_version_found_at_startup_offers_the_download_page() {
    let (mut h, script) = window();
    *script.confirm.borrow_mut() = true;
    assert_eq!(update_arrives(&mut h, Ok(Some(version_999())), false).as_deref(), Some(crate::update::RELEASES_URL));
    assert!(script.asked.borrow()[0].starts_with("Update available: Version 999.0.0 of Open Budget Tracking"));
    assert_eq!(status(&h), "Opening the download page for version 999.0.0 in your web browser.");

    let (mut h, script) = window();
    assert_eq!(update_arrives(&mut h, Ok(Some(version_999())), false), None, "no opens nothing");
    assert_eq!(script.asked.borrow().len(), 1);
    assert!(status(&h).contains("System Settings tab has a link"), "the news is still in the status bar: {}", status(&h));
    assert_eq!(status_live(&h), Live::Polite, "news, not an error");
}

#[test]
fn startup_checks_stay_quiet_unless_there_is_news() {
    let (mut h, script) = window();
    update_arrives(&mut h, Ok(None), false);
    update_arrives(&mut h, Err("GitHub could not be reached.".to_owned()), false);
    assert_eq!(status(&h), "Ready.");
    // A problem already being shown isn't covered up by news.
    app_mut(&mut h).set_error("Your settings could not be read.");
    update_arrives(&mut h, Ok(Some(version_999())), false);
    assert_eq!(status(&h), "Your settings could not be read.");
    assert_eq!(script.asked.borrow().len(), 1, "the dialog still offers it");
}

#[test]
fn a_new_version_is_linked_to_on_the_settings_tab() {
    let (mut h, script) = window();
    command(&mut h, Key::Num5);
    let says = |h: &Window, text: &str| h.root().children_recursive().any(|n| n.accesskit_node().value().is_some_and(|v| v == text));
    assert!(says(&h, &format!("This is version {}.", env!("CARGO_PKG_VERSION"))));
    let links = |h: &Window| h.root().children_recursive().filter(|n| n.accesskit_node().role() == Role::Link).count();
    assert_eq!(links(&h), 0);
    // Checking by hand asks nothing, and answers in the status bar.
    update_arrives(&mut h, Ok(Some(version_999())), true);
    assert!(script.asked.borrow().is_empty(), "{:?}", script.asked.borrow());
    assert!(status(&h).starts_with("Version 999.0.0 of Open Budget Tracking is available."), "{}", status(&h));
    assert_eq!(links(&h), 1);
    focus_on(&mut h, "Download version 999.0.0 from GitHub");
    let key = |pressed| Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    h.event(key(true));
    h.step();
    let opened = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
        _ => None,
    });
    h.event(key(false));
    h.run_steps(2);
    assert_eq!(opened.as_deref(), Some(crate::update::RELEASES_URL));
}

#[test]
fn checking_by_hand_always_answers() {
    let (mut h, _) = window();
    update_arrives(&mut h, Ok(None), true);
    assert_eq!(status(&h), format!("You have the latest version, {}.", env!("CARGO_PKG_VERSION")));
    update_arrives(&mut h, Err("GitHub could not be reached.".to_owned()), true);
    assert_eq!(status(&h), "Could not check for updates. GitHub could not be reached.");
    assert_eq!(status_live(&h), Live::Assertive);
}

#[test]
fn update_checks_can_be_turned_off_from_the_keyboard() {
    let (mut h, _) = window();
    assert!(app(&h).check_updates, "on unless turned off");
    command(&mut h, Key::Num5);
    focus_on(&mut h, "Check for updates when the app starts");
    press(&mut h, Key::ArrowDown);
    assert!(!app(&h).check_updates);
    assert_eq!(app(&h).db.setting("check_updates").unwrap().as_deref(), Some("0"));
    assert_eq!(status(&h), "The app won't check for updates when it starts.");
    focus_on(&mut h, "Check for updates now");
}

#[test]
fn turning_update_checks_off_is_remembered() {
    let db = Db::open_in_memory().unwrap();
    db.set_setting("check_updates", Some("0")).unwrap();
    let (mut h, _) = open(db);
    assert!(!app(&h).check_updates);
    // So starting up asks nothing of GitHub.
    app_mut(&mut h).check_for_updates_at_startup();
    assert!(!app(&h).checking_for_updates);
}
