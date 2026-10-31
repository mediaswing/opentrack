# Open Budget Tracker: die Oberfläche auf Deutsch.
#
# Übersetzt aus en.ftl, der Hauptdatei; siehe die Hinweise dort. Wörter in
# geschweiften Klammern, wie { $name }, setzt die App ein: bitte unverändert
# lassen.

## Allgemein

category-unknown = Unbekannte Kategorie
category-label = Kategorie
save-changes = Änderungen speichern
hint-such-as = Zum Beispiel { $example }
nothing-deleted = Es wurde nichts gelöscht.
cancel = Abbrechen

## Registerkarten und Statuszeile

tab-spending = Ausgaben
tab-summary = Übersicht
tab-categories = Kategorien
tab-data = Import/Export
tab-settings = Einstellungen
tab-announce = Registerkarte { $tab }.
status-ready = Bereit.
status-ready-help = Bereit. Drücken Sie { $key } für die Tastenkürzel.
error-read-categories = Die Kategorien konnten nicht gelesen werden: { $error }
error-count-entries = Die Einträge konnten nicht gezählt werden: { $error }
error-open-database = Die Budget-Datenbank unter { $path } konnte nicht geöffnet werden: { $error }
error-start-window = Das Fenster konnte nicht geöffnet werden: { $error }

## Datum

month-1 = Januar
month-2 = Februar
month-3 = März
month-4 = April
month-5 = Mai
month-6 = Juni
month-7 = Juli
month-8 = August
month-9 = September
month-10 = Oktober
month-11 = November
month-12 = Dezember
month-short-1 = Jan.
month-short-2 = Feb.
month-short-3 = März
month-short-4 = Apr.
month-short-5 = Mai
month-short-6 = Juni
month-short-7 = Juli
month-short-8 = Aug.
month-short-9 = Sept.
month-short-10 = Okt.
month-short-11 = Nov.
month-short-12 = Dez.
weekday-short-1 = Mo.
weekday-short-2 = Di.
weekday-short-3 = Mi.
weekday-short-4 = Do.
weekday-short-5 = Fr.
weekday-short-6 = Sa.
weekday-short-7 = So.
month-year = { $month } { $year }
long-date-dmy = { $weekday }, { $day }. { $month } { $year }
long-date-mdy = { $weekday }, { $month } { $day }, { $year }
long-date-ymd = { $weekday }, { $year } { $month } { $day }
medium-date-dmy = { $day }. { $month } { $year }
medium-date-mdy = { $month } { $day }, { $year }
medium-date-ymd = { $year } { $month } { $day }
day-month-dm = { $day }. { $month }
day-month-md = { $month } { $day }
period-dates = { $first } bis { $last }
period-in-month = Im { $month }
period-from-to = Vom { $first } bis { $last }
date-day = Tag
date-month = Monat
date-year = Jahr
date-invalid = „{ $text }“ ist kein Datum. Geben Sie es als { $pattern } ein, zum Beispiel { $example }.
date-invalid-file = „{ $text }“ ist kein Datum. Geben Sie es als Jahr-Monat-Tag ein, zum Beispiel 2026-10-30.

## Beträge

amount-missing = Geben Sie einen Betrag ein, zum Beispiel { $example }.
amount-invalid = „{ $text }“ ist kein Betrag. Geben Sie einen ein wie { $example }.
amount-too-precise = Beträge können höchstens zwei Stellen nach dem Dezimalzeichen haben.
amount-too-large = Beträge können höchstens { $max } sein.

## Regionen, wie in den Einstellungen aufgeführt

