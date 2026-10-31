//! The language of the interface. Every message the app shows is looked up
//! by its id in a Fluent file in `assets/i18n`, one for each language and
//! built into the app, so a translation only needs a new file. English is
//! used for any message a translation doesn't have.
//!
//! The language follows the system's unless one is chosen on the Settings
//! tab. It is separate from the region (`locale.rs`), which decides how
//! dates and amounts are written: the app can be in English with dates
//! written the German way.
//!
//! Messages are shown with `t!("id")`, or `t!("id", name = value)` for one
//! with values in it, which Fluent calls variables. Numbers passed as
//! values choose the right plural, as in `t!("spending-total", count = 3,
//! amount = total)`.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use std::sync::{Arc, LazyLock, RwLock};

/// Looks up a message in the interface's language, filling in any values.
macro_rules! t {
    ($id:literal) => {
        $crate::i18n::message($id, None)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::message($id, Some(&args))
    }};
}

pub struct Language {
    /// A BCP 47 language tag, such as "de".
    pub tag: &'static str,
    /// The language's name in itself, such as "Deutsch", as listed on the
    /// Settings tab.
    pub name: &'static str,
    source: &'static str,
}

/// Every language the interface can be shown in, English first.
pub const LANGUAGES: &[Language] = &[
    Language { tag: "en", name: "English", source: include_str!("../assets/i18n/en.ftl") },
    Language { tag: "de", name: "Deutsch", source: include_str!("../assets/i18n/de.ftl") },
];

const ENGLISH: &Language = &LANGUAGES[0];

type Bundle = FluentBundle<FluentResource>;

fn bundle(language: &Language) -> Bundle {
    let id = language.tag.parse().expect("the language tags are valid");
    let mut bundle = Bundle::new_concurrent(vec![id]);
    // Fluent marks each value with Unicode isolation characters for mixing
    // left-to-right and right-to-left text. Every language here is
    // left-to-right, and some screen readers would read the marks out.
    bundle.set_use_isolating(false);
    // The tests check that each file reads without errors, so whatever a
    // broken file still has is used rather than nothing.
    let resource = FluentResource::try_new(language.source.to_owned()).unwrap_or_else(|(resource, errors)| {
        log::warn!("{} translation has {} errors", language.tag, errors.len());
        resource
    });
    if let Err(errors) = bundle.add_resource(resource) {
        log::warn!("{} translation has {} repeated messages", language.tag, errors.len());
    }
    bundle
}

static ENGLISH_BUNDLE: LazyLock<Bundle> = LazyLock::new(|| bundle(ENGLISH));

/// The chosen language's messages, or `None` for English.
static CURRENT: RwLock<Option<Arc<Bundle>>> = RwLock::new(None);

#[cfg(test)]
thread_local! {
    /// A language for this thread only, set by `with_language`.
    static OVERRIDE: std::cell::RefCell<Option<Arc<Bundle>>> = const { std::cell::RefCell::new(None) };
}

/// The listed language with this tag.
pub fn find(tag: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.tag.eq_ignore_ascii_case(tag))
}

/// The translated language for a locale tag such as "de-AT" or
/// "de_DE.UTF-8", if there is one.
fn translated(tag: &str) -> Option<&'static Language> {
    find(tag.split(['-', '_', '.', '@']).next().unwrap_or_default())
}

/// The first of the system's preferred languages, most preferred first,
/// that has a translation, otherwise English.
pub fn for_tags<'a>(tags: impl IntoIterator<Item = &'a str>) -> &'static Language {
    tags.into_iter().find_map(translated).unwrap_or(ENGLISH)
}

/// The system's preferred languages, most preferred first, such as
/// "en-GB".
pub fn system_tags() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// The system's language for the interface.
pub fn system() -> &'static Language {
    for_tags(system_tags().iter().map(String::as_str))
}

/// The language chosen on the Settings tab, or the system's.
pub fn resolve(chosen: Option<&str>) -> &'static Language {
    chosen.and_then(find).unwrap_or_else(system)
}

/// Shows the interface in `language` from now on.
pub fn set(language: &Language) {
    let bundle = (language.tag != ENGLISH.tag).then(|| Arc::new(bundle(language)));
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = bundle;
}

/// Runs `f` with this thread's messages in `language`, so a test can check
/// a translation without changing the language for the tests running
/// alongside it.
#[cfg(test)]
pub fn with_language<T>(tag: &str, f: impl FnOnce() -> T) -> T {
    let language = find(tag).expect("a listed language");
    let previous = OVERRIDE.with(|o| o.replace(Some(Arc::new(bundle(language)))));
    let result = f();
    OVERRIDE.with(|o| o.replace(previous));
    result
}

/// A message in the interface's language, or in English if the translation
/// doesn't have it. Use `t!` rather than calling this, unless the id is
/// made up as the app runs.
pub fn message(id: &str, args: Option<&FluentArgs>) -> String {
    #[cfg(test)]
    let chosen = OVERRIDE.with(|o| o.borrow().clone());
    #[cfg(not(test))]
    let chosen: Option<Arc<Bundle>> = None;
    let chosen = chosen.or_else(|| CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone());
    if let Some(text) = chosen.and_then(|bundle| format(&bundle, id, args)) {
        return text;
    }
    format(&ENGLISH_BUNDLE, id, args).unwrap_or_else(|| {
        log::warn!("no message {id}");
        id.to_owned()
    })
}

