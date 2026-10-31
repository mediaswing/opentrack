# Open Budget Tracker: the interface in English.
#
# This is the main file: every other language is translated from it, and
# English is used for any message a translation doesn't have. To add a
# language, copy this file to assets/i18n/<language tag>.ftl, translate the
# text after each "=", and list it in src/i18n.rs.
#
# Words in braces, such as { $name }, are filled in by the app; keep them
# as they are. A number such as { $count } can choose between wordings:
#
#     spending-total = { $count ->
#         [one] { $count } entry
#        *[other] { $count } entries
#     }, totalling { $amount }.
#
# The names in brackets are plural categories (zero, one, two, few, many,
# other), or an exact number such as [0]. The variant marked * is used
# when no other matches.

## Shared

category-unknown = Unknown category
category-label = Category
save-changes = Save changes
hint-such-as = Such as { $example }
nothing-deleted = Nothing was deleted.
cancel = Cancel

## Tabs and the status line

tab-spending = Spending
tab-summary = Summary
tab-categories = Categories
tab-data = Import/Export
tab-settings = Settings
tab-announce = { $tab } tab.
status-ready = Ready.
status-ready-help = Ready. Press { $key } for the keyboard shortcuts.
error-read-categories = Could not read the categories: { $error }
error-count-entries = Could not count the entries: { $error }
error-open-database = Could not open the budget database at { $path }: { $error }
error-start-window = Could not start the window: { $error }

## Dates

month-1 = January
month-2 = February
month-3 = March
month-4 = April
month-5 = May
month-6 = June
month-7 = July
month-8 = August
month-9 = September
month-10 = October
month-11 = November
month-12 = December
month-short-1 = Jan
month-short-2 = Feb
month-short-3 = Mar
month-short-4 = Apr
month-short-5 = May
month-short-6 = Jun
month-short-7 = Jul
month-short-8 = Aug
month-short-9 = Sep
month-short-10 = Oct
month-short-11 = Nov
month-short-12 = Dec
# Monday is 1.
weekday-short-1 = Mon
weekday-short-2 = Tue
weekday-short-3 = Wed
weekday-short-4 = Thu
weekday-short-5 = Fri
weekday-short-6 = Sat
weekday-short-7 = Sun
# Such as "October 2026".
month-year = { $month } { $year }
# A date in the list of entries, for regions that write the day, month or
# year first: "Fri 30 Oct 2026", "Fri, Oct 30, 2026", "Fri, 2026 Oct 30".
long-date-dmy = { $weekday } { $day } { $month } { $year }
long-date-mdy = { $weekday }, { $month } { $day }, { $year }
long-date-ymd = { $weekday }, { $year } { $month } { $day }
# A date without its weekday: "6 Apr 2026", "Apr 6, 2026", "2026 Apr 6".
medium-date-dmy = { $day } { $month } { $year }
medium-date-mdy = { $month } { $day }, { $year }
medium-date-ymd = { $year } { $month } { $day }
# A day of the year, with the month's full name: "6 April", "April 6".
day-month-dm = { $day } { $month }
day-month-md = { $month } { $day }
# The dates shown on the Spending and Summary tabs, such as
# "6 Apr 2026 to 5 Apr 2027".
period-dates = { $first } to { $last }
# The dates shown, starting a sentence that goes on "you spent...".
period-in-month = In { $month }
period-from-to = From { $first } to { $last }
# Make up how a short date is typed, such as "day/month/year".
date-day = day
date-month = month
date-year = year
date-invalid = "{ $text }" is not a date. Type one as { $pattern }, such as { $example }.
date-invalid-file = "{ $text }" is not a date. Type one as year-month-day, such as 2026-10-30.

## Amounts

amount-missing = Type an amount, such as { $example }.
amount-invalid = "{ $text }" is not an amount. Type one such as { $example }.
amount-too-precise = Amounts can have at most two digits after the decimal mark.
amount-too-large = Amounts can be at most { $max }.

## Regions, as listed on the Settings tab