region-en-GB = Vereinigtes Königreich
region-en-US = Vereinigte Staaten
region-en-AU = Australien
region-en-CA = Kanada (Englisch)
region-en-IE = Irland
region-en-IN = Indien
region-en-NZ = Neuseeland
region-en-SG = Singapur
region-en-ZA = Südafrika
region-de-DE = Deutschland
region-de-AT = Österreich
region-de-CH = Schweiz
region-fr-FR = Frankreich
region-fr-CA = Kanada (Französisch)
region-fr-BE = Belgien (Französisch)
region-nl-NL = Niederlande
region-nl-BE = Belgien (Niederländisch)
region-es-ES = Spanien
region-es-MX = Mexiko
region-es-AR = Argentinien
region-it-IT = Italien
region-pt-PT = Portugal
region-pt-BR = Brasilien
region-da-DK = Dänemark
region-fi-FI = Finnland
region-nb-NO = Norwegen
region-sv-SE = Schweden
region-pl-PL = Polen
region-cs-CZ = Tschechien
region-hu-HU = Ungarn
region-el-GR = Griechenland
region-tr-TR = Türkei
region-ru-RU = Russland
region-uk-UA = Ukraine
region-ja-JP = Japan
region-zh-CN = China
region-ko-KR = Südkorea

## Tasten, wie in Kürzeln wie „Strg+N“

key-ctrl = Strg
key-cmd = Cmd
key-option = Wahl
key-alt = Alt
key-shift = Umschalt
key-esc = Esc

## Tastenkürzel

action-new = Neuer Eintrag, oder neue Kategorie auf der Registerkarte Kategorien
action-save = Eintrag oder Kategorie speichern
action-edit = Ausgewählten Eintrag bearbeiten
action-delete = Ausgewählten Eintrag oder ausgewählte Kategorie löschen
action-find = Einträge suchen
action-previous-month = Vorheriger Monat oder Zeitraum
action-next-month = Nächster Monat oder Zeitraum
action-this-month = Zurück zu diesem Monat
action-summary = Ausgaben und Budget für den gezeigten Zeitraum ansagen
action-import = Datei importieren
action-export = In eine Datei exportieren
action-go-to-tab = Zu einer Registerkarte wechseln
action-next-tab = Nächste Registerkarte
action-previous-tab = Vorherige Registerkarte
action-help = Tastenkürzel auflisten
shortcuts-range = { $first } bis { $last }
shortcuts-either = { $first } oder { $second }
shortcuts-line = { $action }: { $keys }
shortcuts-title = Tastenkürzel
shortcuts-off-note = Tastenkürzel sind ausgeschaltet. Schalten Sie sie mit dem Kästchen Tastenkürzel auf der Registerkarte Einstellungen ein.
shortcuts-always = Diese Tasten funktionieren immer: Tab und Umschalt+Tab wechseln zwischen den Bedienelementen, Leertaste oder Eingabe drückt die ausgewählte Schaltfläche, die Pfeiltasten bewegen sich entlang der Registerkarten und durch die Liste der Einträge, und Esc beendet das Bearbeiten oder schließt eine Liste.
keys-nothing-to-save = Auf dieser Registerkarte gibt es nichts zu speichern. Ihr Budget wird laufend gespeichert.
keys-nothing-to-delete = Wählen Sie zum Löschen einen Eintrag auf der Registerkarte Ausgaben oder eine Kategorie auf der Registerkarte Kategorien.
keys-month-changed = { $month }: { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
}, { $spent } ausgegeben.

## Registerkarte Ausgaben

