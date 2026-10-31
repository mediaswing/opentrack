# Changelog

## [Unreleased]

### Added

- The budget can be kept on a MySQL or MariaDB server instead of the
  built-in database, chosen under Database on the Settings tab. A server
  with no entries is offered a copy of the budget in use.

### Changed

- CSV exports put a `'` before names and categories that a spreadsheet
  would otherwise run as a formula (starting with `=`, `+`, `-` or `@`).
  Importing takes it off again.
- A two-digit year, as in 5/3/99, is the nearest such year no more than 20
  years ahead, so 99 is 1999 rather than 2099.
- The average a month on the Summary tab starts from the first month with
  any spending.
- The list of entries stays quick with thousands of entries.

### Fixed

- Exporting over the database the app is using is refused, as copying it
  already was.
- Deleting a category while changing one of its entries no longer moves
  the entry to the first category when it is saved.
- The SQL export loads the same on servers set to `NO_BACKSLASH_ESCAPES`,
  which used to change backslashes and line breaks in names.
- A settings file that can't be read is kept as
  `settings.json.unreadable`, and the app says so, rather than being
  replaced without a word.
- Amounts with leading zeros, such as 007.50, are accepted.
- Two copies of the app open on the same built-in database wait for each
  other's changes rather than failing.

## [1.2.0] - 2026-10-31

The first release from this repository. Open Budget Tracker: an accessible,
keyboard-first budgeting tool.
