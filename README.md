# Open Budget Tracking

Open Budget Tracking is a simple budget tracker, written in Rust. You tell it
what you spent, and it tells you where your money went, month by month.
Accessibility is its guiding principle, as in
[Common Speech Output](https://github.com/mediaswing/speechout), whose layout it
shares: everything can be done from the keyboard, and every control can be used
with a screen reader (VoiceOver on macOS; NVDA, JAWS and Narrator on Windows).

The program is called `opentrack`: that is what to type to start it from a
terminal, and the name of the file that is installed.

## What it does

- **Records spending**: what you bought, how much, the date and a category.
- **Lists a month's spending**, and deletes entries you no longer want.
- **Summarises a month by category**, largest first, with each category's
  share of the total.
- **Starts with eight categories**, such as Food and Drink and Transport, and
  you can add your own.
- **Imports and exports spending** as CSV, for spreadsheets, or JSON, and makes
  **reports** in PDF and Word format that work with screen readers.
- **Shows dates and money the way your computer is set up**, such as
  `£1,234.56` and `18/11/2026` in the UK, or `1.234,56 €` and `18.11.2026` in
  Germany.
- **Keeps everything on your computer** in a SQLite database.

## Installing

Downloads are on the [releases page](https://github.com/mediaswing/opentrack/releases).

| System | Download |
| --- | --- |
| Windows on x64 PCs | `opentrack-<version>-windows-x86_64.msi` |
| macOS on Apple Silicon | `opentrack-<version>-macos-aarch64.pkg` |

The downloads aren't signed with a paid certificate, so your computer may warn
you. On Windows, choose **More info**, then **Run anyway**, then follow the
installer's steps; its last page has a box to start the app. On a Mac the app
has an ad hoc signature: if macOS refuses the package, open **System Settings**,
then **Privacy & Security**, and choose **Open Anyway**.

## Using it

The window has five tabs down the left side, with the selected tab's controls to
the right: Summary, Spending, Categories, Import/Export and System Settings. It
opens on Spending. Each control has a line to itself, apart from the three buttons
that change the month, which share one, and **Save spending**, which is beside
**Category** when there is room. The status bar at the bottom says what happened. Screen readers announce its messages without the focus moving, and a
mistake interrupts whatever is being said. Press F2 to hear the last message again.

### Summary

A month's spending added up by category, largest first, with each category's
share of the total. **Previous month**, **This month** and **Next month** change
the month, and changing it here changes it on the Spending tab too.

### Spending

Adding spending, then the month's entries.

Type **What you bought** and the **Amount**, then choose a **Category** and press
**Save spending**, or Enter in any of the boxes, or Ctrl+S. The status bar reads
back what was saved, and the focus goes back to **What you bought**, ready for the
next one. If something needs changing, the status bar says what, and the focus
goes to that box.

Leave **Date** empty for today, type `yesterday`, or type a date in your usual
order, such as `18/11/2026` in the UK or `11/18/2026` in the US. `2026-11-18`
always works, and leaving out the year means this year. Amounts can be typed with
or without the currency symbol and thousands separators. Both `12.50` and `12,50`
work where they can't be mistaken for thousands.

Below that is every entry in the month, one on each line, in a box that a screen
reader can go through line by line. **Previous month**, **This month** and **Next
month** change the month, and the status bar says how many entries it has and
what they come to. To delete an entry, choose it in **Entry to delete** and press
**Delete this entry**. The app asks before deleting anything.

### Categories

The categories there are, and a box to add a new one, of up to 32 characters.
New categories can be chosen on the Spending tab straight away.

### Import/Export

- **Import from a file** adds the spending in a CSV or JSON file to what is
  already there. The whole file is checked first: if anything in it can't be
  read, nothing is imported, and the status bar says which line has the problem.
  Otherwise the app says how many entries the file has, their dates and total,
  and any new categories, and asks before saving them.
- **Export to a file** saves **All spending**, or the month on show on the
  Summary and Spending tabs, in the **Format** chosen:
  - **CSV**, for spreadsheets, and **JSON**, which can both be imported again.
    JSON also keeps categories with no spending in them.
  - **PDF** and **Word** (.docx) reports, with the total, a table of spending
    by category (and by month, for all spending), and a table of every entry.
    Both have real headings, and tables with header rows, so screen readers can
    move around them, and both say which language they are in.

A CSV file's first line names its columns: `Date`, `Name`, `Amount` and
`Category`, in any order, and other columns are ignored. `Description`, `Item`
or `Payee` will do for `Name`, and `Price` or `Cost` for `Amount`. Columns can be
separated by commas, semicolons or tabs. Dates are best written `2026-11-18`, and
amounts as plain numbers such as `12.50` or `-3.99`, which is how the app writes
them, but dates in your usual order and amounts with currency symbols work too.
Text that a spreadsheet would run as a formula, such as `=SUM(A1:A9)`, is written
after an apostrophe, which spreadsheets hide.

A JSON file is either a list of entries, or an object with a `spending` list and,
if you like, a `categories` list:

```json
{
  "categories": ["Food and Drink", "Gifts"],
  "spending": [
    { "date": "2026-11-18", "name": "Tea", "amount": "2.50", "category": "Food and Drink" }
  ]
}
```

### System Settings

- **Colours and text**: **Standard** has light colours and medium-weight text.
  **High Contrast** has white and yellow on black, with bold text.
- **Locale**: dates and money follow your computer's settings. To use another
  country's formats, type a locale such as `en_GB` or `de_DE` and press **Use this
  locale**. Leave the box empty to follow your computer again.
- **Updates**: which version this is, and **Check for updates now**. When a newer
  version is out, a link to download it appears here. **Check for updates when
  the app starts** can be turned off.
- Where your spending and the debug log are kept.

Settings are saved as soon as they are changed.

## Keyboard

Press F1 at any time to see this list in the app. "Ctrl" is Cmd (⌘) on a Mac, where
F1 and F2 may need you to hold Fn as well, and Page Up and Page Down are Fn+Up and
Fn+Down.

| Keys | Action |
| --- | --- |
| Tab, Shift+Tab | Move between controls |
| Space, Enter | Press the control with the focus |
| Ctrl+1, Ctrl+2, Ctrl+3, Ctrl+4, Ctrl+5 | Go to Summary, Spending, Categories, Import/Export or System Settings |
| Ctrl+N | Add spending: go to the first box on the Spending tab |
| Ctrl+S | Save the spending; on the Categories tab, add the category; on the System Settings tab, use the locale |
| Ctrl+Page Up, Ctrl+Page Down | Show the previous or next month, on the Summary and Spending tabs |
| Escape | Close a dropdown list or the keyboard shortcuts |
| F1 | Show or hide the keyboard shortcuts |
| F2 | Say the status message again |
| Up, Down, Home, End, or a letter | Change the choice in a dropdown |
| Alt+Down (Option+Down on a Mac) | Open a dropdown's list; Alt+Up closes it |
| Ctrl+Plus, Ctrl+Minus, Ctrl+0 | Make everything larger or smaller, or reset it |

## Where your data goes

Everything is kept in one SQLite database, with the debug log beside it:

| System | Folder |
| --- | --- |
| macOS | `~/Library/Application Support/opentrack` |
| Windows | `%APPDATA%\opentrack` |

The database is `opentrack.db`, and the log is `debug.log`. Your spending never
leaves your computer, and the app doesn't send any statistics.

| What you do | What leaves your computer |
| --- | --- |
| Start the app, unless update checks are turned off, or check for updates | A request to GitHub for the latest version number |
| Import or export | Nothing: only the file you choose is read or written |
| Anything else | Nothing |

When a newer version is found as the app starts, it asks whether to open the
download page in your web browser. The app never downloads or installs anything
by itself.

The log records what the app does, such as which locale it chose and any errors,
but not what you bought or how much you spent. When it grows past 1 MB, it is
renamed `debug.log.old` and a new one is started. Set `OPENTRACK_LOG` to `off`,
`error`, `warn`, `info`, `debug` (the default) or `trace` to change how much it
records.

### Starting it differently

These change where things are kept, and the locale, for one run:

| Option | Environment variable | What it does |
| --- | --- | --- |
| `--db PATH` | `OPENTRACK_DB` | Use a different database file. |
| `--locale NAME` | | Show dates and money for another locale, such as `en_GB`. |
| `--log PATH` | | Write the debug log somewhere else. |

`opentrack --help` lists them, and `opentrack --version` prints the version.

The locale is the first of these that is set: `--locale`, the one chosen on the
System Settings tab, `OPENTRACK_LOCALE`, `LC_ALL`, `LC_MONETARY`, `LC_TIME`,
`LANG`, and then the operating system's own setting. If none can be used, `en_US`
is used.
The locale changes how amounts are shown, not their value: switching from `en_GB`
to `de_DE` shows £10 as 10 €. The app's own words are in English.

## Building

You need Rust 1.95 or later.

```sh
cargo build --release
target/release/opentrack
```

`cargo test` runs the tests. Those in `src/ui/tests.rs` drive the window the way
a keyboard and screen reader user would, and check what screen readers are told.
On a Mac, the tests also build and check an installer package.

### Layout

| Path | What it is |
| --- | --- |
| `src/ui/` | The window, with one file per tab, and its accessibility tests |
| `src/package/` | Release packaging, one file per system |
| `src/db.rs` | The SQLite database |
| `src/transfer.rs` | Reading and writing CSV and JSON files |
| `src/report/` | PDF and Word reports, and the zip writer Word files need |
| `src/locale.rs` | Finding the locale, and showing and reading dates and money |
| `src/logger.rs` | The debug log |
| `src/update.rs` | Asking GitHub for the latest version number |
| `assets/schema/` | The database's tables for SQLite, as the app makes them, and for MariaDB |

## Making the installers

The app builds its own installers, which is what the release workflow
(`.github/workflows/release.yml`) does when a version tag such as `v0.1.0` is
pushed.

On a Mac:

```sh
cargo build --release
target/release/opentrack --package-macos dist
```

This makes `dist/opentrack-<version>-macos-<arch>.pkg`, which installs
`Open Budget Tracking.app` into /Applications. It uses Apple's `codesign`,
`pkgbuild` and `productbuild`, then checks that the package holds the app and
that the app's ad hoc signature is valid.

On Windows, install the [WiX Toolset](https://wixtoolset.org/) first:

```powershell
dotnet tool install --global wix --version 6.0.2
wix extension add --global WixToolset.UI.wixext/6.0.2 WixToolset.Util.wixext/6.0.2
cargo build --release
target\release\opentrack.exe --package-msi dist
```

This makes `dist\opentrack-<version>-windows-x86_64.msi`, which installs into
`C:\Program Files\Open Budget Tracking` and adds a Start menu shortcut. Any DLLs
beside `opentrack.exe` are installed with it. The release workflow puts the
Visual C++ runtime there, so the app runs on PCs without the Redistributable.

The icon is `assets/icon/opentrack.svg`. The .png, .icns and .ico files beside
it are made from it, and need making again if it changes.

## Licence

[MIT](LICENSE). The Ubuntu fonts in `assets/fonts` are under the
[Ubuntu Font Licence](assets/fonts/UFL.txt).