region-en-GB = United Kingdom
region-en-US = United States
region-en-AU = Australia
region-en-CA = Canada (English)
region-en-IE = Ireland
region-en-IN = India
region-en-NZ = New Zealand
region-en-SG = Singapore
region-en-ZA = South Africa
region-de-DE = Germany
region-de-AT = Austria
region-de-CH = Switzerland
region-fr-FR = France
region-fr-CA = Canada (French)
region-fr-BE = Belgium (French)
region-nl-NL = Netherlands
region-nl-BE = Belgium (Dutch)
region-es-ES = Spain
region-es-MX = Mexico
region-es-AR = Argentina
region-it-IT = Italy
region-pt-PT = Portugal
region-pt-BR = Brazil
region-da-DK = Denmark
region-fi-FI = Finland
region-nb-NO = Norway
region-sv-SE = Sweden
region-pl-PL = Poland
region-cs-CZ = Czechia
region-hu-HU = Hungary
region-el-GR = Greece
region-tr-TR = Türkiye
region-ru-RU = Russia
region-uk-UA = Ukraine
region-ja-JP = Japan
region-zh-CN = China
region-ko-KR = South Korea

## Keys, as shown in shortcuts such as "Ctrl+N"

key-ctrl = Ctrl
key-cmd = Cmd
key-option = Option
key-alt = Alt
key-shift = Shift
key-esc = Esc

## Keyboard shortcuts

action-new = New entry, or new category on the Categories tab
action-save = Save the entry or category
action-edit = Edit the selected entry
action-delete = Delete the selected entry or category
action-find = Find entries
action-previous-month = Previous month, or the dates before
action-next-month = Next month, or the dates after
action-this-month = Back to this month
action-summary = Say the spending and budget for the dates shown
action-import = Import a file
action-export = Export to a file
action-go-to-tab = Go to a tab
action-next-tab = Next tab
action-previous-tab = Previous tab
action-help = List the keyboard shortcuts
# Such as "Ctrl+1 to Ctrl+5".
shortcuts-range = { $first } to { $last }
shortcuts-either = { $first } or { $second }
shortcuts-line = { $action }: { $keys }
shortcuts-title = Keyboard shortcuts
shortcuts-off-note = Keyboard shortcuts are turned off. Turn them on with the Keyboard shortcuts box on the Settings tab.
shortcuts-always = These keys always work: Tab and Shift+Tab move between controls, Space or Enter presses the focused button, the arrow keys move along the tabs and through the list of entries, and Escape stops editing or closes a list.
keys-nothing-to-save = There is nothing to save on this tab. Your budget is saved as you go.
keys-nothing-to-delete = Choose an entry on the Spending tab, or a category on the Categories tab, to delete.
keys-month-changed = { $month }: { $count ->
    [one] { $count } entry
   *[other] { $count } entries
}, { $spent } spent.

## Spending tab

error-read-entries = Could not read the entries: { $error }
spending-new-announce = New entry. Type what the money was spent on.
spending-find-announce = Find entries. Type part of a name.
spending-editing-announce = Editing { $name }. Change it, then save, or press Escape to cancel.
spending-choose-first = Choose an entry in the list on the Spending tab first.
spending-stopped-editing = Stopped editing. Nothing was changed.
spending-cleared = Cleared the form.
spending-no-category = Add a category on the Categories tab first.
# An entry, as said when it is saved: "Lunch, £8.50, in Food and Drink".
spending-described = { $name }, { $amount }, in { $category }
# An entry, as said when it is deleted: "Lunch, £8.50, on Fri 30 Oct 2026".
spending-described-dated = { $name }, { $amount }, on { $date }
error-save-entry = Could not save the entry: { $error }
spending-elsewhere = It is in { $month }, so it isn't listed for { $shown }.
spending-outside = It is outside { $shown }, so it isn't listed.
spending-saved = Saved { $entry }.
spending-added = Added { $entry }.
spending-next = Type the next entry, or press Tab to move on.
spending-choose-in-list = Choose an entry in the list first.
spending-delete-title = Delete entry
spending-delete-question = Delete { $entry }?
error-delete-entry = Could not delete the entry: { $error }
spending-deleted = Deleted { $entry }.
spending-edit-heading = Edit entry { $line }
spending-add-heading = Add an entry
spending-name-label = What was it?
spending-name-hint = Such as Weekly shop
spending-date-label = Date ({ $pattern })
spending-amount-label = Amount (a minus sign for a refund)
spending-add-button = Add entry
spending-cancel-editing = Cancel editing (Esc)
spending-clear-form = Clear the form (Esc)
month-previous = Previous month
month-this = This month
month-next = Next month
period-label = Dates to show
period-one-month = One month
period-choose-dates = From one date to another
period-first-label = From
period-last-label = To
period-show-dates = Show these dates
period-previous = Earlier dates
period-next = Later dates
period-backwards = The "To" date is before the "From" date. Type one on or after it.
spending-list-all-heading = Entries in every month
spending-list-month-heading = Entries for { $month }
spending-show-label = Show
spending-show-only = Only { $month }
spending-show-every = Every month
spending-filter-label = Category to list
spending-every-category = Every category
spending-find-label = Find entries
spending-find-hint = Part of a name
spending-none-match = No entries match.
spending-none-yet = No entries yet.
spending-total = { $count ->
    [one] { $count } entry
   *[other] { $count } entries
}, totalling { $amount }.
# What a screen reader says for an entry in the list.
spending-row-spoken = { $name }, { $amount }, { $category }, { $date }
spending-edit-chosen = Edit the chosen entry
spending-delete-chosen = Delete the chosen entry

