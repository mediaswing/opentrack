# Open Budget Tracking

Open Budget Tracking is a simple budget tracker, written in Rust. You tell it
what you spent, and it tells you where your money went, month by month.
Everything it says is written to be read aloud: it works well with a screen
reader (VoiceOver on macOS; NVDA, JAWS and Narrator on Windows) and from the
keyboard alone.

The program is called `opentrack`: that is what to type to start it from a
terminal, and the name of the file that is installed.

## What it does

- **Records spending**: what you bought, how much, the date and a category.
- **Lists a month's spending**, with the total first.
- **Summarises a month by category**, largest first, with each category's
  share of the total.
- **Starts with eight categories**, such as Food and Drink and Transport, and
  you can add your own.
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

Open Budget Tracking runs in a text window. On Windows, the Start menu shortcut
opens it in a console window. On a Mac, opening it from Applications opens it in
a new Terminal window.

## Using it

The main menu has these options. Type the number, or the first word, and press
Enter:

1. **Add** spending.
2. **List** spending for a month.
3. **Summary** of a month by category.
4. **Delete** a spending entry.
5. **Categories**: hear the list of categories.
6. **New** category.
7. **Help**: hear these options again.
0. **Quit**.

Every question says what to type. Pressing Enter on an empty line cancels what
you were doing, and nothing is changed.

Each line the app writes is a complete sentence, with no tables or columns. Lists
say how many items they hold before reading them, and end with "End of list".
Dates are written out in words, and an amount below zero is read as "minus"
rather than with a hyphen, which some screen readers skip.

### Dates and amounts

When it asks for a date, press Enter for today, type `yesterday`, or type a date
in your usual order, such as `18/11/2026` in the UK or `11/18/2026` in the US.
`2026-11-18` always works. Leave out the year for this year.

When it asks for a month, press Enter for this month, type `last`, or type a
month such as `11/2026` or `November`.

Amounts can be typed with or without the currency symbol and thousands
separators. Both `12.50` and `12,50` work where they can't be mistaken for
thousands.

## Settings

Open Budget Tracking works these out for itself, and these options change them:

| Option | Environment variable | What it does |
| --- | --- | --- |
| `--db PATH` | `OPENTRACK_DB` | Use a different database file. |
| `--locale NAME` | `OPENTRACK_LOCALE` | Format dates and money for another locale, such as `en_GB` or `de_DE`. |
| `--log PATH` | | Write the debug log somewhere else. |
| | `OPENTRACK_LOG` | How much the debug log records: `off`, `error`, `warn`, `info`, `debug` (the default) or `trace`. |

`opentrack --help` lists the options and says which locale, database and log
file are in use. `opentrack --version` prints the version.

### Where your data is kept

| System | Folder |
| --- | --- |
| macOS | `~/Library/Application Support/opentrack` |
| Windows | `%APPDATA%\opentrack` |

The folder holds the database, `opentrack.db`, and the debug log, `debug.log`.
The log records what the app does, such as which locale it chose and any errors,
but not what you bought or how much you spent. When it grows past 1 MB, it is
renamed `debug.log.old` and a new one is started.

### Locale

The locale is taken from the first of these that is set: `--locale`,
`OPENTRACK_LOCALE`, `LC_ALL`, `LC_MONETARY`, `LC_TIME`, `LANG`, and then the
operating system's own setting. If none can be used, `en_US` is used.

The locale changes how amounts are shown, not their value: switching from
`en_GB` to `de_DE` shows £10 as 10 €. The app's own words are in English.

## Building from source

You need Rust 1.89 or later.

```sh
cargo build --release
target/release/opentrack
```

`cargo test` runs the tests. On a Mac, they also build and check an installer
package.

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

MIT. See [LICENSE](LICENSE).
