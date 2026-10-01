# Lab-Redesign v3 „Wolkenmeer" — 2026-09-30

> **Herkunft:** Entstanden im Lab (`../Unkai Mobile Lab`) und am
> 2026-09-30 in die stabile App übernommen — ohne die Rückgängig-Funktion,
> die im Lab danach hinzukam (eine Funktion, kein Design, und noch nicht
> auf einem Gerät geprüft). Eine Abweichung: Hier ist das App-Icon navy
> (`storm`), Morgenrot ist also nicht die Icon-Farbe, sondern das erste
> Licht über dem Wolkenmeer — und die Farbe, die ein Zähler-Badge schon
> immer hatte. Die Screenshots zeigen das Lab.

v2 (am selben Tag, siehe `2026-09-30-redesign.md`) hat die **Struktur**
repariert: Posteingang zuerst, eine beschriftete Hauptaktion pro Screen,
Rollennamen statt Servernamen. Dieses Dokument hält fest, warum danach
noch einmal das **Aussehen** umgebaut wurde, und wie.

Anlass war das Urteil über v2: „sieht nach KI-Vorlage aus". Die
Kritik unten zeigt, dass das keine Geschmacksfrage war, sondern an
benennbaren Stellen hing.

Gearbeitet wurde mit den Design-Skills in dieser Reihenfolge:
Design-Kritik → Designrichtung (frontend-design) → Design-System-Audit →
Interaction Design → UX-Copy → Barrierefreiheits-Audit (WCAG 2.1 AA).
Geprüft im Simulator (iPhone 17 Pro, iOS 26.5, Deutsch, Testmodus) in
Hell, Dunkel und bei der größten Textgröße.

---

## 1. Kritik an v2 — warum es generisch wirkte

| Befund | Warum es nach Vorlage aussieht |
|---|---|
| Ein Initialen-Kreis in Pastell vor **jeder** Mailzeile | Das Kontaktlisten-Bausatzteil schlechthin. 66 pt Breite pro Zeile für zwei Buchstaben, die der Name daneben schon ausschreibt; die sechs Farben bedeuten nichts. |
| Farbige Symbol-Kacheln in den Einstellungen | Eine Kopie der System-Einstellungen, und fünf gesättigte Farben auf einem Screen, dessen Regel „eine Akzentfarbe" war. |
| Dieselbe Karte überall (20 px Radius, derselbe weiche Schatten) | Absenderblock, Anhang, Mailtext, jede Gruppe, jede Kachel — das „Karten-Kit". Wenn alles gleich gerahmt ist, sagt der Rahmen nichts mehr. |
| Akzentfarbe auf jedem Bedienelement | Leisten-Buttons, Zurück, Datum ungelesener Mails, aktiver Tab, schwebender Button, Links, Zähler. Wenn alles blau ist, ist nichts hervorgehoben. |
| Getönte Pille hinter dem aktiven Tab-Symbol, farbig leuchtender „Neue Nachricht"-Button | Die Standardwerte eines bekannten Komponenten-Frameworks. |
| Das Plattform-Blau, die Systemschrift ohne Eigenheit | Nichts daran ist Unkai; es könnte jede App sein. |
| Weißer Block als Mailtext im Dunkelmodus, Leerzustände als „Symbol im getönten Kreis", gestrichelter Kasten für „Nichts geplant" | Weitere Vorlagen-Reflexe. |

Was gut war und blieb: das Token-System, der Kontrast-Solver, Dynamic
Type, VoiceOver-Zugang zu jeder Geste, große Titel, Gesten mit Toast.

## 2. Richtung: „Wolkenmeer"

*Unkai* (雲海) heißt Wolkenmeer — der Blick von oben, wenn das Tal
voller Wolken liegt. Die App verspricht genau das: ruhig über dem
Postfach-Lärm. Daraus die Palette und fünf Regeln.

**Farbe — zwei Farben, zwei Aufgaben**

| Name | Wert (Hell) | Aufgabe |
|---|---|---|
| Nebel | `#eef2f4` (surface-100) | Grund, auf dem Karten liegen |
| Wolke | `#fbfcfd` (surface-50) | Seiten und Karten |
| Nachtblau | `#141e26` (Tinte) | Text |
| **Petrol** | `#0b7089` (primary-500) | alles, was man **tun** kann: Buttons, Links, Schalter, aktiver Tab |
| **Morgenrot** | `#ca3324` (secondary-500) | alles, was **neu** ist: Ungelesen-Punkt, Tab-Zähler, „Heute" im Kalender — das Rot des Lab-Icons |
| Nacht | `oklch(0.205 0.03 250)` | Dunkelmodus-Grund: ein klares Nachtblau, kein getöntes Schwarz |

Petrol ist bewusst keine Standardfarbe (nicht Plattform-Blau, nicht
Framework-Indigo, kein schwarzer Button). Rot als Signal ist
semantisch ehrlich: rot heißt „schau hier", beim Ungelesen-Punkt wie
beim Löschen.