error-read-entries = Die Einträge konnten nicht gelesen werden: { $error }
spending-new-announce = Neuer Eintrag. Geben Sie ein, wofür das Geld ausgegeben wurde.
spending-find-announce = Einträge suchen. Geben Sie einen Teil eines Namens ein.
spending-editing-announce = { $name } wird bearbeitet. Ändern Sie den Eintrag und speichern Sie ihn, oder drücken Sie Esc zum Abbrechen.
spending-choose-first = Wählen Sie zuerst einen Eintrag in der Liste auf der Registerkarte Ausgaben.
spending-stopped-editing = Bearbeiten beendet. Es wurde nichts geändert.
spending-cleared = Das Formular wurde geleert.
spending-no-category = Fügen Sie zuerst auf der Registerkarte Kategorien eine Kategorie hinzu.
spending-described = { $name }, { $amount }, in { $category }
spending-described-dated = { $name }, { $amount }, am { $date }
error-save-entry = Der Eintrag konnte nicht gespeichert werden: { $error }
spending-elsewhere = Er liegt im { $month } und wird daher für { $shown } nicht aufgeführt.
spending-outside = Er liegt außerhalb von { $shown } und wird daher nicht aufgeführt.
spending-saved = { $entry } gespeichert.
spending-added = { $entry } hinzugefügt.
spending-next = Geben Sie den nächsten Eintrag ein, oder drücken Sie Tab, um weiterzugehen.
spending-choose-in-list = Wählen Sie zuerst einen Eintrag in der Liste.
spending-delete-title = Eintrag löschen
spending-delete-question = { $entry } löschen?
error-delete-entry = Der Eintrag konnte nicht gelöscht werden: { $error }
spending-deleted = { $entry } gelöscht.
spending-edit-heading = Eintrag { $line } bearbeiten
spending-add-heading = Eintrag hinzufügen
spending-name-label = Wofür?
spending-name-hint = Zum Beispiel Wocheneinkauf
spending-date-label = Datum ({ $pattern })
spending-amount-label = Betrag (mit Minuszeichen für eine Erstattung)
spending-add-button = Eintrag hinzufügen
spending-cancel-editing = Bearbeiten abbrechen (Esc)
spending-clear-form = Formular leeren (Esc)
month-previous = Vorheriger Monat
month-this = Dieser Monat
month-next = Nächster Monat
period-label = Gezeigter Zeitraum
period-one-month = Ein Monat
period-choose-dates = Von einem Datum bis zu einem anderen
period-first-label = Von
period-last-label = Bis
period-show-dates = Diesen Zeitraum zeigen
period-previous = Früherer Zeitraum
period-next = Späterer Zeitraum
period-backwards = Das „Bis“-Datum liegt vor dem „Von“-Datum. Geben Sie eines ein, das gleich oder später ist.
spending-list-all-heading = Einträge aller Monate
spending-list-month-heading = Einträge für { $month }
spending-show-label = Anzeigen
spending-show-only = Nur { $month }
spending-show-every = Alle Monate
spending-filter-label = Aufzuführende Kategorie
spending-every-category = Alle Kategorien
spending-find-label = Einträge suchen
spending-find-hint = Teil eines Namens
spending-none-match = Keine Einträge gefunden.
spending-none-yet = Noch keine Einträge.
spending-total = { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
}, insgesamt { $amount }.
spending-row-spoken = { $name }, { $amount }, { $category }, { $date }
spending-edit-chosen = Ausgewählten Eintrag bearbeiten
spending-delete-chosen = Ausgewählten Eintrag löschen

## Registerkarte Übersicht

error-sum-spending = Die Ausgaben konnten nicht zusammengerechnet werden: { $error }
summary-no-budgets = { $when } haben Sie { $spent } ausgegeben. Es sind keine Budgets festgelegt; legen Sie sie auf der Registerkarte Kategorien fest.
summary-within = { $when } haben Sie { $spent } Ihres Budgets von { $budget } ausgegeben, es bleiben { $left }.
summary-over = { $when } haben Sie { $spent } ausgegeben, das sind { $over } über Ihrem Budget von { $budget }.
summary-part-within = { $when } haben Sie { $spent } ausgegeben. Davon entfielen { $budgeted } auf Kategorien mit Budget, es bleiben { $left } Ihres Budgets von { $budget }.
summary-part-over = { $when } haben Sie { $spent } ausgegeben. Davon entfielen { $budgeted } auf Kategorien mit Budget, das sind { $over } über Ihrem Budget von { $budget }.
summary-one-over = { $name } liegt über dem Budget.
summary-many-over = { $count } Kategorien liegen über dem Budget: { $names }.
summary-heading = Ausgaben für { $period }
summary-total-name = Alle Kategorien mit Budget
summary-total-bar = Gesamt
summary-by-category = Nach Kategorie
summary-hidden = { $count ->
    [one] { $count } Kategorie ohne Ausgaben und ohne Budget wird nicht angezeigt.
   *[other] { $count } Kategorien ohne Ausgaben und ohne Budget werden nicht angezeigt.
}
summary-chart-heading = Die { $count } Monate bis { $month }
summary-chart-dates-heading = Monat für Monat, { $period }
summary-chart-description = Diagramm der Ausgaben pro Monat. { $months }.
summary-highest = Höchster Monat: { $amount }
summary-average = Durchschnitt pro Monat: { $amount }.
summary-budget-by-day = Budgets gelten pro Monat. Für diesen Zeitraum zählt jedes für ganze Monate voll und für angebrochene Monate anteilig nach Tagen.
summary-bar-no-budget = { $name }: { $spent } ausgegeben, kein Budget.
summary-bar-amounts = { $spent } von { $budget }
summary-bar-left = { $amount } übrig
summary-bar-over = { $amount } über dem Budget
summary-bar-spoken = { $name }: { $spent } von { $budget } Budget ausgegeben, { $left }.

