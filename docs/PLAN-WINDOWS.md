# Projektplan: Windows-Client für Claude-Nutzung

Zweiter Plan, ergänzend zu [`PLAN.md`](PLAN.md). Der erste Plan beschreibt die
fertige macOS-App; dieser hier beschreibt, was ein Windows-Client zusätzlich
braucht. Stand der Vorabprüfung: **2026-09-24**. Es wurde noch nichts gebaut.

## Ziel und Abgrenzung

Eine Windows-Fassung derselben App: ein Symbol im Infobereich der Taskleiste,
das die Session-Auslastung zeigt, mit demselben Fenster, denselben Limits und
derselben Fehlerbehandlung.

**Nicht Ziel dieses Plans** ist die Entscheidung, ob das Ergebnis öffentlich
verbreitet wird. Dazu stehen unten Befunde und eine Empfehlung, aber die
Entscheidung ist offen und sollte bewusst getroffen werden.

## Ausgangslage

Die macOS-App ist fertig (M0–M6 in `PLAN.md`), 25 Unit-Tests grün, Quellcode
auf GitHub. Es gibt **keinen Windows-Rechner** zum Testen. Vorhanden ist
CrossOver 26.3 auf dem Mac — dazu unten ein eigener Abschnitt, weil es die
offene Kernfrage nur teilweise beantworten kann.

## Befunde der Vorabprüfung

### Was ohne Änderung portierbar ist

Der weit überwiegende Teil. `usage.rs` (Abruf, Normalisierung beider
Antwortformate, Statusbehandlung), das Datenmodell, der Backoff, `notify.rs`
und das komplette Frontend sind plattformneutral. Tauri 2, reqwest, serde,
chrono, `tauri-plugin-notification` und `tauri-plugin-autostart` unterstützen
Windows.

### Hürde 1: Der Tray-Titel existiert unter Windows nicht

Aus dem Tauri-Quelltext (`tauri-2.11.6/src/tray/mod.rs`, `set_title`):

```
/// ## Platform-specific:
///
/// - **Linux:** The title will not be shown unless there is an icon
///   as well. […]
/// - **Windows:** Unsupported
```

Das trifft den Kern der App. Die Windows-Taskleiste kennt kein Textlabel neben
dem Symbol, nur Icon und Tooltip. **Die Prozentzahl muss ins Icon gezeichnet
werden** — bei jedem Abruf ein frisch gerendertes Bitmap.

Damit entfällt auch `icon_as_template()`; laut Quelltext **macOS only**. Hell-
und Dunkeldarstellung der Taskleiste muss selbst behandelt werden.

Nebenbefund für einen späteren Linux-Port: Dort wird der Titel nur zusammen
mit einem Icon angezeigt und „may not be shown in all visualizations“ — auch
dort ist die Icon-Variante der verlässlichere Weg.

### Hürde 2: Der Ablageort der Zugangsdaten unter Windows ist unbekannt

Unter macOS liegt das Token ausschliesslich im Schlüsselbund; eine Datei
`~/.claude/.credentials.json` existiert auf dem Entwicklungsrechner **nicht**
(geprüft). Der Aufruf `/usr/bin/security` hat unter Windows kein Gegenstück.

In Frage kommen Windows Credential Manager, DPAPI oder eine Datei im
Benutzerprofil. **Das muss beobachtet werden, nicht geraten.** Solange es
offen ist, lässt sich W1 nicht abschliessen.

### Der Endpunkt ist eng limitiert (aus `PLAN.md` übernommen)

Messung vom 2026-09-24: ein **erfolgreicher** Abruf sperrt den Endpunkt für
mindestens 5 Minuten. Der Header `retry-after` kommt mit, hat aber den Wert
`0` und taugt nicht zur Steuerung. Das Limit gilt pro Konto — viele Nutzer
erzeugen also keine gemeinsame Last, aber jede Installation konkurriert mit
dem Claude Code desselben Nutzers. Intervall deshalb 10 Minuten, Backoff
additiv. Für Windows gilt das unverändert.

## Die Rolle von CrossOver — und seine Grenzen

CrossOver (Wine) kann zwei Dinge leisten und eines ausdrücklich nicht.

**Geeignet für:** einen Rauchtest der gebauten `.exe`. Startet sie, erscheint
das Symbol, öffnet sich das Fenster, stimmt die Darstellung grob. Das ersetzt
keinen echten Test, findet aber grobe Fehler früh.

**Bedingt geeignet für:** die Suche nach dem Ablageort der Zugangsdaten. Man
kann Claude Code für Windows unter CrossOver installieren und sich anmelden,
und wenn dabei eine Datei im Benutzerprofil entsteht, ist das ein starker
Hinweis.

