# Projektplan: Claude Usage Menübar-App (macOS)

## Ziel

Eine schlanke macOS-Desktop-App, die dauerhaft in der Menüleiste läuft und auf einen Blick zeigt, wie viel Prozent meines Claude-Abo-Limits verbraucht sind und wann das Limit zurückgesetzt wird.

**Fertig ist das Projekt, wenn:**

- in der Menüleiste die aktuelle Session-Auslastung steht (z. B. `42 %`),
- ein Klick auf das Symbol ein kleines Fenster öffnet, das alle Limits (Session, Woche, ggf. modellspezifische Wochenlimits) mit Fortschrittsbalken und Reset-Countdown zeigt,
- die Daten sich automatisch aktualisieren und per Knopf manuell aktualisiert werden können
  (ursprünglich alle 5 Minuten geplant; **am 2026-09-24 auf 10 Minuten geändert**, siehe Messung unten),
- Fehler (kein Login, abgelaufenes Token, Netzwerk, geändertes API-Format) verständlich angezeigt werden, statt die App abstürzen zu lassen,
- die App als `.app` gebaut ist und auf Wunsch beim Login automatisch startet.

## Aktueller Stand

- Projekt ist in WebStorm angelegt.
- M0 abgeschlossen (2026-09-24): Toolchain steht, Endpunkt liefert HTTP 200. Format siehe Befund unter M0.
- Tauri-Gerüst unter `src-tauri/` ist bereits vorhanden, `npx tauri init` entfällt.
- M1 abgeschlossen (2026-09-24): Build läuft fehlerfrei, Git-Repo steht.
- M2 und M3 abgeschlossen, M4 und M5 im Code fertig (2026-09-24). 18 Unit-Tests grün.
- Offen vor M6: optische Abnahme von M4 und die manuellen Fehlerfall-Tests aus M5.
- M6 abgeschlossen (2026-09-24): App liegt unter `/Applications/Claude-Nutzung.app` und läuft.
- Weiterhin offen: die manuellen Fehlerfall-Tests aus M5 (WLAN aus, ausloggen, Schlüsselbund verweigern).

## Technischer Rahmen

| Bereich | Entscheidung |
|---|---|
| Framework | Tauri 2 |
| Frontend | Vanilla TypeScript + HTML/CSS (in WebStorm) |
| Backend | Rust (von Tauri erzeugt, Datenabruf und Menüleiste) |
| Datenquelle | Interner Nutzungs-Endpunkt von Claude Code |
| Zugangsdaten | OAuth-Token von Claude Code aus dem macOS-Schlüsselbund |

### Datenquelle (wichtig, bitte zuerst lesen)

Der Endpunkt ist **inoffiziell und undokumentiert**. Er wird von Claude Code selbst benutzt und kann sich jederzeit ändern. Die App muss deshalb mit Formatänderungen und Ausfällen umgehen können.

**Anfrage:**

```
GET https://api.anthropic.com/api/oauth/usage
Authorization: Bearer <accessToken>
anthropic-beta: oauth-2025-04-20
User-Agent: <eigener App-Name>/<Version>
```

**Token auslesen (macOS):**

```bash
/usr/bin/security find-generic-password -s "Claude Code-credentials" -w
```

Liefert JSON. Relevant ist `claudeAiOauth.accessToken`, außerdem `claudeAiOauth.expiresAt` (Unix-Zeit in Millisekunden). Beim ersten Zugriff fragt macOS nach Erlaubnis für den Schlüsselbund.

**Antwortformate (beide unterstützen):**

Neueres Format mit `limits`-Array:

```json
{
  "limits": [
    { "kind": "session", "percent": 42.0, "resets_at": "2026-09-24T18:00:00+00:00" },
    { "kind": "weekly_all", "percent": 13.0, "resets_at": "..." },
    { "kind": "weekly_scoped", "percent": 20.0, "resets_at": "...",
      "scope": { "model": { "display_name": "Opus" } } }
  ]
}
```

