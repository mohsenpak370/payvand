---
title: Staking, Validatoren und Epochen
description: Stake-Konten, Vote-Konten, Warmup, Kommissionen und ein Leader-Zeitplan, der einen Tag im Voraus feststeht. Wie der Proof of Stake von Solana das Recht, Blöcke zu schreiben, wirklich verteilt.
category: technical
order: 5
level: technical
---

Das Sicherheitsbudget von Solana ist eingesetztes SOL. Die Staking-Mechanik zu verstehen erklärt, warum
sich das Netzwerk unter Last so verhält, wie es sich verhält, und was „Validator“ wirklich heißt.

## Zwei Konten, zwei Aufgaben

- Ein **Stake-Konto** hält das SOL eines Delegierenden und vermerkt, welchem **Vote-Konto** eines
  Validators es zugewiesen ist. Dort liegt das wirtschaftliche Gewicht.
- Ein **Vote-Konto** gehört einem Validator und hält seine Stimmen und Stimmpunkte. Es hält nicht den
  zugewiesenen Einsatz.

Ein Stake-Konto weist jeweils einem Validator zu; auf mehrere zu verteilen heißt mehrere Stake-Konten.
Delegation ist keine Verwahrung — der Validator erhält nie die Möglichkeit, deinen Einsatz zu bewegen,
nur mit dessen Gewicht zu stimmen.

## Warmup, Abkühlen und Epochen

Änderungen an der Delegation wirken nicht sofort. Einsatz **wärmt auf**, um aktiv zu werden, und
**kühlt ab**, bevor er abgehoben werden kann, und das Netzwerk begrenzt, wie viel pro Epoche aktiviert
oder deaktiviert werden kann. Die genaue Verzögerung hängt also davon ab, wie viel Einsatz gerade
unterwegs ist.

Eine **Epoche** ist **432.000 Slots**. Bei der aktuellen Slot-Zeit von 200 ms sind das nominell **24
Stunden**; ausgelassene Slots lassen die reale Zeit schwanken. An Epochengrenzen wirken Einsatzgewichte,
Leader-Zeitpläne und die Verteilung der Belohnungen.

Zwei praktische Werte:

- Die **Mindestdelegation** ist ein Cluster-Wert; das dokumentierte Beispiel aus
  `getStakeMinimumDelegation` ist **1 SOL**. Frag den Cluster ab, statt ihn fest zu verdrahten.
- Die **Kommission** ist der prozentuale Anteil des Validators an den Belohnungen, gesetzt am
  Vote-Konto. Sie wird auf Belohnungen erhoben, nie auf den Kapitalbetrag.

## Wer welchen Block erzeugt

Der **Leader-Zeitplan** wird vor jeder Epoche berechnet. Der Algorithmus setzt eine pseudozufällige
Auswahl aus einer PoH-Tick-Höhe an, zieht die aktiven Validatoren und ihren Einsatz und wählt Leader
gewichtet nach Einsatz. Der Zeitplan deckt 432.000 Slots ab, mit einem über RPC sichtbaren
Lookahead-Versatz von 8.192 Slots.

Deshalb zählt die Konzentration des Einsatzes auch betrieblich und nicht nur wirtschaftlich: Mehr
Einsatz heißt mehr Slots, und ein gut betriebener Validator mit hohem Einsatz erzeugt einen
überproportionalen Anteil der Blöcke.

## Belohnungen, Inflation und Slashing

Die Belohnungen einer Epoche werden über den teilnehmenden Einsatz im Verhältnis
**Einsatz × Stimmpunkte des Validators** aufgeteilt, abzüglich Kommission. Die veröffentlichten
Parameter des Inflations*plans* sind 8 % anfängliche Jahresinflation, 15 % jährliche Verringerung und
1,5 % langfristige Rate — sie beschreiben die Kurve, nicht den heutigen Satz.

**Solana slasht derzeit keinen delegierten Einsatz** für Fehlverhalten von Validatoren. Ein Validator,
der offline geht oder doppelt signiert, verliert Belohnungen und Ruf, nicht das Kapital. Das ist ein
bedeutsamer Unterschied zu Netzwerken, in denen Doppelstimmen Einsatz verbrennen, und es gehört in
jeden Vergleich: Die Sicherheit ruht hier auf stake-gewichteter Abstimmung und darauf, dass
Delegierende ihren Einsatz abziehen, nicht auf einer Verbrennung.

Einen Block zu bestätigen verlangt **zwei Drittel des aktiven Einsatzes**; die Endgültigkeit unter
Alpenglow drückt sich in Votors Zertifikaten aus (siehe [die Alpenglow-Seite](/learn/alpenglow-internals)),
wobei der schnelle Pfad 80 % braucht und die Rückfallpfade 60 %-Schwellen nutzen.

## Was das für eine Anwendung bedeutet

- **Leader-Konzentration ist eine Latenzvariable.** Ein Slot, dessen Leader weit von deinen Nutzern
  entfernt oder schlecht angebunden ist, ist eine langsamere Zahlung — unsichtbar bei 200 ms, sichtbar
  unter Last.
- **Einsatz bewegt sich langsam.** Das Gewicht eines Validators lässt sich nicht für einen Slot
  leihen; Warmup und Abkühlen machen Angriffe durch wiederholtes Feilschen teuer in Zeit wie in
  Kapital.
- **Belohnungen sind keine kostenlose Rendite.** Sie sind Inflation plus Gebühren, geteilt mit einem
  Validator, der eine Kommission nimmt. Wer eine Staking-Rendite nennt, ohne Kommission und
  Inflationsquelle zu benennen, erzählt die halbe Geschichte.

## Quellen

- [Solana: staking](https://solana.com/docs/references/staking) · [stake accounts](https://solana.com/docs/references/staking/stake-accounts)
- [Anza: stake delegation and rewards](https://docs.anza.xyz/consensus/stake-delegation-and-rewards) · [leader rotation](https://docs.anza.xyz/consensus/leader-rotation)
- [getStakeMinimumDelegation](https://solana.com/docs/rpc/http/getstakeminimumdelegation) · [getEpochSchedule](https://solana.com/docs/rpc/http/getepochschedule)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