## Registerkarte Kategorien

budget-negative = Ein Budget kann nicht kleiner als null sein.
categories-new-announce = Neue Kategorie. Geben Sie ihren Namen ein.
categories-none-yet = Es gibt noch keine Kategorien. Fügen Sie zuerst eine hinzu.
categories-saved-budget = { $name } gespeichert, mit einem monatlichen Budget von { $amount }.
categories-saved-no-budget = { $name } gespeichert, ohne Budget.
categories-added = Die Kategorie { $name } wurde hinzugefügt.
categories-none-to-delete = Es gibt keine Kategorien zum Löschen.
categories-delete-title = Kategorie löschen
categories-delete-question = Die Kategorie { $name } löschen?
categories-delete-move-question = Die Kategorie { $name } löschen und { $count ->
    [one] ihren Eintrag
   *[other] ihre { $count } Einträge
} nach { $to } verschieben?
categories-has-entries = { $name } enthält Einträge. Fügen Sie zuerst eine andere Kategorie hinzu, in die sie verschoben werden können.
categories-deleted = Die Kategorie { $name } wurde gelöscht.
error-delete-category = { $name } konnte nicht gelöscht werden: { $error }
categories-change-heading = Kategorie ändern
categories-none-add-below = Es gibt noch keine Kategorien. Fügen Sie unten eine hinzu.
categories-with-budget = { $name } (Budget { $amount })
categories-name-label = Name
categories-name-hint = Bis zu 32 Zeichen
categories-budget-label = Monatliches Budget (leer für keines)
categories-entry-count = { $count ->
    [0] Diese Kategorie enthält keine Einträge.
    [one] Diese Kategorie enthält { $count } Eintrag.
   *[other] Diese Kategorie enthält { $count } Einträge.
}
categories-move-label = Beim Löschen ihre Einträge verschieben nach
categories-delete-button = Diese Kategorie löschen
categories-add-heading = Kategorie hinzufügen
categories-new-name-label = Name der neuen Kategorie
categories-new-name-hint = Zum Beispiel Haustiere
categories-new-budget-label = Ihr monatliches Budget (leer für keines)
categories-add-button = Kategorie hinzufügen

## Registerkarte Import/Export