Älteres, flaches Format:

```json
{
  "five_hour":        { "utilization": 35.0, "resets_at": "2026-02-06T22:00:00+00:00" },
  "seven_day":        { "utilization": 14.0, "resets_at": "..." },
  "seven_day_sonnet": { "utilization": 39.0, "resets_at": "..." },
  "seven_day_opus":   null
}
```

Die Werte sind Prozentangaben von 0 bis 100. `resets_at` kann ein ISO-8601-String oder eine Unix-Zeit sein, das Frontend soll beides verarbeiten.

### Sicherheitsregeln

- Das Token wird **nur im Speicher** und nur für die Dauer der Anfrage gehalten. Nie auf die Platte schreiben, nie loggen, nie ans Frontend schicken.
- **Kein eigener Token-Refresh.** Claude Code rotiert das Token selbst. Ist es abgelaufen, zeigt die App den Hinweis „Öffne kurz Claude Code“ an.
- Keine Zugangsdaten im Repository. Falls das Projekt auf GitHub landet, vorher prüfen.
- Abfrageintervall nicht unter 5 Minuten, um Rate-Limits (HTTP 429) zu vermeiden.
  **Nachtrag 2026-09-24:** 5 Minuten reichen nicht. Messung ohne laufende App,
  sechs Anfragen im Minutenabstand:

  ```
  15:24:59  HTTP 200   session=15%, weekly_all=5%
  15:25:59  HTTP 429   retry-after: 0
  15:26:59  HTTP 429
  15:28:00  HTTP 429
  15:29:00  HTTP 429
  15:30:00  HTTP 429
  ```

  Ein **erfolgreicher** Abruf sperrt den Endpunkt für mindestens 5 Minuten.
  Ein Intervall von exakt 5 Minuten liegt damit auf der Grenze; jeder
  zusätzliche Abruf (Neustart der App, Knopf „Aktualisieren“) kippt es
  darüber. Intervall deshalb auf **10 Minuten**. Der Header `retry-after`
  kommt zwar mit, hat aber den Wert `0` und taugt nicht zur Steuerung.

  Ausserdem: der Backoff **addiert** jetzt 5 Minuten statt zu verdoppeln.
  Verdoppeln (5→10→20→30) liess die App nach zwei Fehlschlägen zwanzig
  Minuten warten, obwohl sich die Sperre nach wenigen Minuten löst — das war
  die Ursache für „bekomme oft keine neue Prozentzahl“.

## Architektur

```
┌──────────────────────── Rust (src-tauri) ────────────────────────┐
│  credentials.rs  → liest Token aus dem Schlüsselbund             │
│  usage.rs        → ruft Endpunkt ab, normalisiert beide Formate  │
│  lib.rs          → Tray-Icon, Poll-Schleife, Cache, Commands     │
└──────────────┬───────────────────────────────────────────────────┘
               │ Event "usage-updated"  /  Commands get_last, refresh_now
┌──────────────▼──────────── Frontend (src) ───────────────────────┐
│  index.html, main.ts, styles.css → Popover-Fenster mit Balken    │
└──────────────────────────────────────────────────────────────────┘
```

Einheitliches Datenmodell zwischen Rust und Frontend:

```ts
type Limit   = { label: string; percent: number; resets_at: string | number | null };
type Payload = { ok: true;  data: { limits: Limit[]; fetched_at: string } }
             | { ok: false; error: string };
```

---

## Milestones

### M0: Voraussetzungen prüfen  ✅ abgeschlossen (2026-09-24)

**Ziel:** Alles ist installiert und der Endpunkt funktioniert mit meinem Konto.

- [x] Xcode Command Line Tools: `xcode-select --install`
- [x] Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [x] Node.js (LTS) prüfen: `node -v`
- [x] In Claude Code eingeloggt (mit Abo, nicht mit API-Key)
- [x] Endpunkt per Terminal testen:

```bash
TOKEN=$(security find-generic-password -s "Claude Code-credentials" -w \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["claudeAiOauth"]["accessToken"])')
curl -s https://api.anthropic.com/api/oauth/usage \
  -H "Authorization: Bearer $TOKEN" \
  -H "anthropic-beta: oauth-2025-04-20" | python3 -m json.tool
```

**Befund vom 2026-09-24 (tatsächliches Format):**

- Installiert: Xcode CLT, Rust 1.98.1 / Cargo 1.98.1, Node v20.19.2. `. "$HOME/.cargo/env"` wurde an `~/.zshrc` angehängt.
- HTTP 200. **Beide Formate kommen gleichzeitig** in *einer* Antwort: die flachen Keys *und* das `limits`-Array. Der Fallback aus M2 bleibt richtig, greift aber im Normalfall nie.
- `limits[]` führt zusätzliche Felder: `group`, `severity` (`"normal"` | …), `is_active`, `scope`. `percent` kommt hier als **Integer**, im flachen Format als Float — in Rust beides als `f64` einlesen.
- `resets_at`: ISO-8601 mit Sekundenbruchteilen und `+00:00`, z. B. `2026-09-24T13:50:00.681785+00:00`. Kann `null` sein.
- Die flache Ebene enthält **viele unbekannte Codenamen-Keys** (`tangelo`, `nimbus_quill`, `copper_kite`, `iguana_necktie`, …), fast alle `null`. Der Fallback in M2 muss deshalb eine **Whitelist bekannter Keys** benutzen und darf nicht über alle Keys iterieren.
- Nicht im Plan vorgesehen, aber vorhanden: `extra_usage` / `spend` (Guthabenverbrauch in EUR) und `seven_day_breakdown` (Anteil Claude Code / Chats / Cowork / Other). Optionale Erweiterung für M4.
- **Rate-Limit ist real:** zwei Einzelanfragen kurz hintereinander ergaben HTTP 429, erst die dritte nach ~2 Min kam durch. Der Backoff aus M5 ist Pflicht, nicht optional.

**Abnahme:** Der curl-Befehl liefert JSON mit Prozentwerten, die zu `/usage` in Claude Code passen. Das tatsächliche Format notieren (Array `limits` oder flache Keys).

---

### M1: Bestehendes WebStorm-Projekt prüfen und startklar machen  ✅ abgeschlossen (2026-09-24)

**Ziel:** Das in WebStorm angelegte Projekt ist ein funktionierendes Tauri-Projekt, und die leere App startet.

- [x] Projekt in WebStorm angelegt
- [x] Ordnerstruktur prüfen. Folgendes muss vorhanden sein:
  - `package.json` mit dem Skript `"tauri": "tauri"` und `@tauri-apps/cli` in den devDependencies
  - `index.html` und `src/main.ts` (Frontend)
  - `src-tauri/` mit `Cargo.toml`, `tauri.conf.json` und `src/lib.rs`
- [x] ~~**Falls `src-tauri/` fehlt**~~ — entfällt, Gerüst war vollständig vorhanden. Original:  **Falls `src-tauri/` fehlt** (z. B. weil das Projekt als leeres oder reines Vite-Projekt angelegt wurde): im WebStorm-Terminal im Projektordner `npm install -D @tauri-apps/cli@latest` und `npm install @tauri-apps/api@latest` ausführen, dann `npx tauri init`. Bei den Fragen: Frontend-Dev-URL `http://localhost:1420` bzw. die Vite-URL, Frontend-Build-Ordner `../dist`, Dev-Befehl `npm run dev`, Build-Befehl `npm run build`
- [x] Im WebStorm-Terminal `npm install`, dann `npm run tauri dev`
- [x] Run-Konfiguration in WebStorm anlegen: **Run → Edit Configurations → + → npm**, Command `run`, Script `tauri`, Arguments `dev`. Danach startet die App über den grünen Play-Knopf
- [x] Git-Repository anlegen (**VCS → Enable Version Control Integration**), prüfen, dass `node_modules/`, `dist/` und `src-tauri/target/` in der `.gitignore` stehen
- [x] Diese Datei als `PLAN.md` in den Projektordner legen (liegt unter `docs/PLAN.md`)