fn format(bundle: &Bundle, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors).into_owned();
    // Errors name the message and the missing value, never what it holds.
    if !errors.is_empty() {
        log::warn!("message {id}: {errors:?}");
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast;
    use std::collections::{BTreeMap, BTreeSet};

    /// Each message's id, and the variables it uses.
    fn messages(language: &Language) -> BTreeMap<String, BTreeSet<String>> {
        let resource = fluent_syntax::parser::parse(language.source)
            .unwrap_or_else(|(_, errors)| panic!("{} has errors: {errors:?}", language.tag));
        let mut out = BTreeMap::new();
        for entry in resource.body {
            if let ast::Entry::Message(m) = entry {
                let mut variables = BTreeSet::new();
                if let Some(pattern) = &m.value {
                    pattern_variables(pattern, &mut variables);
                }
                assert!(out.insert(m.id.name.to_owned(), variables).is_none(), "{} repeats {}", language.tag, m.id.name);
            }
        }
        out
    }

    fn pattern_variables(pattern: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
        for element in &pattern.elements {
            if let ast::PatternElement::Placeable { expression } = element {
                expression_variables(expression, out);
            }
        }
    }

    fn expression_variables(expression: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
        match expression {
            ast::Expression::Select { selector, variants } => {
                inline_variables(selector, out);
                for variant in variants {
                    pattern_variables(&variant.value, out);
                }
            }
            ast::Expression::Inline(inline) => inline_variables(inline, out),
        }
    }

    fn inline_variables(inline: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
        match inline {
            ast::InlineExpression::VariableReference { id } => {
                out.insert(id.name.to_owned());
            }
            ast::InlineExpression::Placeable { expression } => expression_variables(expression, out),
            _ => {}
        }
    }

    #[test]
    fn translations_match_english() {
        let english = messages(ENGLISH);
        for language in &LANGUAGES[1..] {
            let translated = messages(language);
            for (id, variables) in &translated {
                let Some(expected) = english.get(id) else { panic!("{} has {id}, which English doesn't", language.tag) };
                // A translation can leave a value out, such as a count
                // that a plural already makes clear, but not add one.
                assert!(variables.is_subset(expected), "{} {id} uses {variables:?}, English {expected:?}", language.tag);
            }
            let missing: Vec<&String> = english.keys().filter(|id| !translated.contains_key(*id)).collect();
            assert!(missing.is_empty(), "{} is missing {missing:?}", language.tag);
        }
    }

    /// Every `t!("id"` in the source has an English message.
    #[test]
    fn every_message_used_exists() {
        let english = messages(ENGLISH);
        let mut files = vec![std::path::PathBuf::from("src")];
        let mut checked = 0;
        while let Some(path) = files.pop() {
            if path.is_dir() {
                files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for (at, _) in source.match_indices("t!(\"") {
                // Not the end of another macro's name, such as format!(".
                if source[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = &source[at + 4..];
                let id = &rest[..rest.find('"').unwrap()];
                if id != "id" {
                    assert!(english.contains_key(id), "{} uses {id}, which en.ftl doesn't have", path.display());
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "only found {checked} messages");
    }

    /// Messages whose ids are made up as the app runs.
    #[test]
    fn made_up_ids_exist() {
        let english = messages(ENGLISH);
        let mut ids: Vec<String> = Vec::new();
        ids.extend((1..=12).flat_map(|m| [format!("month-{m}"), format!("month-short-{m}")]));
        ids.extend((1..=7).map(|d| format!("weekday-short-{d}")));
        ids.extend(crate::locale::REGIONS.iter().map(|r| format!("region-{}", r.tag)));
        ids.extend(crate::db::STARTING_CATEGORIES.iter().map(|id| id.to_string()));
        for id in ids {
            assert!(english.contains_key(&id), "en.ftl doesn't have {id}");
        }
    }

    /// The starting categories fit the database's limit in every language.
    #[test]
    fn starting_categories_fit() {
        for language in LANGUAGES {
            with_language(language.tag, || {
                for id in crate::db::STARTING_CATEGORIES {
                    let name = message(id, None);
                    assert!(crate::db::check_category_name(&name).is_ok(), "{} {id}: {name}", language.tag);
                }
            });
        }
    }

    #[test]
    fn languages_from_system_tags() {
        assert_eq!(for_tags(["de-AT"]).tag, "de");
        assert_eq!(for_tags(["de_DE.UTF-8"]).tag, "de");
        assert_eq!(for_tags(["en-GB", "de-DE"]).tag, "en");
        assert_eq!(for_tags(["gsw-CH", "de-CH"]).tag, "de", "the first preferred language with a translation");
        assert_eq!(for_tags(["ja-JP"]).tag, "en", "English when there is no translation");
        assert_eq!(for_tags([]).tag, "en");
    }

    #[test]
    fn messages_fill_in_values_and_plurals() {
        assert_eq!(t!("categories-entry-count", count = 0), "No entries are in this category.");
        assert_eq!(t!("categories-entry-count", count = 1), "1 entry is in this category.");
        assert_eq!(t!("categories-entry-count", count = 12345), "12345 entries are in this category.");
        with_language("de", || {
            assert_eq!(t!("categories-entry-count", count = 1), "Diese Kategorie enthält 1 Eintrag.");
            assert_eq!(t!("categories-entry-count", count = 12), "Diese Kategorie enthält 12 Einträge.");
        });
        let other = t!("categories-entry-count", count = 2);
        assert_eq!(other, "2 entries are in this category.", "other threads' language is unchanged");
        assert_eq!(message("no-such-message", None), "no-such-message");
    }
}