## Summary tab

error-sum-spending = Could not add up the spending: { $error }
summary-no-budgets = { $when } you spent { $spent }. No budgets are set; add them on the Categories tab.
summary-within = { $when } you spent { $spent } of your { $budget } budget, leaving { $left }.
summary-over = { $when } you spent { $spent }, which is { $over } over your { $budget } budget.
summary-part-within = { $when } you spent { $spent }. { $budgeted } of it was in categories with a budget, leaving { $left } of your { $budget } budget.
summary-part-over = { $when } you spent { $spent }. { $budgeted } of it was in categories with a budget, which is { $over } over your { $budget } budget.
summary-one-over = { $name } is over budget.
summary-many-over = { $count } categories are over budget: { $names }.
summary-heading = Spending for { $period }
summary-total-name = All categories with a budget
summary-total-bar = Total
summary-by-category = By category
summary-hidden = { $count ->
    [one] { $count } category with nothing spent and no budget isn't shown.
   *[other] { $count } categories with nothing spent and no budget aren't shown.
}
summary-chart-heading = The { $count } months to { $month }
summary-chart-dates-heading = Month by month, { $period }
summary-chart-description = Chart of spending each month. { $months }.
summary-highest = Highest month: { $amount }
summary-average = Average a month: { $amount }.
summary-budget-by-day = Budgets are monthly, so for these dates each counts in full for every whole month and in part, by the day, for part of a month.
summary-bar-no-budget = { $name }: { $spent } spent, no budget.
summary-bar-amounts = { $spent } of { $budget }
summary-bar-left = { $amount } left
summary-bar-over = { $amount } over budget
summary-bar-spoken = { $name }: { $spent } spent of { $budget } budget, { $left }.

## Categories tab

budget-negative = A budget can't be less than nothing.
categories-new-announce = New category. Type its name.
categories-none-yet = There are no categories yet. Add one first.
categories-saved-budget = Saved { $name }, with a monthly budget of { $amount }.
categories-saved-no-budget = Saved { $name }, with no budget.
categories-added = Added the category { $name }.
categories-none-to-delete = There are no categories to delete.
categories-delete-title = Delete category
categories-delete-question = Delete the category { $name }?
categories-delete-move-question = Delete the category { $name }, and move { $count ->
    [one] its entry
   *[other] its { $count } entries
} to { $to }?
categories-has-entries = { $name } has entries. Add another category to move them to first.
categories-deleted = Deleted the category { $name }.
error-delete-category = Could not delete { $name }: { $error }
categories-change-heading = Change a category
categories-none-add-below = There are no categories yet. Add one below.
# A category in the list of categories to change.
categories-with-budget = { $name } (budget { $amount })
categories-name-label = Name
categories-name-hint = Up to 32 characters
categories-budget-label = Monthly budget (empty for none)
categories-entry-count = { $count ->
    [0] No entries are in this category.
    [one] { $count } entry is in this category.
   *[other] { $count } entries are in this category.
}
categories-move-label = When deleting, move its entries to
categories-delete-button = Delete this category
categories-add-heading = Add a category
categories-new-name-label = Name of the new category
categories-new-name-hint = Such as Pets
categories-new-budget-label = Its monthly budget (empty for none)
categories-add-button = Add category