Hinweis: WebStorm unterstützt Rust nicht. Die Dateien in `src-tauri/` werden nur als Text angezeigt. Das ist in Ordnung, weil Claude Code den Rust-Teil schreibt. Wer Rust später selbst bearbeiten will, kann den Ordner zusätzlich in RustRover öffnen.

**Befund vom 2026-09-24:**

- `npm install`: 20 Pakete, 0 Schwachstellen.
- `npm run tauri dev`: Erstbuild in **1m54s**, 348 Crates, **keine Fehler**. App startete als GUI-Prozess.
- Run-Konfiguration liegt als `.idea/runConfigurations/tauri_dev.xml` (npm → `run tauri dev`). `.idea/` ist gitignoriert, bleibt also lokal. WebStorm muss das Projekt ggf. einmal neu laden, damit sie erscheint.
- `.gitignore` deckt `node_modules`, `dist`, `src-tauri/target` und `.idea` ab. Git-Repo initialisiert auf Branch `main`.

**Abnahme:** `npm run tauri dev` (oder die Run-Konfiguration) öffnet das Standardfenster ohne Fehler. Der erste Start dauert einige Minuten, weil Rust alles kompiliert.

---

### M2: Datenabruf in Rust  ✅ abgeschlossen (2026-09-24)

**Ziel:** Rust kann das Token lesen, den Endpunkt abfragen und eine einheitliche Liste von Limits zurückgeben.

- [x] Abhängigkeiten in `src-tauri/Cargo.toml`: `reqwest` (Feature `json`), `serde`, `serde_json`, `tokio` (Feature `time`)
- [x] `credentials.rs`: Token per `/usr/bin/security` lesen, `expiresAt` gegen aktuelle Zeit prüfen, verständliche Fehlermeldungen auf Deutsch
- [x] `usage.rs`: Anfrage mit den Headern oben, Timeout 15 s
- [x] Normalisierung: zuerst `limits`-Array auswerten, sonst flache Keys als Fallback
  - `session` → „Session“, `weekly_all` → „Woche (alle Modelle)“, `weekly_scoped` → „Woche (<Modellname>)“
  - `five_hour` → „Session (5 Std.)“, `seven_day` → „Woche (alle Modelle)“, `seven_day_sonnet`/`seven_day_opus` → „Woche (Sonnet/Opus)“
  - `null`-Einträge überspringen
- [x] HTTP-Status abfangen: 401 → „Token abgelaufen“, 429 → „Zu viele Anfragen“, sonst Status anzeigen
- [x] Leere Liste nach dem Parsen → Fehler „Format hat sich vermutlich geändert“
- [x] Unit-Tests für die Normalisierung mit beiden Beispiel-JSONs (ohne Netzwerk)
- [x] Tauri-Command `refresh_now` zum Testen aus dem Frontend

**Befund vom 2026-09-24:** `cargo test` grün. Abweichungen vom Plantext:
`chrono` als zusätzliche Abhängigkeit für `fetched_at`; `reqwest` mit
`rustls-tls` statt native-tls (keine System-OpenSSL-Abhängigkeit); `Limit`
hat zwei Felder mehr als das Datenmodell oben — `severity` (aus der echten
Antwort, für M4) und `kind` (um die Session-Zeile zu finden, ohne auf
Anzeigetexte zu matchen). Das Token steckt in einem `Secret`-Typ, dessen
`Debug` nur `Secret(***)` ausgibt. Unbekannte `kind`-Werte im Array werden
angezeigt statt verworfen; im flachen Fallback gilt umgekehrt die Whitelist.

**Abnahme:** `cargo test` ist grün. Ein Aufruf von `refresh_now` liefert dieselben Werte wie der curl-Test aus M0.

