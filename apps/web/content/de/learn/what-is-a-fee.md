---
title: Warum jede Zahlung einen Bruchteil eines Cents kostet
description: Wer die Gebühr bekommt, warum der Preis auf dem Code die vollen Kosten sind, und warum eine fehlgeschlagene Zahlung trotzdem etwas kostet.
category: basics
order: 6
---

Jede Zahlung ist eine neue Zeile im gemeinsamen Heft, und sie zu schreiben kostet einen Computer
echte Arbeit. Die **Gebühr** bezahlt diese Arbeit.

Auf Solana beträgt die Grundgebühr **5.000 Lamports**, also 0,000005 SOL — ein Bruchteil eines
Cents. Sie wird pro Signatur berechnet, eine einfache Zahlung ist also eine Gebühr.

## Wie das im Vergleich zur Karte aussieht

Bei einer Karte zahlt der Laden etwa 1–3 % jedes Verkaufs an das Kartennetzwerk, plus eine monatliche
Miete fürs Terminal. Bei einer Zahlung von 4,50 sind das ein paar Cents, die auf der Seite des Ladens
verschwinden — deshalb lehnen kleine Läden Karten manchmal unter einem Mindestbetrag ab.

Eine Krypto-Gebühr unterscheidet sich in zwei Punkten:

- **Sie ist pauschal.** 2 zu zahlen oder 2.000 kostet denselben winzigen Betrag, weil die Arbeit
  dieselbe ist.
- **Der Verkäufer zahlt sie, aus dem Preis.** Keine Terminalmiete, kein Monatsvertrag und nichts
  obendrauf: Der Zahler zahlt genau den Preis auf dem Code, und Netzgebühr wie Miete des Belegs
  gehen von der Seite des Verkäufers ab.

## Warum du kein SOL brauchst, auch wenn du in USDC zahlst

Die Gebühr wird immer in der eigenen Coin des Netzwerks bezahlt, in SOL. Denk an die Briefmarke: Der
Brief kann Dollar enthalten, aber die Marke muss von der Post selbst sein.

Diese Marke musst aber nicht du kaufen. Payvands Relayer zahlt die Netzgebühr und legt die kleinen
Mieten für die Zahlung und ihren Beleg auf der Kette vor, und dieselbe Transaktion erstattet sie ihm
aus dem Preis. Ein Wallet voller USDC und ohne SOL kann also zahlen, und sein Guthaben sinkt genau um
den Preis.

Die Gebühr zählt pro Signatur: Eine Abrechnung trägt eine, wenn das Wallet selbst zahlt, und zwei,
wenn ein Relayer sie einreicht. Der Preis wird für genau diese Anzahl berechnet, ein Relayer behält
also nie mehr, als er ausgegeben hat. Nur wenn kein Relayer die Zahlung übernimmt, legt das Wallet
diese Kosten selbst vor — und dieselbe Transaktion gibt sie zurück.

## Wenn das Netzwerk ausgelastet ist

Solana erlaubt zusätzlich eine optionale **Prioritätsgebühr** — ein kleines Trinkgeld für schnellere
Aufnahme, ähnlich einem Express-Zuschlag. Meistens ist sie nicht nötig, und Payvand fügt sie
gewöhnlichen Zahlungen nicht hinzu.

## Warum eine fehlgeschlagene Zahlung trotzdem etwas kostet

Wenn eine Transaktion ins Netzwerk gelangt und scheitert, werden die Änderungen zurückgenommen, die
Gebühr aber trotzdem berechnet, weil die Arbeit bereits getan wurde. Es ist ein winziger Betrag, aber
ein Grund, nicht auf einen Button zu hämmern, der immer wieder scheitert — behebe zuerst die Ursache.

## Noch ein kleiner Posten: die Einlage für ein Konto

Ein neues Konto im Netzwerk anzulegen braucht eine kleine Einlage in SOL, die gehalten wird, solange
das Konto existiert. Sie ist **rückzahlbar** — das Schließen des Kontos gibt sie zurück. Der monatliche
Topf von Payvand macht das einmal, deshalb kostet das Öffnen eines Budgets etwas mehr als das Limit
selbst.

## Was das für dich bedeutet

Gebühren sind nichts, worüber du dir Sorgen machen oder das du einplanen müsstest. Der Preis auf dem
Code sind die vollen Kosten: Netzgebühr und Miete des Belegs stecken bereits darin und bleiben so
klein, dass sie leichter zu ignorieren als auszurechnen sind.

Die Netzwerkgebühr ist nur der kleinste Teil dessen, was eine Zahlung kostet. Wohin der Rest geht — der Anteil des Protokolls, der dauerhafte Beleg auf der Blockchain und die Kosten, die den Dienst am Laufen halten — steht in [Was eine Zahlung kostet](/learn/what-a-payment-costs).