data-import-title = In Open Budget Tracker importieren
data-export-title = Aus Open Budget Tracker exportieren
data-filter-csv-or-json = Ausgaben (CSV) oder Sicherung (JSON)
data-filter-csv = Ausgaben (CSV)
data-filter-json = Sicherung (JSON)
data-filter-sql = MariaDB-Skript (SQL)
data-filter-db = SQLite-Datenbank
data-nothing-imported = Es wurde nichts importiert.
data-nothing-exported = Es wurde nichts exportiert.
data-exported-csv = { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
} nach { $file } exportiert.
data-exported-json = { $categories ->
    [one] { $categories } Kategorie
   *[other] { $categories } Kategorien
} und { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
} in { $file } gesichert.
data-exported-sql = Ein MariaDB-Skript mit allen Kategorien und { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträgen
} wurde in { $file } geschrieben.
error-export = Der Export ist fehlgeschlagen: { $error }
data-imported-csv = { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
} aus { $file } importiert.
data-imported-new-categories = { $count ->
    [one] Dafür wurde { $count } neue Kategorie hinzugefügt.
   *[other] Dafür wurden { $count } neue Kategorien hinzugefügt.
}
error-import-csv = { $file } konnte nicht importiert werden. Es wurde nichts hinzugefügt. { $error }
data-import-backup-title = Sicherung importieren
data-import-backup-question = Alles durch die Sicherung ersetzen, oder die Einträge der Sicherung zu den vorhandenen hinzufügen? Beim Ersetzen werden die jetzigen Kategorien und Einträge gelöscht.
data-replace = Ersetzen
data-add = Hinzufügen
data-replaced = Alles wurde durch { $categories ->
    [one] { $categories } Kategorie
   *[other] { $categories } Kategorien
} und { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
} aus { $file } ersetzt.
data-added = { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
} und { $categories ->
    [one] { $categories } neue Kategorie
   *[other] { $categories } neue Kategorien
} aus { $file } hinzugefügt.
error-import-json = { $file } konnte nicht importiert werden. Es wurde nichts geändert. { $error }
data-no-copy = Es wurde keine Kopie erstellt.
data-live-database = Das ist die Datenbank, die Open Budget Tracker gerade verwendet, daher kann sie nicht durch eine Kopie ersetzt werden. Wählen Sie einen anderen Namen oder Ordner.
data-live-database-export = Das ist die Datenbank, die Open Budget Tracker gerade verwendet, daher kann sie nicht durch einen Export ersetzt werden. Wählen Sie einen anderen Namen oder Ordner.
data-copied = Eine Kopie der Datenbank wurde als { $file } gespeichert.
error-copy-database = Die Datenbank konnte nicht kopiert werden: { $error }
data-export-heading = Export
data-export-note = CSV-Dateien lassen sich in Tabellenkalkulationen öffnen. JSON-Sicherungen enthalten alles, auch die Budgets, und können wieder importiert werden. Für ein Skript, das die Tabellen in MariaDB oder MySQL neu anlegt, verwenden Sie das Tastenkürzel In eine Datei exportieren und wählen den Dateityp SQL.
data-export-csv = Ausgaben als CSV exportieren
data-export-json = Sicherung von allem als JSON exportieren
data-import-heading = Import
data-import-note = Eine CSV-Datei braucht eine erste Zeile mit den Spaltennamen: name, date, price und category. Datumsangaben werden als Jahr-Monat-Tag geschrieben, zum Beispiel 2026-10-30. Kategorien, die es noch nicht gibt, werden hinzugefügt. Ist eine Zeile fehlerhaft, wird nichts importiert, und die Meldung nennt die Zeile.
data-import-csv = Ausgaben aus CSV importieren
data-import-json = JSON-Sicherung importieren
data-database-heading = Datenbank
data-database-location = Speicherort der Datenbank
data-copy-button = Kopie der Datenbank speichern

## Registerkarte Einstellungen

