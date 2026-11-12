---
title: Wann ist eine Zahlung wirklich fertig?
description: Eine Kartenzahlung lässt sich wochenlang zurückdrehen. Eine Solana-Zahlung wird in unter einer Sekunde endgültig. Was „endgültig“ heißt und was sich im Oktober 2026 geändert hat.
category: solana
order: 2
---

Hier ist die Alltagsversion einer technischen Frage. Du tippst, um einen Kaffee zu bezahlen. Wann kann
der Laden dir den Becher beruhigt geben und dich gehen lassen?

Bei einer Karte lautet die ehrliche Antwort „in ein paar Tagen“, deshalb lassen sich Kartenzahlungen
zurückholen und deshalb zahlen Läden für dieses Risiko. Bei Solana lautet die Antwort inzwischen
**unter einer Sekunde**, und das gilt seit einem Upgrade namens **Alpenglow**, das am 21. Oktober 2026
im Hauptnetz live ging.

## Drei Zustände, in denen eine Zahlung sein kann

- **Gesehen (processed).** Ein Computer hat die Zahlung im Blick. Sie kann noch ersetzt werden, wenn
  das Netzwerk sich auf eine andere Version der Ereignisse umstellt. Nicht verlässlich.
- **Bestätigt (confirmed).** Genug des Netzwerks ist sich einig. Vor Alpenglow war das schnell, aber
  schwächer als endgültig — eine Zahlung konnte bestätigt und bei einer seltenen Umordnung trotzdem
  verloren sein.
- **Endgültig (finalized).** Dauerhaft aufgeschrieben. Sie lässt sich nicht rückgängig machen und
  nicht umsortieren, auch nicht von den Validatoren, die sie geschrieben haben.

Vor Alpenglow dauerte die Endgültigkeit auf Solana etwa **13 Sekunden**. Das brachte Läden in eine
unangenehme Lage: 13 Sekunden an der Theke warten, oder „bestätigt“ akzeptieren und ein kleines
Risiko tragen. Beides ist kein schöner Weg, Kaffee zu verkaufen.

## Was sich geändert hat

Alpenglow hat das alte Abstimmungssystem durch eines ersetzt, in dem Validatoren direkt abstimmen und
ihre Stimmen zu einem **Zertifikat** gesammelt werden. Sind sich Validatoren mit 80 % der eingesetzten
SOL in einer einzigen Runde einig, ist die Zahlung sofort endgültig; wenn nicht, entscheidet eine
zweite Runde.

Die praktischen Zahlen:

- **Typischerweise etwa 150 Millisekunden** — rund eine Siebtelsekunde — von der Aufnahme der Zahlung
  bis zur Endgültigkeit.
- Manchmal schneller (etwa 100 ms), manchmal langsamer. Das Live-Dashboard von Solana zeigt meist
  Endgültigkeit um 200 Millisekunden.
- Es hängt davon ab, wie der Einsatz verteilt ist, und ein garantierter Höchstwert wird nicht
  veröffentlicht. Sicher sagen lässt sich: **deutlich unter einer Sekunde**.

Eine Folge, die man kennen sollte: Unter Alpenglow bedeuten **„confirmed“ und „finalized“ dasselbe**.
Die beiden Bezeichnungen gibt es noch, aber beide meinen jetzt dauerhaft festgeschrieben. Nur
„processed“ ist weiterhin ein früher, unsicherer Zustand.

## Warum das an der Theke zählt

Der Laden kann den Kaffee in dem Moment übergeben, in dem die App „bestätigt“ meldet, weil nichts
mehr zu warten ist. Kein Schwebezustand, kein „mal sehen, ob es durchgeht“, keine 13 Sekunden Pause
mit einer Schlange hinter dir. Das macht eine Krypto-Zahlung zu einem Karten-Tippen statt zu einer
Banküberweisung.

## Die andere Seite von endgültig

Derselbe Mechanismus, der eine Zahlung für einen Laden dauerhaft zu Geld macht, macht einen Fehler
dauerhaft zum Fehler. Falsche Adresse, falscher Betrag, eine Seite, die dich reingelegt hat — einmal
endgültig, immer endgültig, und niemand da, bei dem du Einspruch einlegen kannst.

Geschwindigkeit und Unumkehrbarkeit sind also eine Eigenschaft, nicht zwei. Genau deshalb lohnt es
sich, [zu lesen, was du vor dem Signieren prüfen solltest](/learn/checking-before-you-sign).

## Was das für dich bedeutet

Wenn Payvand eine Zahlung als bestätigt anzeigt, ist das Geld wirklich angekommen und bleibt
angekommen — in weniger Zeit, als du brauchst, um das Handy zurück in die Tasche zu stecken. Der Teil,
der weiterhin deine Aufmerksamkeit verdient, ist die halbe Sekunde *vor* deiner Bestätigung.