## Import/Export tab

data-import-title = Import into Open Budget Tracker
data-export-title = Export from Open Budget Tracker
data-filter-csv-or-json = Spending (CSV) or backup (JSON)
data-filter-csv = Spending (CSV)
data-filter-json = Backup (JSON)
data-filter-sql = MariaDB script (SQL)
data-filter-db = SQLite database
data-nothing-imported = Nothing was imported.
data-nothing-exported = Nothing was exported.
data-exported-csv = Exported { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} to { $file }.
data-exported-json = Backed up { $categories ->
    [one] { $categories } category
   *[other] { $categories } categories
} and { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} to { $file }.
data-exported-sql = Wrote a MariaDB script with every category and { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} to { $file }.
error-export = Could not export: { $error }
data-imported-csv = Imported { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} from { $file }.
data-imported-new-categories = { $count ->
    [one] Added { $count } new category for them.
   *[other] Added { $count } new categories for them.
}
error-import-csv = Could not import { $file }. Nothing was added. { $error }
data-import-backup-title = Import backup
data-import-backup-question = Replace everything with the backup, or add the backup's entries to what is here? Replacing deletes the categories and entries here now.
data-replace = Replace
data-add = Add
data-replaced = Replaced everything with { $categories ->
    [one] { $categories } category
   *[other] { $categories } categories
} and { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} from { $file }.
data-added = Added { $count ->
    [one] { $count } entry
   *[other] { $count } entries
} and { $categories ->
    [one] { $categories } new category
   *[other] { $categories } new categories
} from { $file }.
error-import-json = Could not import { $file }. Nothing was changed. { $error }
data-no-copy = No copy was made.
data-live-database = That is the database Open Budget Tracker is using, so it can't be replaced with a copy. Choose another name or folder.
data-live-database-export = That is the database Open Budget Tracker is using, so it can't be replaced with an export. Choose another name or folder.
data-copied = Saved a copy of the database as { $file }.
error-copy-database = Could not copy the database: { $error }
data-export-heading = Export
data-export-note = CSV files open in spreadsheets. JSON backups hold everything, budgets included, and can be imported again. For a script that recreates the tables in MariaDB or MySQL, use the Export to a file shortcut and choose the SQL file type.
data-export-csv = Export spending as CSV
data-export-json = Export a backup of everything as JSON
data-import-heading = Import
# The column names (name, date, price, category) are part of the file
# format, so they stay in English.
data-import-note = A CSV file needs a first row naming its columns: name, date, price and category. Dates are written year-month-day, such as 2026-10-30. Categories that don't exist yet are added. If any row is wrong, nothing is imported, and the message says which row.
data-import-csv = Import spending from CSV
data-import-json = Import a JSON backup
data-database-heading = Database
data-database-location = Where the database is kept
data-copy-button = Save a copy of the database

## Settings tab

