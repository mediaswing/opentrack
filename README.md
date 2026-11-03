# Open Budget Tracker

A desktop budget tracker written in Rust. Record your spending, group it into
categories, and see where your money goes month by month.

The window has tabs down the left and the selected pane on the right:

| Tab | What it does |
| --- | --- |
| **Dashboard** | A chart of this year's total spend, grouped by month or by category (chosen from a dropdown); months and categories with no spending are hidden |
| **Transactions** | Add, edit and delete spending; filter by month, category or text; import a CSV file |
| **Categories** | Add, rename and delete categories, with counts and totals for each |
| **Reports** | Stacked monthly bar chart for a year, plus a category breakdown |
| **Settings** | Choose the database (SQLite or MariaDB/MySQL), appearance, locale and currency, update checks, backup and restore, file locations |

## Importing from CSV

**Import CSV…** on the Transactions tab adds transactions from a CSV file, such
as a bank statement export. You can also drop the file on the window. The app
guesses which column is which, and you can change its guesses before importing:

- **Date**: `2026-11-02`, `02/11/2026`, `2.11.26`, `2 Nov 2026` and similar.
  Day-first or month-first is worked out from the file where it can be, and
  otherwise follows your locale. A time after the date is ignored.
- **Description** and **Amount** are required. Amounts are read as in the
  Amount field. Most bank statements show spending as negative, so the app
  offers to flip the signs.
- **Category** is optional. Categories that don't exist yet can be added, and
  rows without one go into a category you choose.

Cells may be separated by commas, semicolons, tabs or `|`. Files that aren't
UTF-8 are read as Windows-1252. Rows with the same date, description and amount
as a transaction already recorded are left out, so importing the same statement
twice doesn't double it. The import is all or nothing: if any row fails,
nothing is added.

## Appearance

**Settings → Appearance** offers two looks, and the choice takes effect and is
saved straight away:

- **Standard**: light colours, with text in Ubuntu Medium.
- **High contrast**: white text and outlines on black, with yellow for what is
  selected or under the pointer, and text in Ubuntu Bold. Its text colours
  meet WCAG AAA contrast (at least 7:1).

Neither follows the system's light or dark mode.

## Backup and restore

**Settings → Backup** saves every category and transaction to one JSON file,
and restores from one. A backup restores into SQLite or MariaDB/MySQL, so it is
also how to move your data from one to the other. Amounts are stored as
decimal text and ids are kept, so a restore gives back exactly what was saved.
You can also drop a backup (`.json`) on the window to restore it.

A restore replaces everything in the current database, after asking. Before it
does, the app saves a copy of the current data in the `backups` folder inside
the data folder (`before-restore-<date>-<time>.json`). A backup that is
damaged, or that was made by a newer version of the app, is refused before
anything is changed. The restore itself is all or nothing.

## Building and running

You need a recent stable Rust toolchain (edition 2024).

```sh
cargo run --release
```

The binary is called `opentrack` (`target/release/opentrack`).

## Packaging

The app packages itself, so there are no shell scripts. Build a release
binary, then run it with a packaging option on the platform you are packaging
for. It packages its own executable:

```sh
cargo build --release
./target/release/opentrack --package-macos dist   # macOS: dist/opentrack-<version>-macos-<arch>.pkg
./target/release/opentrack --package-deb dist     # Linux: dist/opentrack_<version>_<arch>.deb
./target/release/opentrack --package-msi dist     # Windows: dist/opentrack-<version>-windows-<arch>.msi
```

The code is in `src/package/`, in `macos.rs`, `linux.rs` and `windows.rs`.
Each is only compiled on its own platform.

**macOS.** The binary is put into `Open Budget Tracker.app`, which is ad hoc
signed (`codesign --sign -`) with the hardened runtime. It is then wrapped in a
`.pkg` that installs it into `/Applications`. The installer itself is unsigned,
because a `.pkg` can only be signed with an Apple "Developer ID Installer"
certificate. An ad hoc signature lets Apple Silicon run the app, but Gatekeeper
does not trust it. People installing a downloaded copy have to approve it in
**System Settings → Privacy & Security**.

**Linux.** The `.deb` is assembled in Rust (ar, tar and xz) and is not signed.
Release downloads use hyphens in their names (`opentrack-<version>-amd64.deb`).
A package you build yourself keeps Debian's underscores.