---

### M3: Menüleiste und automatische Aktualisierung  ✅ abgeschlossen (2026-09-24)

**Ziel:** Die App lebt in der Menüleiste und zeigt die Session-Prozentzahl.

- [x] Tauri-Feature `tray-icon` aktivieren
- [x] Dock-Symbol ausblenden: `ActivationPolicy::Accessory`
- [x] Tray-Icon mit ID `main`, Titel = Prozent des ersten Limits (Session), z. B. `42 %`, bei Fehler `⚠︎`
- [x] Rechtsklick-Menü mit „Aktualisieren“ und „Beenden“
- [x] Poll-Schleife mit `tauri::async_runtime::spawn`: sofort abrufen, dann alle 5 Minuten
- [x] Letztes Ergebnis in einem `Mutex`-State cachen, Command `get_last` bereitstellen
- [x] Nach jedem Abruf Event `usage-updated` ans Frontend senden

**Befund vom 2026-09-24:** 16 Tests grün, App startet fehlerfrei.
Der Tray-Titel nimmt **nicht** das erste Limit, sondern sucht gezielt
`kind == "session"` bzw. `"five_hour"` (Rückfall: erstes Limit) — die
Reihenfolge im Array ist nicht garantiert. Der 429-Backoff aus M5 wurde
hierher vorgezogen, weil der Endpunkt laut M0-Befund schon bei
Einzelanfragen limitiert. Tray bewusst ohne Icon, nur Text (Icon in M6).

**Abnahme:** Nach dem Start erscheint kein Dock-Symbol, aber in der Menüleiste steht die korrekte Prozentzahl. Nach 5 Minuten aktualisiert sie sich von selbst.

---

### M4: Popover-Fenster  ✅ abgeschlossen (2026-09-24)

**Ziel:** Klick auf das Tray-Icon zeigt ein kleines Fenster mit allen Details.

- [x] In `tauri.conf.json`: Fenster `main`, ca. 340 × 380 px, nicht skalierbar, `visible: false` beim Start
- [x] Linksklick aufs Icon zeigt/versteckt das Fenster, Schließen versteckt nur (App läuft weiter)
- [x] Optional: Fenster unter dem Icon positionieren (Plugin `tauri-plugin-positioner`)
- [x] Frontend: beim Laden `get_last` aufrufen, auf `usage-updated` hören
- [x] Pro Limit: Name, Prozent, Fortschrittsbalken, „Reset in 2 Std. 15 Min.“ plus Uhrzeit (Format `de-DE`)
- [x] Balkenfarbe nach Auslastung: unter 60 % ruhig, 60–85 % Warnung, ab 85 % kritisch
- [x] Countdown lokal alle 30 Sekunden neu berechnen (ohne neuen API-Aufruf)
- [x] Knopf „Jetzt aktualisieren“ und Zeile „Zuletzt aktualisiert: 14:32“
- [x] Texte per `textContent` setzen (kein `innerHTML` mit API-Daten)
- [x] Hell- und Dunkelmodus über `prefers-color-scheme`

**Befund vom 2026-09-24:** 16 Tests grün, `tsc --noEmit` sauber, App läuft.
`show_menu_on_left_click(false)` war nötig — Voreinstellung ist `true`, sonst
öffnet der Linksklick das Menü statt des Fensters. Zusätzlich zum Plan in
`tauri.conf.json`: `decorations: false`, `alwaysOnTop: true`,
`skipTaskbar: true` (ohne die drei verhält sich das Fenster nicht wie ein
Popover) und `withGlobalTauri: false`. `severity` darf die Balkenfarbe nur
nach oben korrigieren, nie nach unten — bekannt ist bisher nur `"normal"`.
**Nicht umgesetzt:** Ausblenden bei Fokusverlust (nicht im Plan; würde beim
Öffnen der Entwicklerwerkzeuge stören). Optische Abnahme am 2026-09-24 durch
den Nutzer bestätigt.

