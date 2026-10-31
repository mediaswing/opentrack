# Open Budget Tracker

Open Budget Tracker keeps track of what you spend, sorts it into categories,
and shows it against a monthly budget for each one. It is written in Rust,
and like the [Speech Output Engine](https://github.com/mediaswing/speechout)
it is built for the keyboard first: every feature works without a mouse, and
the whole interface is exposed to screen readers (Narrator, NVDA and JAWS on
Windows, VoiceOver on macOS, Orca on Linux).

The program file is called `opentrack`.

## What it does

- **Records spending**: what it was, the date, the amount (a minus sign for
  a refund) and its category.
- **Lists, finds and filters entries** by month or any run of dates (such as
  a financial year), by category, or by part of their name, with a running
  total.
- **Sets a monthly budget for each category**, and shows each category's
  spending as a bar that fills towards its budget and turns red when over.
- **Charts spending month by month** as columns: the last twelve months, or
  each month in the dates chosen.
- **Imports and exports** spending as CSV, everything (budgets included) as a
  JSON backup, and everything as a MariaDB SQL script (from Ctrl+E).
- **Keeps its own SQLite database**, so there is no server to install, or
  keeps the budget on a **MySQL or MariaDB server** instead, if you have one.
- **Writes a debug log** when you turn it on, to help fix problems.

## Installing on Windows

Each [release](https://github.com/mediaswing/opentrack/releases) has a
Windows Installer (.msi) for x64 PCs, such as
`opentrack-1.0.0-windows-x86_64.msi`. It installs Open Budget Tracker and
creates a Start menu shortcut. It installs for your account only, in
`%LOCALAPPDATA%\Programs\OpenBudgetTracker`, so it doesn't need
administrator rights. The installer is the standard Windows one and works
from the keyboard and with screen readers: press Enter for **Next**, then
**Install**, then **Finish**. To update, install the new version's `.msi`; it
replaces the old one. To uninstall, go to Settings › Apps › Installed apps
(or Apps & features on Windows 10), find Open Budget Tracker and choose
**Uninstall**. Updating and uninstalling never touch your budget, which is
kept separately (see [Where things are kept](#where-things-are-kept)).

To install without any questions, for example on many computers, run
`msiexec /i <the .msi file> /qn`.

## Installing on macOS

Each release has an installer package (.pkg) for Macs with Apple Silicon
(M1 or later) running macOS 11 or later, such as
`opentrack-1.0.0-macos-aarch64.pkg`. It puts **Open Budget Tracker** in your
Applications folder, so it asks for an administrator password. To update,
install the new version's `.pkg`; it replaces the old app. To uninstall, drag
the app from Applications to the Bin. Your budget is kept either way.

The package and the app aren't notarised by Apple yet, so macOS blocks the
package the first time you open it. If you downloaded it from this project's
releases page, go to System Settings › Privacy & Security and choose
**Open Anyway**, then open the package again. The app itself may need the same
the first time it starts.

## Installing on Linux

Each release has a Debian package (.deb) for x64 PCs running Ubuntu 22.04
or later (or another Debian-based system of the same age), such as
`opentrack-1.0.0-linux-x86_64.deb`. Open it with App Center or Software, or
install it from a terminal in the folder you downloaded it to:

```
sudo apt install ./opentrack-1.0.0-linux-x86_64.deb
```

It puts **Open Budget Tracker** among your applications and `opentrack` in
`/usr/bin`, and installs the libraries it needs. To update, install the new
version's `.deb`; it replaces the old one. To uninstall, run
`sudo apt remove opentrack`. Your budget is kept either way.

## The tabs

- **Spending**: the form for adding or changing an entry, then the list of
  entries for the dates shown (see below). Press Tab from the Find box to reach the list; the
  arrow keys, Page Up, Page Down, Home and End move through it. Enter or
  Space (or F2) edits the entry, and Delete or Ctrl+D deletes it.
- **Summary**: the spending for the dates shown against the budget, a bar
  for each category, and a chart of the twelve months up to the month shown,
  or of each month in the dates shown. Every bar can be
  reached with Tab, and screen readers read out its amounts.
- **Categories**: rename a category, set or clear its monthly budget, delete
  it (moving its entries to another category), or add a new one.
- **Import/Export**: the file formats, and a copy of the database itself.
- **Settings**: the language, the region, the financial year, the currency
  symbol, keyboard shortcuts, asking
  before deleting, update checks, and the debug log. When **Check for updates when
  the app starts** is ticked (it is at first), the app asks GitHub whether
  there is a newer release, and if there is, offers to open its download
  page. Nothing about you or your budget is sent. **Check for updates now**
  checks straight away.

## Choosing the dates shown

The Spending and Summary tabs show one month at first. Under **Dates to
show**, choose **From one date to another** to see any run of dates instead:
it starts as this financial year, and you can type other dates in **From**
and **To**, then press Enter or **Show these dates**. **Earlier dates** and
**Later dates** (F3 and F4) move to the run of dates before or after, a year
at a time for a financial year, and **This month** (F5) goes back to showing
one month.

Budgets are monthly, so for a run of dates each one counts in full for every
whole month and in part, by the day, for part of a month: a £30 budget is
£360 over a year.

The financial year starts on the region's usual day for personal tax: 6 April
in the United Kingdom, 1 July in Australia, 1 April in India and New Zealand,
1 March in South Africa, and 1 January elsewhere. Choose another day under
**Financial year** on the Settings tab.

## Dates and amounts

Dates and amounts are written the way your computer's region settings say:
in the United Kingdom a date is typed 30/10/2026 and an amount shown as
£1,234.56; in the United States, 10/30/2026 and $1,234.56; in Germany,
30.10.2026 and 1.234,56 €. Over 35 regions are known. If yours isn't, the
app uses your country's or language's nearest match, and leaves the currency
symbol out rather than guess it. Choose another region under **Dates and
amounts written as in** on the Settings tab, or type your own currency
symbol there.

A date written year-month-day, such as 2026-10-30, is always understood, and
so is either decimal mark when only one or two digits follow it, so 8.50 and
8,50 are the same everywhere. Files you import or export always use one
format (below), so they can be moved between computers.

## Languages

The app is in English and German. It uses your computer's language if it
has a translation for it, and English otherwise. Choose another under
**Language of the app** on the Settings tab. The language and the region are
separate, so the app can be in English while dates and amounts are written
the German way. A new budget's starting categories are named in the
language the app is in when the budget is made.

### Translating

Each language is one file in `assets/i18n`, in the
[Fluent](https://projectfluent.org) format. To add one, copy `en.ftl` to a
file named with the language's tag, such as `fr.ftl`, translate the text
after each `=`, and add the language to `LANGUAGES` in `src/i18n.rs`. The
notes at the top of `en.ftl` explain plurals and the values in braces.
`cargo test` checks that a translation has every message and uses only the
values English does. The app's fonts cover Latin, Greek and Cyrillic
scripts. It can't yet show right-to-left scripts, or scripts that need
shaping, such as Arabic or Hindi.

## Keyboard shortcuts

On a Mac, Cmd takes the place of Ctrl, except in Ctrl+Tab.

| Shortcut | What it does |
| --- | --- |
| Ctrl+N | New entry (a new category on the Categories tab) |
| Ctrl+S | Save the entry or category; Enter in a box does too |
| F2 | Edit the chosen entry |
| Ctrl+D | Delete the chosen entry or category |
| Ctrl+F | Find entries |
| F3 or Ctrl+[ | Previous month, or the dates before |
| F4 or Ctrl+] | Next month, or the dates after |
| F5 | Back to this month |
| F7 | Say the spending and budget for the dates shown |
| Ctrl+I | Import a CSV or JSON file |
| Ctrl+E | Export to CSV, JSON or SQL (chosen by the file name) |
| Ctrl+1 to Ctrl+5 | Go to a tab |
| Ctrl+Tab, Ctrl+Shift+Tab | Next and previous tab |
| F1 | List the keyboard shortcuts |
| Esc | Stop editing, clear the form, or close a list |

These always work, even with the shortcuts turned off: Tab and Shift+Tab move
between controls, Space or Enter presses buttons, the Left and Right arrows
(and Home and End) move along the tabs when one has focus, and Up and Down
change a dropdown's choice. Alt+Down opens a dropdown's list.

As in the Speech Output Engine, no shortcut is a single letter, digit or
symbol, which screen readers use for quick navigation (WCAG 2.1.4), and none
uses Alt or Option, so AltGr characters are never taken. Shortcuts can be
turned off on the Settings tab.

## Importing and exporting

**CSV** files need a first row naming the columns `name`, `date`, `price`
and `category`, in any order (`amount` and `bucket` also work). Dates are
written year-month-day, such as `2026-10-30`. Categories that don't exist
yet are created. If any row is wrong, nothing is imported, and the message
says which row.

```csv
name,date,price,category
Weekly shop,2026-10-24,64.20,Food and Drink
Train ticket,2026-10-25,12.40,Transport
```

**JSON** backups hold every category, budget and entry. When importing one,
you choose whether it replaces everything or is added to what is there.

**SQL** export writes a MariaDB (or MySQL) script that creates the
`category`, `spending` and `budget` tables and fills them. Press Ctrl+E and
choose the SQL file type.

**Save a copy of the database** on the Import/Export tab writes a complete
SQLite copy, which can be opened with any SQLite tool. When the budget is on
a database server, back it up with the server's own tools instead, or export
a JSON backup.

## Where things are kept

| What | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Database | `%LOCALAPPDATA%\OpenBudgetTracker\budget.db` | `~/Library/Application Support/OpenBudgetTracker/budget.db` | `~/.local/share/opentrack/budget.db` |
| Settings | `%APPDATA%\OpenBudgetTracker\settings.json` | `~/Library/Application Support/OpenBudgetTracker/settings.json` | `~/.config/opentrack/settings.json` |
| Debug log | `%LOCALAPPDATA%\OpenBudgetTracker\logs` | `~/Library/Application Support/OpenBudgetTracker/logs` | `~/.local/share/opentrack/logs` |

The debug log is off until you tick **Write a debug log** on the Settings
tab (or start the app with `--debug-log`), and the folder can be changed
there. Each session starts a fresh `opentrack.log`, and the one before is
kept as `opentrack.previous.log`. The log records what the app did and any
errors, never what money was spent on or how much, so it is safe to attach
to a bug report.

## The database

The tables follow the original MariaDB schema: `category` (`id`, `short`)
and `spending` (`line`, `name`, `date`, `price`, `bucket`), with category ids
starting at 22 and the same eight starting categories. For SQLite, `price`
is stored as whole pence (or cents) so that totals are exact, and dates are
ISO 8601 text. A `budget` table adds the monthly budget for each category.

### Using a MySQL or MariaDB server

The budget can be kept on a MySQL or MariaDB server instead of in the
built-in database. Under **Database** on the Settings tab, choose **On a
MySQL or MariaDB server**, type the server, port, database, user name and
password, and choose **Connect and use this server**. The database must
already exist, and the user needs to be able to create tables in it the
first time.

- The app makes the tables if they aren't there, in the original schema
  (`price` as `DECIMAL(8,2)`, `date` as `DATE`). These are the same tables
  the SQL export writes, so a database loaded from an export can be used
  straight away. The tables must use InnoDB.
- If the server's database has no entries, you're offered a copy of the
  budget you're using now. The built-in database is never changed, and
  **Use the built-in database** goes back to it.
- **Encrypt the connection (TLS)** is on at first, and checks the server's
  certificate. Many servers on a home computer or network, including recent
  MariaDB versions that make their own certificate, need it turned off.
- The password is kept in `settings.json` (see above), which only your
  account can read.
- If the server can't be reached when the app starts, it offers to use the
  built-in database for that session. `--database FILE` always uses that
  file.

## Command line

```
opentrack                  Start the application
opentrack --debug-log      Start the application, writing the debug log
opentrack --database FILE  Start the application with another database file
opentrack --version        Print the version
opentrack --help           Print this help
opentrack --package-macos DIR
                           Build an installer package (.pkg) in DIR (macOS)
opentrack --package-deb DIR
                           Build a Debian package (.deb) in DIR (Linux)
```

## Building

Install Rust (1.88 or later), then:

```
cargo build --release
```

The program is `target/release/opentrack` (`opentrack.exe` on Windows).
SQLite is built into it, so nothing else needs installing. On Linux, the
libraries for a windowed app are needed; on Debian and Ubuntu:

```
sudo apt install pkg-config libxkbcommon-dev libwayland-dev libgl1-mesa-dev   libx11-dev libxcursor-dev libxrandr-dev libxi-dev gcc
```

Run the tests with `cargo test`. They include tests that drive the whole
window from the keyboard, without a screen. To run the database tests
against a MySQL or MariaDB server too, give a user that can create and drop
databases:

```
OPENTRACK_TEST_MYSQL=mysql://user:password@127.0.0.1:3306 cargo test
```

## Releasing

Releases are built the same way as the Speech Output Engine's.

1. Add a section for the new version to [`CHANGELOG.md`](CHANGELOG.md) and
   bump `version` in `Cargo.toml` (then run `cargo build` so `Cargo.lock`
   follows).
2. Push a tag such as `v1.0.0`. The Windows installer takes its version from
   the tag, so the tag must be three numbers with no suffix such as `-beta`
   (the first two at most 255, the third at most 65535), and it must match
   `Cargo.toml`. The workflow stops with an error if it doesn't.

The [release workflow](.github/workflows/release.yml) builds a Windows `.msi`
installer for x64 (the `.exe` with the Visual C++ runtime DLLs it needs,
described by [`src/installer/opentrack.wxs`](src/installer/opentrack.wxs) and
built with WiX Toolset), an Apple Silicon `.pkg` installer (built by
`opentrack --package-macos`, in [`src/package/macos.rs`](src/package/macos.rs))
and an x64 `.deb` package for Debian and Ubuntu (built on Ubuntu 22.04 by
`opentrack --package-deb`, in [`src/package/linux.rs`](src/package/linux.rs)).
It tests installing, upgrading and uninstalling the `.msi` and the `.deb`, and
installing the `.pkg`, then publishes the downloads on a GitHub release, using the newest
version section of the changelog as the release notes. Before publishing,
each download is submitted to VirusTotal, and a table of scan links is added
to the release notes. This needs a repository secret called `VT_API_KEY`
containing a VirusTotal API key.

The [CI workflow](.github/workflows/ci.yml) runs the tests on Windows, macOS
and Linux for every push to `main` and every pull request, and builds and
checks a `.pkg` on macOS and a `.deb` on Linux.

## Licence

GPL-3.0-or-later; see `LICENSE`. Google Sans is used under the SIL Open Font
License; see `assets/fonts/OFL.txt`. The icon was drawn for this app; see
`assets/icon/CREDITS.txt`.
