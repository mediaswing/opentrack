# Changelog

## [Unreleased]

- **A new Import/Export tab.** Spending can be imported from CSV or JSON
  files, and exported as CSV, JSON, or a PDF or Word report. A file is checked
  in full before anything is saved, and the app says what it holds and asks
  first. Reports have real headings and tables with header rows, so they work
  with screen readers.
- **The tabs are now Summary, Spending, Categories, Import/Export and System
  Settings.** The Spending tab has the boxes for adding spending above the
  month's entries, so Ctrl+1 to Ctrl+5 go to different tabs than before.
  **Save spending** is beside **Category**, or below it in a narrow window.
- **The debug log no longer records what you bought or how much you spent.**
  It recorded each status bar message, which can say both.
- **Two copies of the app opened at once share the database** instead of the
  second failing to open it and keeping nothing.
- **The example date in the Date box can be typed back** in locales whose own
  short date has month names, such as Arabic ones, which now show numbers.
- **Amounts in currencies with three decimal places**, such as the Kuwaiti
  dinar, are accepted as the Amount box's example shows them.

## [0.2.0] - 2026-11-18

- **The app checks for a new version when it starts**, by asking GitHub for the
  latest version number. If there is one, a dialog offers to open the download
  page in your web browser; nothing is downloaded or installed by the app. The
  Settings tab has a **Check for updates now** button, and the check when the
  app starts can be turned off there.

## [0.1.0] - 2026-11-18

- **First release.** Record what you spend, list a month's spending, see a
  summary by category, and add your own categories. Everything is kept in a
  SQLite database on your own computer.
- **Made for the keyboard and screen readers**, with the same layout as Common
  Speech Output: tabs down the left, one control on each line, and a status bar
  that screen readers announce. F1 lists the keyboard shortcuts, and there is a
  High Contrast appearance.
- **Dates and money follow your computer's locale**, including the currency
  symbol, separators and date order.
- **A debug log**, `debug.log`, is kept beside the database.
- **Installers** for macOS on Apple Silicon (.pkg) and Windows on x64 PCs (.msi).