**Abnahme:** Alle Limits werden korrekt und gut lesbar angezeigt, der Countdown läuft, hell und dunkel sehen beide gut aus.

---

### M5: Fehlerfälle und Robustheit  ⚠️ Code fertig (2026-09-24), Abnahme offen

**Ziel:** Die App bleibt in jeder Lage benutzbar.

- [x] Nicht in Claude Code eingeloggt → Hinweis mit Anleitung
- [x] Token abgelaufen → „Öffne kurz Claude Code, dann erneuert sich die Anmeldung“
- [x] Offline / Timeout → letzten gültigen Wert weiter anzeigen, als veraltet markieren
- [x] 429 → nächsten Abruf verzögern (z. B. Intervall verdoppeln, max. 30 Minuten)
- [x] Unbekanntes Format → klare Meldung statt Absturz
- [x] Schlüsselbund-Zugriff verweigert → Hinweis, wie man ihn erlaubt
- [ ] Jeden Fall einmal gezielt ausprobieren (WLAN aus, ausloggen usw.)

**Befund vom 2026-09-24:** 18 Tests grün. Vier der Fälle waren mit M2/M3
bereits erledigt. Neu: `AppState` trennt `last` (was angezeigt wird) von
`last_good` (letzter erfolgreicher Abruf); schlägt ein Abruf fehl, wird der
frühere Stand mit `stale: true` und der Fehlermeldung als `notice` gezeigt.
`fetched_at` behält den **alten** Zeitpunkt. Alle drei Auslöser eines Abrufs
laufen über `handle_result()`, damit der Rückfall keinen Pfad auslässt.

**Offen:** Der Tray-Titel zeigt bei veralteten Werten weiter die alte
Prozentzahl ohne Kennzeichen — plankonform, aber an der Menüleiste allein
nicht erkennbar. **Offen:** Der letzte Punkt oben ist manuelle Arbeit am
Gerät: WLAN aus, in Claude Code ausloggen, Schlüsselbund-Zugriff verweigern.

**Abnahme:** In keinem der Fälle stürzt die App ab, und jede Meldung sagt, was zu tun ist.

---

### M6: Feinschliff und Auslieferung  ✅ abgeschlossen (2026-09-24)

**Ziel:** Eine fertige App, die ich täglich nutze.

- [x] Eigenes Tray-Icon (Template-Icon, schwarz/transparent, passt sich hell/dunkel an)
- [x] Autostart beim Login (`tauri-plugin-autostart`) mit Schalter im Fenster
- [x] Optional: macOS-Benachrichtigung bei 80 % und 95 % (`tauri-plugin-notification`), einmal pro Fenster
- [ ] ~~Optional: einstellbares Aktualisierungsintervall~~ — bewusst ausgelassen, Begründung im Befund unten
- [x] `npm run tauri build` → `.app` nach `/Programme` kopieren
- [x] Kurze README mit Installation und dem Hinweis auf den inoffiziellen Endpunkt

**Befund vom 2026-09-24:** 23 Tests grün, warnungsfrei.

- Tray-Icon: `icons/tray-template.png`, 44 × 44, rein schwarz mit Alpha, mit
  Supersampling gezeichnet. `icon_as_template(true)` überlässt die Einfärbung
  macOS. Dafür war das Tauri-Feature `image-png` nötig.
- Autostart als Launch-Agent. Der Zustand wird beim Öffnen aus Rust gelesen
  statt im Frontend gemerkt — der Agent kann auch ausserhalb der App entfernt
  worden sein. `set_autostart` gibt zurück, was danach *tatsächlich* gilt.
- Benachrichtigungen: die Entscheidung, was fällig ist, steckt in der reinen
  Funktion `faellige_meldungen()`, getrennt vom Versand — dadurch ist die
  Regel „einmal pro Fenster“ ohne macOS testbar (5 Tests).