update-checking = Es wird nach Updates gesucht.
update-failed = Es konnte nicht nach Updates gesucht werden.
update-failed-detail = Es konnte nicht nach Updates gesucht werden: { $error }.
update-latest = Sie haben die neueste Version, { $version }.
update-title = Update verfügbar
update-available = Version { $new } von Open Budget Tracker ist verfügbar. Sie haben Version { $current }.
update-open-question = Die Download-Seite im Webbrowser öffnen?
update-later = Version { $version } ist verfügbar. Sie können sie später über die Registerkarte Einstellungen herunterladen.
update-opened = Die Download-Seite für Version { $version } wurde geöffnet.
error-open-browser = Der Webbrowser konnte nicht geöffnet werden: { $error }. Die Download-Seite ist { $url }
settings-language-heading = Sprache (Language)
settings-language-label = Sprache der App (Language)
settings-language-automatic = Automatisch: { $language } (Ihr System ist auf { $system } eingestellt)
settings-language-changed = Die App ist jetzt auf Deutsch.
settings-region-heading = Region
settings-region-automatic = Automatisch: { $region } (Ihr System ist auf { $system } eingestellt)
settings-region-label = Datum und Beträge geschrieben wie in
settings-region-changed = Datumsangaben werden als { $pattern } geschrieben, zum Beispiel { $example }, und Beträge so: { $amount }.
settings-dates-note = Datumsangaben werden als { $pattern } eingegeben, zum Beispiel { $example }, und so angezeigt: { $today }.
settings-year-heading = Geschäftsjahr
settings-year-day-label = Tag des Beginns
settings-year-month-label = Monat des Beginns
settings-year-note = Das aktuelle Geschäftsjahr geht vom { $period }. Wenn Sie auf der Registerkarte Ausgaben oder Übersicht „{ $choice }“ wählen, wird es gezeigt.
settings-year-changed = Das Geschäftsjahr beginnt jetzt am { $date }.
settings-usual-year = Den üblichen Beginn der Region verwenden ({ $date })
settings-money-heading = Geld
settings-currency-label = Währungssymbol
settings-currency-hint = Zum Beispiel €, £ oder $
settings-currency-refused = { $symbol } kann nicht als Währungssymbol verwendet werden, da es als Teil eines Betrags gelesen werden könnte.
settings-currency-refused-note = { $symbol } kann nicht als Währungssymbol verwendet werden: Es darf keine Ziffern enthalten und nicht nur aus Punkten, Kommas oder Vorzeichen bestehen. Beträge werden weiterhin so angezeigt: { $amount }.
settings-amounts-shown = Beträge werden so angezeigt: { $amount }.
settings-no-symbol = Kein Währungssymbol verwenden, da für die Region keines hinterlegt ist
settings-usual-symbol = Das übliche Symbol der Region verwenden, { $symbol }
settings-keyboard-heading = Tastatur
settings-shortcuts-checkbox = Tastenkürzel
settings-shortcuts-on = Tastenkürzel sind eingeschaltet.
settings-shortcuts-off = Tastenkürzel sind ausgeschaltet.
settings-list-shortcuts = Tastenkürzel auflisten
settings-deleting-heading = Löschen
settings-confirm-delete = Vor dem Löschen eines Eintrags nachfragen
settings-updates-heading = Updates
settings-updates-note = Bei der Suche nach Updates wird GitHub nach der neuesten Version gefragt. Es wird nichts über Sie oder Ihr Budget gesendet.
settings-check-at-start = Beim Start der App nach Updates suchen
settings-check-now = Jetzt nach Updates suchen
settings-log-heading = Debug-Protokoll
settings-log-note = Das Debug-Protokoll zeichnet auf, was die App getan hat, und alle Fehler, um bei der Behebung von Problemen zu helfen. Es zeichnet nie auf, wofür oder wie viel Sie Geld ausgegeben haben.
settings-log-checkbox = Debug-Protokoll schreiben
settings-log-dir-label = Protokollordner
settings-log-dir-hint = Ein Ordner
settings-use-folder = Diesen Ordner verwenden
settings-choose-folder = Ordner wählen…
settings-open-folder = Protokollordner öffnen
settings-unreadable = Ihre Einstellungen konnten nicht gelesen werden, daher gelten die üblichen.
settings-unreadable-kept = Ihre Einstellungen konnten nicht gelesen werden, daher gelten die üblichen. Die alte Einstellungsdatei wurde als { $file } aufbewahrt.
settings-about-heading = Über
settings-about = Open Budget Tracker { $version }. Die Programmdatei heißt opentrack.
log-writing = Das Debug-Protokoll wird nach { $path } geschrieben.
log-off = Das Debug-Protokoll ist ausgeschaltet.
log-off-error = Das Debug-Protokoll ist ausgeschaltet: { $error }.
error-start-log = Das Debug-Protokoll konnte in { $dir } nicht gestartet werden: { $error }
log-dir-not-absolute = Geben Sie den vollständigen Pfad eines Ordners für das Protokoll ein, oder wählen Sie einen.
log-dir-saved = Der Protokollordner wurde gespeichert. Setzen Sie ein Häkchen bei Debug-Protokoll schreiben, um es zu starten.
error-open-folder = { $dir } konnte nicht geöffnet werden: { $error }

