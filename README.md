# Claude-Nutzung

Eine kleine macOS-App für die Menüleiste. Sie zeigt, wie viel des
Claude-Abo-Limits verbraucht ist und wann es zurückgesetzt wird.

In der Menüleiste steht die Session-Auslastung als Prozentzahl. Ein Klick auf
das Symbol öffnet ein Fenster mit allen Limits, Fortschrittsbalken und
Reset-Countdown. Die Daten werden alle 5 Minuten aktualisiert.

## Wichtig: inoffizielle Datenquelle

Die App liest den Endpunkt `https://api.anthropic.com/api/oauth/usage`. Der ist
**undokumentiert und nicht offiziell**. Claude Code selbst benutzt ihn, aber er
kann sich jederzeit ändern oder verschwinden. Passiert das, zeigt die App eine
Meldung statt abzustürzen — funktionieren wird sie dann trotzdem nicht mehr.

Dasselbe gilt für den Speicherort der Zugangsdaten: die App liest das
OAuth-Token von Claude Code aus dem macOS-Schlüsselbund (Eintrag
`Claude Code-credentials`). Ändert Claude Code das, muss der Name in
`src-tauri/src/credentials.rs` nachgezogen werden — er steht dort als einzelne
Konstante.

## Voraussetzungen

- macOS
- Claude Code, angemeldet **mit einem Abo** (nicht mit einem API-Schlüssel)
- Zum Bauen: Node.js (LTS), Rust, Xcode Command Line Tools

## Installation

```bash
npm install
npm run tauri build
```

Das Ergebnis liegt unter
`src-tauri/target/release/bundle/macos/Claude-Nutzung.app` und kann nach
`/Applications` gezogen werden.

Gebaut wird nur die `.app` (`bundle.targets` in `tauri.conf.json` steht auf
`["app"]`). Ein DMG wird bewusst nicht erzeugt — es wird für die eigene
Installation nicht gebraucht, und das DMG-Bundling schlug auf diesem Rechner
aus ungeklärter Ursache fehl.

Die App ist **nicht signiert und nicht notarisiert**. Beim ersten Start blockt
macOS sie deshalb. Abhilfe: im Finder Rechtsklick auf die App → „Öffnen" →
im Dialog nochmals „Öffnen". Das ist nur einmal nötig.

Beim ersten Abruf fragt macOS nach Erlaubnis für den Schlüsselbund. Ohne diese
Erlaubnis kann die App das Token nicht lesen.

## Bedienung

| Aktion | Wirkung |
|---|---|
| Linksklick aufs Symbol | Fenster zeigen oder verstecken |
| Klick irgendwo daneben | Fenster schliesst sich |
| Rechtsklick aufs Symbol | Menü mit „Aktualisieren" und „Beenden" |
| Knopf „Aktualisieren" | Sofortiger Abruf |
| Haken „Beim Login starten" | Autostart ein- oder ausschalten |

Bei 80 % und 95 % Auslastung kommt eine Benachrichtigung — je Limit und
Reset-Zeitraum einmal.

In der Menüleiste steht die Session-Auslastung. Ein Sternchen dahinter
(`42 %*`) heisst: der Wert stammt aus einem früheren Abruf, der letzte ist
fehlgeschlagen — der Grund steht im Fenster. Ein `⚠︎` statt einer Zahl heisst,
dass noch gar kein Wert vorliegt.

Das Fenster schliesst sich, sobald es den Fokus verliert, wie ein übliches
Menüleisten-Popover. Beim Öffnen der Entwicklerwerkzeuge verschwindet es
deshalb ebenfalls.

## Umgang mit dem Token

- Das Token wird nur im Arbeitsspeicher gehalten, nur für die Dauer der
  Anfrage. Es wird nie auf die Platte geschrieben, nie geloggt und nie ans
  Frontend gegeben.
- Die App erneuert das Token **nicht** selbst. Claude Code rotiert es. Ist es
  abgelaufen, steht im Fenster der Hinweis, kurz Claude Code zu öffnen.
- Das Abfrageintervall liegt bei 5 Minuten. Nach einem HTTP 429 verdoppelt es
  sich bis maximal 30 Minuten.

## Entwicklung

```bash
npm run tauri dev     # App mit Hot Reload starten
cd src-tauri && cargo test   # Unit-Tests (ohne Netzwerk)
npx tsc --noEmit      # Frontend prüfen
```

Der Rust-Teil liegt in `src-tauri/src/`:

| Datei | Zweck |
|---|---|
| `credentials.rs` | Token aus dem Schlüsselbund |
| `usage.rs` | Abruf und Normalisierung beider Antwortformate |
| `tray.rs` | Symbol und Menü in der Menüleiste |
| `notify.rs` | Benachrichtigungen bei 80 % und 95 % |
| `lib.rs` | Zustand, Poll-Schleife, Commands |

Der Projektplan mit allen Meilensteinen und den Befunden aus der Umsetzung
steht in [`docs/PLAN.md`](docs/PLAN.md).

WebStorm unterstützt Rust nicht — die Dateien unter `src-tauri/` werden nur als
Text angezeigt. Wer Rust bearbeiten will, öffnet den Ordner zusätzlich in
RustRover.