**Schrift — „Rund spricht, Text liest"**: SF Pro Rounded (schwer, eng
gesetzt) für die Stimme der App — Screen-Titel, Abschnittstitel, den
Betreff im Leser, Kalenderziffern. Das Logo ist mit runden Strichenden
gezeichnet; die runde Schrift ist dieselbe Hand. SF Pro für alles, was
man liest. Beides Systemschriften: nichts eingebettet, Dynamic Type
greift weiter.

**Regeln**

1. **Tinte auf Nebel.** Farbe ist ein Signal, nie Dekoration.
2. **Zwei Farben, zwei Aufgaben** (siehe oben).
3. **Rund spricht, Text liest.**
4. **Rahmen nur für Gruppen.** Eine Karte heißt „gehört zusammen"
   (Einstellungen, Formulare). Mailliste, Kontakte und die Mail selbst
   stehen auf der Seite. Karten sind flach, ohne Schatten.
5. **Eine linke Kante** (`--gutter`, 20 pt). Titel, Zeilentext,
   Abschnittstitel und Kartenkanten beginnen dort; Markierungen wie der
   Ungelesen-Punkt hängen davor im Rand.
6. **Leisten schweben.** Tab-Leiste, Leser-Werkzeugleiste, Sheets und
   Mehrfachauswahl sind Kapseln über dem Home-Indikator; die Liste
   läuft darunter durch.

Das eine laute Element: der große, schwere, runde Titel. Alles
darum bleibt leise.

**Gegenprobe gegen typische KI-Looks:** kein Creme-Grund mit Serife und
Terrakotta; kein Fast-Schwarz mit einer Neonfarbe; keine
Zeitungs-Haarlinien; kein Karten-Kit; keine GROSSBUCHSTABEN-Überzeilen,
keine „A · B · C"-Metazeilen, keine Pfeile hinter Buttontexten.

## 3. Umsetzung