**Windows.** The app writes the WiX source for an `.msi` and builds it with the
[WiX Toolset](https://wixtoolset.org/), which you need to install first. It
needs the .NET SDK:

```sh
dotnet tool install --global wix --version 6.0.2
```

If `wix` isn't on the `PATH`, set `OPENTRACK_WIX` to the full path of
`wix.exe`. The `.msi` installs `opentrack.exe` for all users into
`C:\Program Files\Open Budget Tracker` and adds a Start menu shortcut. It
replaces an older version that is already installed. Uninstalling it, from
**Settings → Apps**, leaves your data folder in place (see [Files](#files)). Neither
the program nor the installer is signed, so Windows SmartScreen warns about a
downloaded copy: choose **More info → Run anyway**.

### Releases

`.github/workflows/ci.yml` builds and tests the app on Windows, Ubuntu and
macOS on every push to `main` and every pull request. On each system that
includes building and checking a package.

Pushing a version tag such as `v1.0.0` runs `.github/workflows/release.yml`.
It checks that the tag matches the version in `Cargo.toml`. It then builds the
`.deb` on Ubuntu 22.04, the `.pkg` on macOS (Apple Silicon) and the `.msi` on
Windows (x64), and tests them: dpkg-deb checks the `.deb`, the `.pkg` is
installed, and the `.msi` is installed and uninstalled. Finally it publishes
them on a GitHub release. The release
notes come from that version's section of `CHANGELOG.md`. If the repository has
a `VT_API_KEY` secret, the downloads are also scanned with VirusTotal.

## Locale

Dates and amounts follow the system locale, which is detected at startup. For
example, `en-GB` shows `£1,234.50` and `02/11/2026`, `en-US` shows `$1,234.50`
and `11/02/2026`, and `de-DE` shows `1.234,50 €` and `02.11.2026`. The detected
locale is written to `output.log`.

Under **Settings → Region** you can set a different locale tag (such as `fr-FR`)
or replace the currency symbol. Leave both blank to follow the system.

If the app doesn't know a region's currency, it shows the generic sign `¤`.
Set the symbol under **Settings → Region** in that case.

Amounts can be typed with either `.` or `,` as the decimal separator. The last
`.` or `,` counts as the decimal point when one or two digits follow it; with
three digits after it, it is a thousands separator (`1,234` is 1234.00).
Thousands can also be separated with spaces or apostrophes (`1 234,50`,
`1'234.50`), and a currency symbol and minus sign may be typed around the
number. Anything else, such as `0.125` (three decimal places) or `1.2.3`, is
rejected rather than guessed at.

The locale only changes formatting. Amounts are not converted between currencies.

## Update checks

At startup the app asks GitHub for the latest release
(`api.github.com/repos/mediaswing/opentrack/releases/latest`). If it is newer
than the running version, the status bar shows a link to its release page.
Nothing is downloaded or installed; you install the new version yourself.
Drafts and pre-releases are ignored.

Under **Settings → Updates** you can turn the startup check off (then press
**Save & connect**) or press **Check now**. The result is also written to
`output.log`.

## Storage

By default the app keeps its own SQLite database. Nothing needs to be installed;
SQLite is compiled in.

To use MariaDB or MySQL instead, open **Settings**, choose *MariaDB / MySQL*,
fill in host, port, database, user and password, then press **Save & connect**.
The database itself must already exist. The app creates the tables and default
categories the first time it connects. **Test connection** only looks: it
doesn't create a database file, tables or categories.

The default categories are only added to a new database. If you delete them
all, they stay deleted. Category names are unique ignoring case (`Pets` and
`pets` can't both exist), with either backend.

Tables are created with the `utf8mb4` character set, so any text can be
stored. Tables created by version 1.0.0 use the server's default, which on some
servers can't hold characters such as emoji (the app logs a warning). To
convert them:

```sql
ALTER TABLE category CONVERT TO CHARACTER SET utf8mb4;
ALTER TABLE spending CONVERT TO CHARACTER SET utf8mb4;
```

Both backends use the same schema:

- `schema/mariadb.sql`: the MariaDB/MySQL schema
- `schema/sqlite.sql`: the SQLite equivalent, with the same category IDs (starting at 22)

Amounts are `DECIMAL(8,2)`, so the largest single amount is 999,999.99. Negative
amounts are allowed, for refunds.

Switching backends does not copy data between them.

## Files

The config file, the default SQLite database and the debug log all live in one
data folder:

| OS | Folder |
| --- | --- |
| macOS | `~/Library/Application Support/opentrack/` |
| Linux | `~/.local/share/opentrack/` |
| Windows | `%APPDATA%\opentrack\data\` |

Set the `OPENTRACK_HOME` environment variable to use a different folder.

- `config.json`: settings. A MariaDB/MySQL password is stored here **in plain text**.
  If the file can't be read, the app starts with default settings, says so in
  the status bar, and first copies the file to `config.json.invalid` so saving
  doesn't lose it.
- `opentrack.db`: the default SQLite database.
- `output.log`: debug information. It is recreated on each launch. The app logs
  at debug level; other libraries log only warnings and errors.

The **Settings** tab shows the exact paths and has buttons to copy them.

## Development

```sh
cargo test     # money parsing/formatting, schema splitting, SQLite round trip
cargo clippy
```

## Security

To report a vulnerability, please use GitHub's private reporting rather than a
public issue. See [SECURITY.md](SECURITY.md) for details.

## License

Open Budget Tracker is released under the [MIT License](LICENSE).

The app's text is set in Ubuntu Medium and Ubuntu Bold (`assets/fonts/`),
which are built into the program. They are copyright Canonical Ltd. and used
under the [Ubuntu Font Licence 1.0](assets/fonts/UFL.txt).