## Datenbank, in den Einstellungen, und ein MySQL- oder MariaDB-Server

settings-database-heading = Datenbank
settings-database-label = Wo das Budget gespeichert wird
settings-database-built-in = In der App, auf diesem Computer
settings-database-server = Auf einem MySQL- oder MariaDB-Server
settings-database-in-use = Jetzt verwendet: { $location }
settings-use-built-in = Eingebaute Datenbank verwenden
settings-server-host = Server
settings-server-port = Port
settings-server-database = Datenbank
settings-server-user = Benutzername
settings-server-password = Passwort
settings-server-tls = Verbindung verschlüsseln (TLS)
settings-server-note = Die App legt ihre Tabellen in der Datenbank an, falls sie noch fehlen. Das Passwort wird in der Einstellungsdatei auf diesem Computer gespeichert. Schalten Sie die Verschlüsselung nur für einen Server auf diesem Computer oder in einem vertrauenswürdigen Netzwerk aus.
settings-server-connect = Verbinden und diesen Server verwenden
settings-server-port-invalid = „{ $text }“ ist keine Portnummer. Geben Sie eine Zahl von 1 bis 65535 ein, zum Beispiel 3306.
server-describe = { $database } auf { $host }:{ $port }
server-host-missing = Geben Sie den Namen oder die Adresse des Servers ein.
server-database-missing = Geben Sie den Namen der Datenbank auf dem Server ein.
error-connect-server = Keine Verbindung zu { $server } möglich
server-tls-hint = Das Zertifikat des Servers ist keines, dem dieser Computer vertraut, wie bei einem Server, der es selbst erstellt hat, üblich. Wenn der Server auf diesem Computer oder in einem vertrauenswürdigen Netzwerk ist, schalten Sie „Verbindung verschlüsseln“ aus und verbinden Sie sich erneut.
error-path-not-text = Der Dateiname { $path } enthält Zeichen, die die Datenbank nicht verwenden kann. Wählen Sie einen anderen Namen.
error-reconnect-server = Der Datenbankserver hat die Verbindung geschlossen, und die erneute Verbindung ist fehlgeschlagen
error-server-tables = Die Tabellen in { $server } konnten nicht eingerichtet werden
server-not-innodb = Diese Tabellen verwenden nicht InnoDB, daher können Änderungen nicht in einem Schritt erfolgen: { $tables }. Wandeln Sie sie mit ALTER TABLE … ENGINE = InnoDB um.
server-no-copy = Das Budget liegt auf einem Datenbankserver und kann daher nicht hier in eine Datei kopiert werden. Verwenden Sie die Sicherungswerkzeuge des Servers oder exportieren Sie eine JSON-Sicherung.
server-copy-title = Budget auf den Server kopieren?
server-copy-question = { $server } enthält noch keine Einträge. { $count ->
    [one] Den 1 Eintrag
   *[other] Die { $count } Einträge
} und { $categories ->
    [one] 1 Kategorie
   *[other] { $categories } Kategorien
} aus der jetzt verwendeten Datenbank dorthin kopieren?
server-copy-replaces = Was dort bereits ist, etwa die Startkategorien, wird ersetzt. Die jetzt verwendete Datenbank bleibt unverändert.
error-copy-to-server = Das Budget konnte nicht nach { $server } kopiert werden: { $error }
server-in-use = { $server } wird jetzt verwendet.
server-in-use-copied = { $count ->
    [one] 1 Eintrag wurde
   *[other] { $count } Einträge wurden
} nach { $server } kopiert, und die Datenbank wird jetzt verwendet.
built-in-in-use = Die eingebaute Datenbank unter { $path } wird jetzt verwendet.
startup-server-failed = Keine Verbindung zum Datenbankserver möglich. { $error }
startup-server-fallback = Diesmal die eingebaute Datenbank verwenden? Die Einstellungen bleiben unverändert, daher wird der Server beim nächsten Start erneut versucht.
status-built-in-fallback = Der Datenbankserver war nicht erreichbar, daher wird vorerst die eingebaute Datenbank verwendet.
data-server-note = Das Budget liegt auf einem MySQL- oder MariaDB-Server. Sichern Sie es mit den Werkzeugen des Servers oder exportieren Sie oben eine JSON-Sicherung.