**Nicht geeignet als Beweis:** Wine bildet Credential Manager und DPAPI eigen
nach. Legt Claude Code das Token dort ab, kann der Befund unter Wine anders
aussehen als unter echtem Windows — sowohl falsch positiv als auch falsch
negativ. **Jeder unter CrossOver gewonnene Befund ist eine Hypothese, die auf
echtem Windows zu bestätigen ist**, bevor Code darauf aufgebaut wird.

## Meilensteine

### W0: Ablageort der Zugangsdaten klären

**Ziel:** Es ist belegt, woher das Token unter Windows kommt.

- [ ] Claude Code für Windows unter CrossOver installieren und anmelden
- [ ] Benutzerprofil auf neue Dateien prüfen (`%USERPROFILE%\.claude\`,
      `%APPDATA%`, `%LOCALAPPDATA%`)
- [ ] Credential Manager im CrossOver-Container auf neue Einträge prüfen
- [ ] Befund festhalten: Pfad bzw. Eintragsname, Format, Feldnamen
- [ ] **Befund auf echtem Windows bestätigen** (fremder Rechner, VM, CI-Runner)

**Abnahme:** Der Pfad bzw. Eintrag ist benannt, das JSON-Feld für das Token
ist benannt, und die Bestätigung stammt nicht nur aus Wine. Ohne diesen
Meilenstein ist W1 blockiert — hier nicht weitermachen und raten.

---

### W1: `credentials.rs` plattformfähig machen

**Ziel:** Der Rest der App merkt nicht, auf welchem System sie läuft.

- [ ] Zugriff hinter ein Trait ziehen, etwa `TokenQuelle`
- [ ] macOS-Implementierung: der bestehende `security`-Aufruf, unverändert
- [ ] Windows-Implementierung nach dem Befund aus W0
- [ ] `Secret`-Typ und die Regel „nie loggen, nie ans Frontend“ bleiben
- [ ] Fehlermeldungen pro Plattform: der Hinweis auf die
      Schlüsselbundverwaltung ergibt unter Windows keinen Sinn
- [ ] Unit-Tests für die Auswertung des jeweiligen Formats, ohne echten Zugriff

**Abnahme:** `cargo test` grün auf macOS, die macOS-App verhält sich
unverändert, und der Windows-Zweig ist wenigstens kompilierbar
(`cargo check --target x86_64-pc-windows-msvc`).

---

### W2: Prozentzahl ins Tray-Icon zeichnen

**Ziel:** Unter Windows ist die Auslastung ohne Hovern ablesbar.

- [ ] Schriftrasterung, z. B. `ab_glyph`, mit eingebetteter Schrift unter
      freier Lizenz
- [ ] Icon zur Laufzeit erzeugen: 16 × 16 und 32 × 32, je hell und dunkel
- [ ] Sonderfälle: dreistellig (`100`), Fehlerzustand, veralteter Wert
      (auf macOS das Sternchen — im Icon braucht es eine eigene Form)
- [ ] Tooltip mit dem Langtext, da dort Platz ist
- [ ] Ergebnis über `tray.set_icon()` setzen statt `set_title()`
- [ ] macOS behält den Textweg; die Verzweigung über `#[cfg(target_os)]`

**Abnahme:** Unter CrossOver ist die Zahl im Infobereich lesbar, auch bei
100 % und im Fehlerfall. Auf macOS hat sich nichts geändert.

---

### W3: Übrige Plattformunterschiede

**Ziel:** Kein Verhalten, das nur auf einem System stimmt.

- [ ] `ActivationPolicy::Accessory` ist bereits `#[cfg(target_os = "macos")]`;
      unter Windows übernimmt `skipTaskbar: true` die Rolle — prüfen
- [ ] `tauri-plugin-autostart`: unter Windows der Registry-Run-Schlüssel. Der
      Parameter `MacosLauncher` bleibt in der Signatur, ist dort aber wirkungslos
- [ ] Popover-Position: der Positioner kennt `TrayCenter`, `TrayBottomCenter`,
      `TrayBottomLeft`, `TrayLeft`, `TrayRight`. Die Windows-Taskleiste sitzt
      meist unten rechts, das Fenster muss also **über** dem Symbol erscheinen —
      welche Variante das leistet, ist auszuprobieren
- [ ] Rechtsklick-Menü und Linksklick-Toggle unter Windows prüfen; die
      250-ms-Blur-Sperre aus M4 kann dort ein anderes Timing brauchen
- [ ] `decorations: false` und `alwaysOnTop` unter Windows gegenprüfen

**Abnahme:** Beide Plattformen verhalten sich gleich, wo es gleich sein soll,
und bewusst unterschiedlich, wo das System es verlangt.

---

### W4: Bauen und Ausliefern

**Ziel:** Es entsteht reproduzierbar eine Windows-Binärdatei.

- [ ] GitHub-Action mit `windows-latest`, die `.exe` bzw. `.msi` baut
- [ ] `bundle.targets` pro Plattform setzen (auf macOS steht es auf `["app"]`)
- [ ] Rauchtest unter CrossOver
- [ ] README um einen Windows-Abschnitt ergänzen, inklusive
      SmartScreen-Hinweis

**Abnahme:** Ein getaggter Commit erzeugt ohne Handarbeit eine Windows-Datei,
die unter CrossOver startet.

---

### W5: Entscheidung über die Verbreitung

**Kein Programmier-Meilenstein.** Vier Punkte, die vorher zu klären sind.

**Der Nutzerkreis ist klein.** Die App ist kein Dienst, sie liest lokale
Zugangsdaten. „Jeder“ heisst: wer Claude Code installiert hat und mit einem
**Abo** angemeldet ist, nicht mit API-Schlüssel. Für alle anderen zeigt sie
nur eine Fehlermeldung.

**Signierung kostet Geld.** Unsigniert blockt SmartScreen deutlich sichtbar,
macOS ebenso. Für den Eigengebrauch war das belanglos — einmal Rechtsklick.
Bei Fremden ist es die Hürde, an der die meisten abbrechen. Windows-Zertifikat
dreistellig pro Jahr, Apple 99 $.

**Der Endpunkt ist undokumentiert.** Ändert Anthropic ihn, brechen alle
Installationen gleichzeitig, und der Verbreiter ist der Ansprechpartner. Ob
die Nutzungsbedingungen etwas zum Ansprechen interner Endpunkte sagen, **ist
ungeprüft** und vor einer öffentlichen Verbreitung zu klären.

**Das Vertrauensproblem — der schwerste Punkt.** Eine Binärdatei, die das
OAuth-Token aus dem Anmeldespeicher extrahiert und an einen Server schickt,
hat exakt die Form von Schadsoftware, die Zugangsdaten stiehlt. Dieser Code
tut das nachweislich nicht — Token nur im Speicher, nie geloggt, nie ans
Frontend. Einem heruntergeladenen `.exe` sieht man das aber nicht an, und es
gibt keinen Weg, es jemandem zu beweisen.

**Empfehlung:** Verbreitung als **Quellcode mit Bauanleitung**, nicht als
fertige Binärdatei. Das umgeht die Zertifikatskosten, und vor allem: wer
selbst baut, kann `credentials.rs` vorher lesen. Bei einem Werkzeug, dessen
Kern der Zugriff auf fremde Zugangsdaten ist, wiegt Nachprüfbarkeit schwerer
als Bequemlichkeit. Wenn es doch fertige Dateien sein sollen, ist der
ehrlichste Zwischenweg ein öffentlich nachvollziehbarer CI-Build aus einem
getaggten Commit — dann ist wenigstens belegbar, aus welchem Quellcode die
Datei stammt.

---

## Aufwand

Grobe Schätzung, **sobald W0 geklärt ist**: drei bis fünf Tage für W1 bis W4.
W0 selbst ist nicht schätzbar — es hängt davon ab, ob sich der Ablageort unter
CrossOver zeigt und wie schnell sich ein echter Windows-Rechner zur
Bestätigung findet.

## Risiken

| Risiko | Umgang |
|---|---|
| Wine-Befund weicht von echtem Windows ab | W0 gilt erst als erledigt, wenn ein echter Rechner bestätigt hat |
| Claude Code für Windows läuft nicht unter CrossOver | Dann fällt W0 auf einen fremden Rechner oder eine VM zurück; ohne Befund nicht weiterbauen |
| Zahl im 16×16-Icon unlesbar | Schmale Schrift, notfalls nur die Zehnerstelle plus Farbcodierung; Langtext im Tooltip |
| Endpunkt ändert sich | Wie in `PLAN.md`: beide Formate unterstützen, letzten Wert behalten, klare Meldung |
| Verbreitung ohne geklärte Nutzungsbedingungen | W5 vor der Veröffentlichung abschliessen, nicht danach |

## Hinweise für die Umsetzung

- Reihenfolge einhalten: **W0 blockiert alles Übrige.** Ohne belegten
  Ablageort führt W1 zu Code, der auf einer Vermutung steht.
- Wie im ersten Plan: ein Meilenstein pro Durchgang, danach Abnahme, danach
  Commit.
- Die Sicherheitsregeln aus `PLAN.md` gelten unverändert, besonders: Token nur
  im Speicher, nie loggen, nie ins Repository.
