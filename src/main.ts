import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Muss zum Rust-Modell in src-tauri/src/usage.rs passen.
type ResetsAt = string | number | null;

type Limit = {
  kind: string;
  label: string;
  percent: number;
  resets_at: ResetsAt;
  severity: string | null;
};

type UsageData = {
  limits: Limit[];
  fetched_at: string;
  stale: boolean;
  notice: string | null;
};

type Payload = { ok: true; data: UsageData } | { ok: false; error: string };

const inhalt = document.querySelector<HTMLElement>("#inhalt")!;
const stand = document.querySelector<HTMLElement>("#stand")!;
const refreshKnopf = document.querySelector<HTMLButtonElement>("#refresh")!;

/// Letztes Ergebnis, damit der Countdown ohne neuen Abruf weiterlaufen kann.
let letztes: Payload | null = null;

const uhrzeit = new Intl.DateTimeFormat("de-DE", {
  hour: "2-digit",
  minute: "2-digit",
});

// ---------------------------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------------------------

/** `resets_at` kommt als ISO-String oder als Unix-Zeit (Sekunden oder Millisekunden). */
function zuDatum(wert: ResetsAt): Date | null {
  if (wert === null || wert === undefined) return null;

  if (typeof wert === "number") {
    // Werte unter 1e12 sind Sekunden, darüber Millisekunden.
    const ms = wert < 1e12 ? wert * 1000 : wert;
    const d = new Date(ms);
    return Number.isNaN(d.getTime()) ? null : d;
  }

  const d = new Date(wert);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** "Reset in 2 Std. 15 Min." bzw. "Reset in 7 Min." */
function countdownText(ziel: Date): string {
  const restMs = ziel.getTime() - Date.now();
  if (restMs <= 0) return "Reset steht an";

  const restMin = Math.floor(restMs / 60000);
  const tage = Math.floor(restMin / 1440);
  const stunden = Math.floor((restMin % 1440) / 60);
  const minuten = restMin % 60;

  if (tage > 0) {
    return stunden > 0
      ? `Reset in ${tage} Tg. ${stunden} Std.`
      : `Reset in ${tage} Tg.`;
  }
  if (stunden > 0) return `Reset in ${stunden} Std. ${minuten} Min.`;
  return `Reset in ${minuten} Min.`;
}

/**
 * Stufe für die Balkenfarbe. Grundlage sind die Schwellen aus dem Plan
 * (unter 60 ruhig, 60–85 Warnung, ab 85 kritisch). Eine vom Server
 * mitgelieferte `severity` darf nur nach oben korrigieren, nie nach unten —
 * bekannt ist bisher nur der Wert "normal", alles andere wäre geraten.
 */
function stufe(limit: Limit): "ruhig" | "warnung" | "kritisch" {
  const ausProzent =
    limit.percent >= 85 ? 2 : limit.percent >= 60 ? 1 : 0;

  const ausSeverity =
    limit.severity === "critical" ? 2 : limit.severity === "warning" ? 1 : 0;

  const hoechste = Math.max(ausProzent, ausSeverity);
  return hoechste === 2 ? "kritisch" : hoechste === 1 ? "warnung" : "ruhig";
}

/** Alle Texte per textContent — nie innerHTML mit Daten aus der API. */
function textEl(tag: string, klasse: string, text: string): HTMLElement {
  const el = document.createElement(tag);
  el.className = klasse;
  el.textContent = text;
  return el;
}

// ---------------------------------------------------------------------------
// Darstellung
// ---------------------------------------------------------------------------

function zeichneLimit(limit: Limit): HTMLElement {
  const zeile = document.createElement("div");
  zeile.className = "limit";

  const kopf = document.createElement("div");
  kopf.className = "limit-kopf";
  kopf.append(
    textEl("span", "limit-name", limit.label),
    textEl("span", "limit-prozent", `${Math.round(limit.percent)} %`),
  );

  const spur = document.createElement("div");
  spur.className = "spur";
  const balken = document.createElement("div");
  balken.className = `balken ${stufe(limit)}`;
  // Werte ausserhalb 0–100 würden den Balken aus der Spur laufen lassen.
  balken.style.width = `${Math.min(100, Math.max(0, limit.percent))}%`;
  spur.append(balken);

  zeile.append(kopf, spur);

  const ziel = zuDatum(limit.resets_at);
  if (ziel) {
    zeile.append(
      textEl(
        "div",
        "limit-reset",
        `${countdownText(ziel)} · ${uhrzeit.format(ziel)} Uhr`,
      ),
    );
  }

  return zeile;
}

function zeichne(payload: Payload): void {
  inhalt.replaceChildren();

  if (!payload.ok) {
    const box = document.createElement("div");
    box.className = "fehler";
    box.append(
      textEl("p", "fehler-titel", "Keine Daten"),
      textEl("p", "fehler-text", payload.error),
    );
    inhalt.append(box);
    stand.textContent = "";
    return;
  }

  const { limits, fetched_at, stale, notice } = payload.data;

  // Veraltete Zahlen bleiben stehen, werden aber deutlich gekennzeichnet.
  if (stale) {
    const box = document.createElement("div");
    box.className = "veraltet";
    box.append(textEl("p", "veraltet-titel", "Werte sind veraltet"));
    if (notice) box.append(textEl("p", "veraltet-text", notice));
    inhalt.append(box);
  }

  for (const limit of limits) {
    inhalt.append(zeichneLimit(limit));
  }

  const geholt = zuDatum(fetched_at);
  if (!geholt) {
    stand.textContent = "";
  } else if (stale) {
    stand.textContent = `Stand von ${uhrzeit.format(geholt)} — nicht aktuell`;
  } else {
    stand.textContent = `Zuletzt aktualisiert: ${uhrzeit.format(geholt)}`;
  }
}

// ---------------------------------------------------------------------------
// Abläufe
// ---------------------------------------------------------------------------

function uebernimm(payload: Payload | null): void {
  if (!payload) return;
  letztes = payload;
  zeichne(payload);
}

async function jetztAktualisieren(): Promise<void> {
  refreshKnopf.disabled = true;
  try {
    uebernimm(await invoke<Payload | null>("refresh_now"));
  } catch (e) {
    uebernimm({ ok: false, error: `Abruf fehlgeschlagen: ${String(e)}` });
  } finally {
    refreshKnopf.disabled = false;
  }
}

refreshKnopf.addEventListener("click", () => {
  void jetztAktualisieren();
});

// Beim Laden den zwischengespeicherten Stand zeigen, damit das Fenster
// nicht leer aufgeht, und danach auf Aktualisierungen aus Rust hören.
void invoke<Payload | null>("get_last").then(uebernimm).catch(() => {
  /* Noch kein Abruf durch — die Poll-Schleife meldet sich gleich. */
});

void listen<Payload>("usage-updated", (event) => uebernimm(event.payload));

// Countdown lokal weiterzählen, ohne die API erneut zu belasten.
setInterval(() => {
  if (letztes) zeichne(letztes);
}, 30_000);