## Die Kategorien, mit denen ein neues Budget beginnt, je höchstens 32 Zeichen

start-entertainment = Unterhaltung
start-food-and-drink = Essen und Trinken
start-utilities-and-bills = Nebenkosten und Rechnungen
start-web-services = Webdienste
start-transport = Verkehr
start-accommodation = Unterkunft
start-healthcare = Gesundheit
start-social-events = Gesellige Anlässe

## Prüfen des Budgets und der Dateien

error-create-folder = { $path } konnte nicht erstellt werden
error-open-file = { $path } konnte nicht geöffnet werden
error-read-file = { $path } konnte nicht gelesen werden
error-write-file = { $path } konnte nicht geschrieben werden
db-newer = die Datenbank wurde von einer neueren Version von Open Budget Tracker erstellt (Schema { $version })
category-exists = Es gibt bereits eine Kategorie namens { $name }.
category-name-missing = Geben Sie einen Namen für die Kategorie ein.
category-name-too-long = Kategorienamen können höchstens { $max } Zeichen lang sein.
budget-range = Ein Budget muss zwischen 0 und 999.999,99 liegen.
db-move-entries-first = { $count ->
    [one] Wählen Sie zuerst eine andere Kategorie für den Eintrag dieser Kategorie.
   *[other] Wählen Sie zuerst eine andere Kategorie für die { $count } Einträge dieser Kategorie.
}
entry-gone = Eintrag { $line } existiert nicht mehr.
entry-name-missing = Geben Sie ein, wofür das Geld ausgegeben wurde.
entry-name-too-long = Namen können höchstens { $max } Zeichen lang sein.
entry-amount-too-large = Beträge können höchstens 999.999,99 sein.
entry-year-range = Daten müssen in den Jahren { $first } bis { $last } liegen.
context-category = Kategorie { $id } ({ $name })
context-budget = Budget für Kategorie { $name }
context-entry = Eintrag { $line }
csv-header = Die erste Zeile muss die Spalten benennen: name, date, price und category.
csv-row = Zeile { $row }
csv-row-error = Zeile { $row }: { $error }
csv-name-empty = Zeile { $row }: Der Name ist leer.
csv-name-too-long = Zeile { $row }: Namen können höchstens { $max } Zeichen lang sein.
backup-not-ours = Das ist keine Sicherung von Open Budget Tracker.
backup-newer = Diese Sicherung wurde von einer neueren Version von Open Budget Tracker erstellt.
backup-entry-error = Eintrag { $line }: { $error }
backup-missing-category = Eintrag { $line } gehört zur Kategorie { $category }, die in der Sicherung fehlt.
file-too-large = { $file } ist zu groß zum Importieren.
file-not-utf8 = { $file } ist kein UTF-8-Text.
update-unreachable = GitHub konnte für die Suche nach Updates nicht erreicht werden
update-http-error = GitHub hat bei der Suche nach Updates den HTTP-Fehler { $code } gemeldet
update-bad-reply = GitHub hat eine unerwartete Antwort gesendet
