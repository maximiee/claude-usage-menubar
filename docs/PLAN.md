# Projektplan: Claude Usage Menübar-App (macOS)

## Ziel

Eine schlanke macOS-Desktop-App, die dauerhaft in der Menüleiste läuft und auf einen Blick zeigt, wie viel Prozent meines Claude-Abo-Limits verbraucht sind und wann das Limit zurückgesetzt wird.

**Fertig ist das Projekt, wenn:**

- in der Menüleiste die aktuelle Session-Auslastung steht (z. B. `42 %`),
- ein Klick auf das Symbol ein kleines Fenster öffnet, das alle Limits (Session, Woche, ggf. modellspezifische Wochenlimits) mit Fortschrittsbalken und Reset-Countdown zeigt,
- die Daten sich automatisch alle 5 Minuten aktualisieren und per Knopf manuell aktualisiert werden können,
- Fehler (kein Login, abgelaufenes Token, Netzwerk, geändertes API-Format) verständlich angezeigt werden, statt die App abstürzen zu lassen,
- die App als `.app` gebaut ist und auf Wunsch beim Login automatisch startet.

## Aktueller Stand

- Projekt ist in WebStorm angelegt.
- M0 abgeschlossen (2026-09-24): Toolchain steht, Endpunkt liefert HTTP 200. Format siehe Befund unter M0.
- Tauri-Gerüst unter `src-tauri/` ist bereits vorhanden, `npx tauri init` entfällt.
- M1 abgeschlossen (2026-09-24): Build läuft fehlerfrei, Git-Repo steht.
- Nächster Schritt: M2 (Datenabruf in Rust).

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

### M2: Datenabruf in Rust

**Ziel:** Rust kann das Token lesen, den Endpunkt abfragen und eine einheitliche Liste von Limits zurückgeben.

- [ ] Abhängigkeiten in `src-tauri/Cargo.toml`: `reqwest` (Feature `json`), `serde`, `serde_json`, `tokio` (Feature `time`)
- [ ] `credentials.rs`: Token per `/usr/bin/security` lesen, `expiresAt` gegen aktuelle Zeit prüfen, verständliche Fehlermeldungen auf Deutsch
- [ ] `usage.rs`: Anfrage mit den Headern oben, Timeout 15 s
- [ ] Normalisierung: zuerst `limits`-Array auswerten, sonst flache Keys als Fallback
  - `session` → „Session“, `weekly_all` → „Woche (alle Modelle)“, `weekly_scoped` → „Woche (<Modellname>)“
  - `five_hour` → „Session (5 Std.)“, `seven_day` → „Woche (alle Modelle)“, `seven_day_sonnet`/`seven_day_opus` → „Woche (Sonnet/Opus)“
  - `null`-Einträge überspringen
- [ ] HTTP-Status abfangen: 401 → „Token abgelaufen“, 429 → „Zu viele Anfragen“, sonst Status anzeigen
- [ ] Leere Liste nach dem Parsen → Fehler „Format hat sich vermutlich geändert“
- [ ] Unit-Tests für die Normalisierung mit beiden Beispiel-JSONs (ohne Netzwerk)
- [ ] Tauri-Command `refresh_now` zum Testen aus dem Frontend

**Abnahme:** `cargo test` ist grün. Ein Aufruf von `refresh_now` liefert dieselben Werte wie der curl-Test aus M0.

---

### M3: Menüleiste und automatische Aktualisierung

**Ziel:** Die App lebt in der Menüleiste und zeigt die Session-Prozentzahl.

- [ ] Tauri-Feature `tray-icon` aktivieren
- [ ] Dock-Symbol ausblenden: `ActivationPolicy::Accessory`
- [ ] Tray-Icon mit ID `main`, Titel = Prozent des ersten Limits (Session), z. B. `42 %`, bei Fehler `⚠︎`
- [ ] Rechtsklick-Menü mit „Aktualisieren“ und „Beenden“
- [ ] Poll-Schleife mit `tauri::async_runtime::spawn`: sofort abrufen, dann alle 5 Minuten
- [ ] Letztes Ergebnis in einem `Mutex`-State cachen, Command `get_last` bereitstellen
- [ ] Nach jedem Abruf Event `usage-updated` ans Frontend senden

**Abnahme:** Nach dem Start erscheint kein Dock-Symbol, aber in der Menüleiste steht die korrekte Prozentzahl. Nach 5 Minuten aktualisiert sie sich von selbst.

---

### M4: Popover-Fenster

**Ziel:** Klick auf das Tray-Icon zeigt ein kleines Fenster mit allen Details.

- [ ] In `tauri.conf.json`: Fenster `main`, ca. 340 × 380 px, nicht skalierbar, `visible: false` beim Start
- [ ] Linksklick aufs Icon zeigt/versteckt das Fenster, Schließen versteckt nur (App läuft weiter)
- [ ] Optional: Fenster unter dem Icon positionieren (Plugin `tauri-plugin-positioner`)
- [ ] Frontend: beim Laden `get_last` aufrufen, auf `usage-updated` hören
- [ ] Pro Limit: Name, Prozent, Fortschrittsbalken, „Reset in 2 Std. 15 Min.“ plus Uhrzeit (Format `de-DE`)
- [ ] Balkenfarbe nach Auslastung: unter 60 % ruhig, 60–85 % Warnung, ab 85 % kritisch
- [ ] Countdown lokal alle 30 Sekunden neu berechnen (ohne neuen API-Aufruf)
- [ ] Knopf „Jetzt aktualisieren“ und Zeile „Zuletzt aktualisiert: 14:32“
- [ ] Texte per `textContent` setzen (kein `innerHTML` mit API-Daten)
- [ ] Hell- und Dunkelmodus über `prefers-color-scheme`

**Abnahme:** Alle Limits werden korrekt und gut lesbar angezeigt, der Countdown läuft, hell und dunkel sehen beide gut aus.

---

### M5: Fehlerfälle und Robustheit

**Ziel:** Die App bleibt in jeder Lage benutzbar.

- [ ] Nicht in Claude Code eingeloggt → Hinweis mit Anleitung
- [ ] Token abgelaufen → „Öffne kurz Claude Code, dann erneuert sich die Anmeldung“
- [ ] Offline / Timeout → letzten gültigen Wert weiter anzeigen, als veraltet markieren
- [ ] 429 → nächsten Abruf verzögern (z. B. Intervall verdoppeln, max. 30 Minuten)
- [ ] Unbekanntes Format → klare Meldung statt Absturz
- [ ] Schlüsselbund-Zugriff verweigert → Hinweis, wie man ihn erlaubt
- [ ] Jeden Fall einmal gezielt ausprobieren (WLAN aus, ausloggen usw.)

**Abnahme:** In keinem der Fälle stürzt die App ab, und jede Meldung sagt, was zu tun ist.

---

### M6: Feinschliff und Auslieferung

**Ziel:** Eine fertige App, die ich täglich nutze.

- [ ] Eigenes Tray-Icon (Template-Icon, schwarz/transparent, passt sich hell/dunkel an)
- [ ] Autostart beim Login (`tauri-plugin-autostart`) mit Schalter im Fenster
- [ ] Optional: macOS-Benachrichtigung bei 80 % und 95 % (`tauri-plugin-notification`), einmal pro Fenster
- [ ] Optional: einstellbares Aktualisierungsintervall
- [ ] `npm run tauri build` → `.app` nach `/Programme` kopieren
- [ ] Kurze README mit Installation und dem Hinweis auf den inoffiziellen Endpunkt

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
