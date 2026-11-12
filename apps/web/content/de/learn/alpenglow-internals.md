---
title: Alpenglow, von innen
description: Votors Zertifikate, der schnelle Pfad über 80 %, der Rückfall über 60 % und was „endgültig in 150 ms“ an Annahmen kostet. Die Konsensänderung, die TowerBFT ersetzt hat.
category: technical
order: 4
level: technical
---

Alpenglow ist die größte Änderung am Konsens von Solana seit dem Start: Es hat **TowerBFT** durch einen
neuen Finalisierungsmechanismus namens **Votor** ersetzt. Es ist im mainnet live, aktiviert zu Beginn
von **Epoche 1064 am 21. Oktober 2026**, nachdem SIMD-0326 im September 2025 mit 98,27 % des
teilnehmenden Einsatzes angenommen wurde.

Die zweite Phase, **Rotor**, die das Blockverbreitungsprotokoll Turbine ersetzt, ist nicht ausgeliefert
und nicht terminiert. Verwechsle die beiden nicht — die Upgrade-Seite ist in der Reihenfolge eindeutig.

## Was TowerBFT tat und warum es gehen musste

TowerBFT erreichte Endgültigkeit durch angesammelte **Lockouts**: Jede Stimme für einen späteren Block
erhöhte den Lockout eines Validators, und Endgültigkeit verlangte etwa **12,8 Sekunden** angesammelter
Bestätigungen. Die Stimmen selbst waren Transaktionen in Blöcken — Konsensverkehr konkurrierte mit
Nutzerzahlungen um denselben Blockspeicher.

Für Abrechnung ist das überlebbar. Für eine Kaffeeschlange ist es hoffnungslos.

## Wie Votor finalisiert

Validatoren tauschen Stimmen jetzt **direkt** aus, außerhalb des Blockstroms, und sammeln sie zu
**Quorum-Zertifikaten** — stake-gewichteten Beweisen, dass eine Schwelle so gestimmt hat. SIMD-0326
beschreibt fünf Zertifikatstypen:

| Zertifikat | Schwelle | Rolle |
|---|---|---|
| Notarization | 60 % | Validatoren einigen sich auf einen Block |
| Skip | 60 % | Validatoren einigen sich, einen Slot zu überspringen |
| Finalization | 60 % | Finalisierung in der zweiten Runde |
| Fast-Finalization | 80 % | Endgültigkeit in einer Runde |
| Notar-fallback | 60 % | Pfad, wenn die schnelle Runde nicht voll wird |

Der Glückspfad lautet also: Notarisiert **80 % des Einsatzes** einen Block in einer einzigen Runde,
existiert ein Fast-Finalization-Zertifikat und der Block ist endgültig — keine zweite Runde, keine
angesammelten Lockouts.

Passiert das nicht, fällt das Protokoll auf einen langsameren Zwei-Runden-Pfad mit den 60 %-Schwellen
zurück: ein Notarization-Zertifikat plus das passende Zertifikat der zweiten Runde. Die Endgültigkeit
kommt trotzdem, nur mit einem zusätzlichen Hin und Zurück.

## Das Fehlermodell

Alpenglow ist für **20 % feindlichen Einsatz plus 20 % offline Einsatz** zusammen ausgelegt. Das ist
nicht die Behauptung, dass 40 % byzantinischer Einsatz toleriert wird — die Aufteilung 20+20 ist die
Annahme, und die 80 %-Schwelle des schnellen Pfads kodiert sie.

Das zählt, wenn man Marketingzahlen liest. „80 % des Einsatzes“ ist keine Supermehrheit im klassischen
Zwei-Drittel-Sinn; es ist die Schwelle, die eine Runde *unter dieser Annahme* ausreichen lässt.

## Woher die 150 ms kommen

Der Designbeitrag von Anza nennt einen **Median von etwa 150 ms**, mit Fällen um **100 ms**, aus
Simulationen mit der damaligen Einsatzverteilung, ausdrücklich ohne Rechenaufwand. Die Upgrade-Seite
von Solana beschreibt ~150 ms als Erwartung, und das Live-Dashboard zeigt meist um 200 ms.

Ein veröffentlichtes Maximum gibt es nicht. Die ehrliche technische Aussage ist: ein Hin und Zurück
zwischen Validatoren, plus Übertragung, plus Varianz in Einsatzverteilung und Slot-Timing.

## Was sich über die Zahl hinaus geändert hat

- **Stimmen-Transaktionen sind aus den Blöcken verschwunden**, der Konsens verbraucht also keinen
  Blockspeicher mehr, den Zahlungen brauchen.
- **Proof of History taktet die Blockproduktion nicht mehr.** Timeouts und Zertifikate von Votor haben
  diese Aufgabe übernommen, und Tick-Einträge haben den Shred-Strom verlassen.
- **`confirmed` und `finalized` sind zusammengefallen.** Anza dokumentiert beide als dauerhaft
  festgeschrieben; `processed` bleibt der schwache frühe Zustand. Neuer Code sollte `finalized`
  bevorzugen, und Solana sagt, `confirmed` werde in einer späteren Version entfernt.
- **Die Ausführung ist unberührt.** SVM, Programme, Kontomodell und Transaktionsformat haben sich nicht
  geändert. Eine Zahlungs-App muss nicht neu geschrieben werden; sie muss einem anderen
  Commitment-Label vertrauen.

## Wie du einen Cluster prüfst

`getAgGenesisCert` meldet, ob ein Cluster migriert ist, und die Alpenglow-Genesis für das mainnet ist
Slot **459.653.000**. Auf devnet war die Aktivierung Epoche **1167, 25. September 2026**. Für alles
Sichtbare frag lieber das Commitment ab, statt ein Datum anzunehmen.

## Was das für eine App wie diese bedeutet

Ein Kassiervorgang kann „finalized“ als „Ware rausgeben“ behandeln, weil keine Lockout-Periode mehr
abzuwarten ist. Das Risiko, das es nicht beseitigt, ist das menschliche: Endgültigkeit macht eine
falsche Adresse schneller denn je für immer falsch. Deshalb zählt der Bestätigungsbildschirm weiter
mehr als das Bestätigungsabzeichen.

## Quellen

- [Solana: Alpenglow upgrade and status](https://solana.com/upgrades/alpenglow) · [dashboard](https://solana.com/upgrades/alpenglow/dashboard)
- [SIMD-0326: Alpenglow](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md)
- [Anza: Alpenglow, a new consensus for Solana](https://www.anza.xyz/blog/alpenglow-a-new-consensus-for-solana)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