update-checking = Checking for updates.
update-failed = Could not check for updates.
update-failed-detail = Could not check for updates: { $error }.
update-latest = You have the latest version, { $version }.
update-title = Update available
update-available = Version { $new } of Open Budget Tracker is available. You have version { $current }.
update-open-question = Open the download page in your web browser?
update-later = Version { $version } is available. You can download it later from the Settings tab.
update-opened = Opened the download page for version { $version }.
error-open-browser = Could not open the web browser: { $error }. The download page is { $url }
settings-language-heading = Language
# Keep the English word "Language" in a translation, so someone who can't
# read the language they ended up in can still find the way back.
settings-language-label = Language of the app
settings-language-automatic = Automatic: { $language } (your system is set to { $system })
settings-language-changed = The app is now in English.
settings-region-heading = Region
settings-region-automatic = Automatic: { $region } (your system is set to { $system })
settings-region-label = Dates and amounts written as in
settings-region-changed = Dates are written { $pattern }, such as { $example }, and amounts like this: { $amount }.
settings-dates-note = Dates are typed { $pattern }, such as { $example }, and shown like this: { $today }.
settings-year-heading = Financial year
settings-year-day-label = Day it starts
settings-year-month-label = Month it starts
settings-year-note = This financial year is { $period }. Choosing "{ $choice }" on the Spending or Summary tab shows it.
settings-year-changed = The financial year now starts on { $date }.
settings-usual-year = Use the region's usual start ({ $date })
settings-money-heading = Money
settings-currency-label = Currency symbol
settings-currency-hint = Such as £, $ or €
settings-currency-refused = { $symbol } can't be used as a currency symbol, as it could be read as part of an amount.
settings-currency-refused-note = { $symbol } can't be used as a currency symbol: it can't contain digits or be only dots, commas or signs. Amounts are still shown like this: { $amount }.
settings-amounts-shown = Amounts are shown like this: { $amount }.
settings-no-symbol = Use no currency symbol, as the region has none listed
settings-usual-symbol = Use the region's usual symbol, { $symbol }
settings-keyboard-heading = Keyboard
settings-shortcuts-checkbox = Keyboard shortcuts
settings-shortcuts-on = Keyboard shortcuts are on.
settings-shortcuts-off = Keyboard shortcuts are off.
settings-list-shortcuts = List the keyboard shortcuts
settings-deleting-heading = Deleting
settings-confirm-delete = Ask before deleting an entry
settings-updates-heading = Updates
settings-updates-note = Checking for updates asks GitHub for the newest version. Nothing about you or your budget is sent.
settings-check-at-start = Check for updates when the app starts
settings-check-now = Check for updates now
settings-log-heading = Debug log
settings-log-note = The debug log records what the app did and any errors, to help fix problems. It never records what you spent money on or how much.
settings-log-checkbox = Write a debug log
settings-log-dir-label = Log folder
settings-log-dir-hint = A folder
settings-use-folder = Use this folder
settings-choose-folder = Choose a folder…
settings-open-folder = Open the log folder
settings-about-heading = About
settings-about = Open Budget Tracker { $version }. The program file is called opentrack.
log-writing = Writing the debug log to { $path }.
log-off = The debug log is off.
log-off-error = The debug log is off: { $error }.
error-start-log = Could not start the debug log in { $dir }: { $error }
log-dir-not-absolute = Type the full path of a folder for the log, or choose one.
log-dir-saved = Saved the log folder. Tick Write a debug log to start it.
error-open-folder = Could not open { $dir }: { $error }

## The categories a new budget starts with, at most 32 characters each

start-entertainment = Entertainment
start-food-and-drink = Food and Drink
start-utilities-and-bills = Utilities and Bills
start-web-services = Web Services
start-transport = Transport
start-accommodation = Accommodation
start-healthcare = Healthcare
start-social-events = Social Events

## Checking the budget and files

error-create-folder = could not create { $path }
error-open-file = could not open { $path }
error-read-file = could not read { $path }
error-write-file = could not write { $path }
db-newer = the database was made by a newer version of Open Budget Tracker (schema { $version })
category-exists = There is already a category called { $name }.
category-name-missing = Type a name for the category.
category-name-too-long = Category names can be at most { $max } characters long.
budget-range = A budget must be between 0 and 999,999.99.
db-move-entries-first = { $count ->
    [one] Choose another category for this category's entry first.
   *[other] Choose another category for this category's { $count } entries first.
}
entry-gone = Entry { $line } no longer exists.
entry-name-missing = Type what the money was spent on.
entry-name-too-long = Names can be at most { $max } characters long.
entry-amount-too-large = Amounts can be at most 999,999.99.
# Where a problem is, in an error such as "entry 12: Names can be at most…".
context-category = category { $id } ({ $name })
context-budget = budget for category { $name }
context-entry = entry { $line }
csv-header = The first row must name the columns: name, date, price and category.
csv-row = row { $row }
csv-row-error = Row { $row }: { $error }
csv-name-empty = Row { $row }: the name is empty.
csv-name-too-long = Row { $row }: names can be at most { $max } characters long.
backup-not-ours = This is not an Open Budget Tracker backup.
backup-newer = This backup was made by a newer version of Open Budget Tracker.
backup-entry-error = Entry { $line }: { $error }
backup-missing-category = Entry { $line } is in category { $category }, which the backup doesn't have.
file-too-large = { $file } is too large to import.
file-not-utf8 = { $file } is not UTF-8 text.
update-unreachable = could not reach GitHub to check for updates
update-http-error = GitHub returned HTTP error { $code } when checking for updates
update-bad-reply = GitHub sent an unexpected reply