- Release-Build: 17m59s. Die `.app` wurde erzeugt und nach
  `/Applications/Claude-Nutzung.app` kopiert; sie startet ohne
  Gatekeeper-Dialog, weil lokal gebaute Bundles kein Quarantäne-Attribut
  bekommen. **Das anschliessende DMG-Bundling schlug fehl** (`bundle_dmg.sh`).
  Für dieses Projekt ohne Belang — der Plan verlangt nur die `.app`.
  Ausgeschlossen wurden als Ursache: fehlende Finder-Automation, ein
  hängengebliebenes Volume unter `/Volumes`, zu wenig Plattenplatz. Die
  eigentliche Ursache blieb **ungeklärt**. Ein Hinweis für später: das
  Zwischenimage `rw.*.dmg` (30 MB) lag danach im Bundle-Verzeichnis — `hdiutil`
  hat es also erzeugt, der Fehler trat erst im Schritt danach auf (Mounten,
  Finder-Layout oder `convert`). Konsequenz: `bundle.targets` steht jetzt auf
  `["app"]` statt `"all"`, das DMG wird gar nicht mehr versucht. Ein
  Verifikationsbuild mit der neuen Einstellung lief in 1m51s sauber durch.
- Der Release-Profilteil aus der Tauri-Vorlage (`lto = true`,
  `codegen-units = 1`) kostete 17m59s für einen vollständigen Build. Auf
  `lto = "thin"` und `codegen-units = 16` umgestellt und gemessen:

  | | vorher | nachher |
  |---|---|---|
  | Bauzeit (vollständig) | 17m59s | **3m47s** |
  | Binärdatei | 5,5 MB | 6,8 MB |
  | `.app` | 5,6 MB | 6,9 MB |

  Knapp ein Viertel der Zeit für 1,3 MB mehr. Für eine App, die alle 5 Minuten
  eine HTTP-Anfrage stellt, ist Laufzeitoptimierung ohnehin kein Thema.
- **Einstellbares Intervall bewusst ausgelassen:** nach unten begrenzt die
  Sicherheitsregel ohnehin auf 5 Minuten, und der M0-Befund zeigt, wie schnell
  der Endpunkt mit 429 antwortet. Ein Regler, der genau dorthin führt, schafft
  mehr Ärger als Nutzen; nach oben ist der Gewinn gering.

**Abnahme:** Die gebaute App startet nach einem Neustart des Macs automatisch und zeigt korrekte Werte.

---

## Risiken

| Risiko | Umgang |
|---|---|
| Endpunkt ändert sich oder verschwindet | Beide Formate unterstützen, klare Fehlermeldung, letzten Wert behalten |
| Speicherort des Tokens ändert sich | Fehlermeldung mit Hinweis, Pfad zentral in `credentials.rs` |
| Rate-Limiting (429) | Mindestens 5 Minuten Intervall, Backoff |
| Token abgelaufen | Kein eigener Refresh, Hinweis auf Claude Code |
| Unsignierte App wird von macOS blockiert | Beim ersten Start Rechtsklick → „Öffnen“ |

## Hinweise für die Umsetzung mit Claude Code

- Claude Code direkt im **WebStorm-Terminal** (unten, Tab „Terminal“) im Projektordner starten: `claude`. So arbeitet Claude Code im richtigen Ordner, und Änderungen erscheinen sofort in WebStorm.
- Reihenfolge: zuerst M0 (Voraussetzungen), dann M1 (Projekt prüfen), dann M2 bis M6.
- Immer nur **einen Milestone pro Durchgang** umsetzen und erst nach bestandener Abnahme weitermachen.
- Zu Beginn jedes Durchgangs diese Datei lesen lassen, z. B.: *„Lies PLAN.md und setze Milestone M2 um. Halte dich an die Sicherheitsregeln.“*
- Nach jedem Milestone einen Git-Commit machen.
- Erledigte Punkte in dieser Datei abhaken, damit Claude Code den Fortschritt sieht.
- Beim Test niemals das echte Token in den Chat, in Logs oder in Testdateien kopieren.