| Bereich | v2 | v3 |
|---|---|---|
| Haus-Theme | Blau, Systemschrift | Petrol + Morgenrot, Nebel/Nacht, SF Pro Rounded für Überschriften (`themes/unkai.css`) |
| Kontrast-Solver | 19 Tokens | + `--ui-signal-ink/-fill`, `--ui-on-signal` (Rolle „neu", aus der secondary-Skala, für alle 23 Themes gelöst) |
| Mailzeile | Avatar, Punkt in eigener Spalte, farbiges Datum | kein Avatar, Punkt hängt im Rand, Text an der Kante, Zeiten in Tabellenziffern |
| Leser | Absender-Karte, Mailtext-Karte | Briefkopf zwischen Haarlinien, Text auf weißem Papier; im Dunkeln als Blatt auf der Nacht |
| Tab-Leiste | Leiste über die ganze Breite, getönte Pille | schwebende Kapsel, aktiver Tab als Raute mit fettem Label, Zähler in Morgenrot |
| „Neue Nachricht" | Pille mit farbigem Schein | Petrol-Pille, neutraler Schatten, klappt beim Runterscrollen zum Kreis ein |
| Einstellungen | farbige Symbol-Kacheln je Kategorie | Symbole in gedämpfter Tinte; Farbe nur für Warnung/Gefahr. `FeatureDef.tone` entfernt |
| Kontakte | eine Karte pro Buchstabe | eine ruhige Liste, Buchstaben als angeheftete Marken |
| Kalender | Heute als Akzent-Ring | Heute in Morgenrot, Auswahl in Petrol, Ziffern rund; Ortsangabe mit eigenem Symbol statt Emoji |
| Sheets | am unteren Rand angeklebt | schwebend, rundum gerundet; Scrim in der Nachtfarbe |
| Toasts | fest codiertes Systemgrau | Umkehrung der Seite (Tinte als Grund) — ≥ 7:1 in jedem Theme |
| Leerzustände | Symbol in getöntem Kreis | großes, leises Symbol, Titel in der runden Schrift |
| Erster Start | zentriert, Logo auf schwebender Karte | linksbündig an der Kante, Logo in App-Icon-Größe, Titel groß und rund |
| „Papierkorb leeren" | rote gefüllte Kapsel | leise Kapsel mit roter Schrift; Rot-gefüllt erst im Bestätigungsdialog |
| Verschieben-Auswahl | „INBOX", „Drafts", „Junk" | Rollennamen wie in der Postfachliste (`mail/folderNames.ts`) |

## 4. Design-System-Audit

| Kategorie | Gefunden | Behoben |
|---|---|---|
| Farben | 3 fest codiert (Toast-Grau, Sheet-Scrim, Avatar-Rohschattierungen) | alle auf Tokens; Ausnahme bewusst: Anbieter-Monogramme (Markenfarbe, dekorativ) |
| Schriftgrößen | 1 px-Größe (Konto-Emoji) | folgt Dynamic Type |
| Radien | 12 Rohwerte (`999px`, `0.75rem`, …) | Tokens; neue Stufe `--r-xs`, Karten 18 statt 20 |
| Abstände | 7 Seiten-Einzüge auf 16 statt 20 pt | `.page-inset` / `--gutter` |
| Schatten | Schatten unter jeder Karte | entfernt; `--e-float` nur für Schwebendes |
| Tote Daten | `tone` an Features/Einstellungen | entfernt |

## 5. Interaction Design

| Befund | Entscheidung |
|---|---|
| Tab-Leiste und „Neue Nachricht" schweben beide unten rechts über der Liste | Die Pille klappt beim Runterscrollen zum Kreis ein und kommt beim Hochscrollen zurück (Hysterese 24 pt, Überscroll-Federn ignoriert). |
| Großer Titel klappte im Posteingang nie ein | Vorbestehender Fehler: `NavBar` suchte den Scroller einmal beim Mounten, die Liste kommt aber erst nach dem Spinner. Jetzt Scroll-Ereignis in der Capture-Phase am Screen. |
| Löschen/Archivieren bieten **kein** Rückgängig, obwohl `CLAUDE.md` es verspricht — `toasts.undo()` hat keinen Aufrufer | Braucht die neue UID im Zielordner vom Backend (Rust); eigene Aufgabe, nicht Teil des Redesigns. |
| Schalter 36 × 20 | 46 × 28, Knopf mit Federkurve. |

## 6. UX-Copy

| Schlüssel | vorher | nachher |
|---|---|---|
| `mobile_end_of_mailbox` | „{count} Nachrichten · das war alles" | „Das waren alle {count} Nachrichten." |
| `mobile_outbox_attempts` | „{count} Versuche · eingereiht {when}" | „{count} Versuche, eingereiht {when}" |
| `mobile_theme_desc_unkai` | „Ruhig und klar – der Unkai-Stil" | „Ruhige Flächen, Petrol und Rot für Neues" |
| Verschieben-Toast | „Nach INBOX verschoben" | „Nach Posteingang verschoben" |

Englisch jeweils entsprechend. Keine Bedienelemente umbenannt, daher
bleibt die Einführung (`Intro.svelte`) korrekt.

## 7. Barrierefreiheits-Audit (WCAG 2.1 AA)

Gemessene Kontraste der neu entstandenen Paare (Haus-Theme):

| Element | Hell | Dunkel | Ziel |
|---|---|---|---|
| Aktiver Tab (Akzent auf Raute) | 5,13 | 4,59 | 4,5 |
| Inaktiver Tab | 5,95 | 7,04 | 4,5 |
| Ungelesen-Punkt | 5,88 | 7,28 | 3 |
| Gelesenes Datum (blass) | 5,12 | 5,98 | 4,5 |
| Zähler (Morgenrot) | 5,24 | 5,24 | 4,5 |
| Toast | 15,3 | 15,3 | 4,5 |
| Avatar-Initialen | 4,94 | 6,59 | 4,5 |
| „Papierkorb leeren" | 5,04 | 5,35 | 4,5 |
| „Neue Nachricht" | 5,69 | 5,69 | 4,5 |
| Schalter aus (Spur) | 3,31 | 3,52 | 3 |

`accessibleColors.test.ts` prüft die neuen Signal-Tokens für alle 23
Themes und fünf eigene Themes, Standard und „Kontrast erhöhen".

| # | Befund | Kriterium | Behoben durch |
|---|---|---|---|
| 1 | Eingeklappte Pille: Name hing an einem Label mit Breite 0 | 4.1.2 | `aria-label` mit demselben Text (2.5.3 bleibt erfüllt) |
| 2 | Tastatur-/Schaltersteuerungs-Fokus konnte unter der schwebenden Leiste landen | 2.4.7 | `scroll-padding-bottom: var(--bar-clearance)` |
| 3 | Angeheftete Kontakt-Buchstaben konnten eine fokussierte Zeile verdecken | 2.4.7 | `scroll-margin-top` |
| 4 | Großer Titel lief bei größter Textgröße aus dem Bild („Posteingang") | 1.4.4 / 1.4.10 | Silbentrennung nach Dokumentsprache („Postein-gang") |
| 5 | Kontakt-Buchstaben waren `div`s | 1.3.1 | jetzt `h2` |

Nicht per Kommandozeile prüfbar, braucht ein Gerät: VoiceOver-Reihenfolge
mit der schwebenden Leiste, Voice Control („Tippe Neue Nachricht" im
eingeklappten Zustand), Tastatur auf dem iPad.

## 8. Bilder

Simulator, iPhone 17 Pro, iOS 26.5, Testmodus: [`2026-09-30-v3/`](2026-09-30-v3/).
Der Stand davor liegt in [`2026-09-30/`](2026-09-30/) (`nachher-*` = v2).

**Nicht im Simulator gesehen:** der erste Start (Willkommen-Screen). Um
ihn zu zeigen, hätte der Testmodus verlassen werden müssen, während eine
zweite Sitzung im selben Simulator am Rückgängig-Thema arbeitete.
