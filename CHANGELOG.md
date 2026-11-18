# Changelog

## [Unreleased]

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
