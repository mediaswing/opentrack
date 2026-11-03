# Changelog

All notable changes to Open Budget Tracker are recorded here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and each
release's section becomes its GitHub release notes.

## [Unreleased]

### Added

- People sharing a MariaDB/MySQL database each see only their own
  transactions. Each person is identified by their Windows SID, or their login
  name on macOS and Linux, shown under Settings. Teams, set up by the database's
  administrator, let members share chosen transactions with each other with the
  new **Share with** choice; only a transaction's owner can change or delete
  it. Categories are still shared by everyone. Transactions recorded before
  this have no owner and stay visible to everyone. This keeps people apart in
  the app but is not a security boundary; see the README.

### Changed

- On a shared MariaDB/MySQL database, backups hold only your own transactions,
  and restoring replaces only those, leaving everyone else's data alone.

- The Dashboard chart leaves out months and categories with no spending in
  the year, so only bars with something to show are drawn.

## [1.1.0] - 2026-11-03

### Added

- Import transactions from a CSV file, such as a bank statement, with
  **Import CSV…** on the Transactions tab or by dropping the file on the
  window. The columns are guessed and can be changed, a preview shows what
  will be added, unreadable lines are listed by line number, and transactions
  already recorded are left out.
- Back up every category and transaction to a JSON file, and restore one,
  under Settings → Backup. A backup restores into either database, so it also
  moves data between SQLite and MariaDB/MySQL. Restoring asks first and saves
  a copy of the current data before replacing it.
- A choice of appearance under Settings → Appearance: Standard (light colours,
  Ubuntu Medium) or High contrast (white and yellow on black, Ubuntu Bold).
  Text was Ubuntu Light before, and the colours followed the system's light
  or dark mode.

### Fixed

- The Windows installer (`.msi`) is about a quarter of its old size. It no
  longer stores a second, uncompressed copy of the program to use as the
  Start menu shortcut's icon; the shortcut now shows the installed program's
  own icon.

## [1.0.0] - 2026-11-02

### Added

- Desktop budget tracker with tabs for Dashboard, Transactions, Categories,
  Reports and Settings.
- Dashboard chart of this year's spending, grouped by month or by category.
- Storage in a built-in SQLite database, or in MariaDB/MySQL.
- Dates and amounts formatted for the system locale, with optional overrides.
- Debug log in `output.log`.
- Checks GitHub for a newer release at startup and shows a link to it in the
  status bar. It can be turned off, or run on demand, under Settings → Updates.
- Ad hoc signed macOS installer (`.pkg`) and Debian package (`.deb`), built by
  the app itself with `--package-macos` and `--package-deb`.
- Keyboard shortcuts: Ctrl+1 to Ctrl+5 switch tabs, Ctrl+N starts a new
  transaction, Ctrl+F searches, Ctrl+S adds or saves (and saves settings on the
  Settings tab), and Esc cancels editing. On macOS they use Cmd. They are listed
  under Settings.

### Changed

- Unknown regions now show the generic currency sign `¤` instead of `£`, and
  more regions have their own currency (including Taiwan, Argentina, the
  Philippines and Thailand).
- Category colours depend only on the category, so deleting one no longer
  changes the others' colours.
- Test connection only checks the database. It no longer creates a SQLite
  file, tables or default categories.
- MariaDB/MySQL tables are created with the `utf8mb4` character set.

### Fixed

- Amounts that aren't well-formed numbers are rejected instead of being
  misread: `0.125` was saved as 125.00, `1e3` as 13.00 and `1.2.3` as 12.30.
  A Unicode minus sign (`−`) now makes the amount negative instead of being
  ignored.
- Connecting to MariaDB/MySQL opens one connection instead of ten, and a
  server that stops responding can no longer freeze the window indefinitely.
- Deleting every category no longer brings back the defaults on the next start.
- Category names are unique ignoring case on SQLite too, as on MariaDB/MySQL.
- Reports: "Highest month" picks the earliest month on a tie and is hidden
  when no month has spending, instead of showing December.
- `--version` and `--help` print in a Windows console.
- An unreadable `config.json` is copied to `config.json.invalid` and reported,
  instead of being overwritten by the next save. Settings are saved in one step.
- The README's instructions for checking the .deb's signature use the release
  downloads' file names.
