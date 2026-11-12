---
title: Was passiert, wenn du mit einem Wallet zahlst
description: Vom Scannen des Codes bis zum ankommenden Geld — die sechs Schritte jeder Krypto-Zahlung, und was du dabei wirklich bestätigst.
category: basics
order: 4
---

Eine Krypto-Zahlung fühlt sich wie eine Kartenzahlung an, weil der Bildschirm im Laden sich gleich
verhält. Unter der Haube ist sie viel kürzer: kein Kartennetzwerk, kein Acquirer, kein
Sammelabrechnungslauf. Hier ist der ganze Weg, in der Reihenfolge, in der er abläuft.

## 1. Du bekommst eine Zahlungsanfrage

In Payvand gibt der Verkäufer einen Betrag ein und zeigt dir einen QR-Code. Der Code ist kein Bild
einer Zahlung — er *ist* die Zahlung: die Adresse des Verkäufers, der Betrag, die Währung und eine
kurze Referenz, die diese Rechnung von jeder anderen Rechnung desselben Verkäufers unterscheidet.
Wenn du ihn scannst, liest dein Wallet diese Angaben direkt aus.

An der Anfrage des Verkäufers wurde nichts „an einen Server geschickt“, das den Betrag nachträglich
ändern könnte. Der Betrag steckt in der Adresse, bei der das Geld ankommen muss.

## 2. Dein Wallet zeigt dir, was du gleich bestätigst

Im Wallet-Fenster steht die Zahlung. Das ist der wichtigste Bildschirm in Krypto, lies ihn also:
**Betrag, Währung, Empfänger, Netzwerkgebühr**. Wenn dort etwas anderes steht als auf dem
Bildschirm des Verkäufers, halt an und prüf warum, bevor du signierst. Diese Gewohnheit steht in
[Lies, bevor du signierst](/learn/checking-before-you-sign).

## 3. Du bestätigst, und das Wallet signiert

Bestätigen heißt nicht „der App sagen, sie soll zahlen“. Es schließt den privaten Schlüssel auf
deinem Gerät gerade lang genug auf, um genau diese eine Transaktion zu signieren, und dann wieder zu.
Die App, die du benutzt, sieht den Schlüssel nie; sie bekommt eine Signatur, die nur für genau diese
Transaktion gilt.

## 4. Die Transaktion geht ins Netzwerk

Die signierte Transaktion wird ins Netzwerk geschickt und in einen Block aufgenommen. Auf Solana
dauert es normalerweise ein paar Sekunden, bis sie auftaucht, und die Zahlung wird in deutlich unter
einer Sekunde *dauerhaft endgültig* — seit dem Alpenglow-Upgrade im Oktober 2026 meist etwa 150
Millisekunden ([was Finalität bedeutet](/learn/when-is-a-payment-final)). Du kannst es über den
Bestätigungslink verfolgen, den Wallet oder App dir zeigt.

## 5. Auf der Seite des Verkäufers aktualisiert es sich

Weil das Netzwerk öffentlich ist, kann die Seite des Verkäufers dessen Adresse beobachten und das
Eintreffen der Zahlung sehen, ohne dass es ihr jemand sagt. In Payvand springt die Rechnung in
diesem Moment auf **Bezahlt** — in derselben Sekunde, in der dein Wallet dir bestätigt.

## 6. Die Gebühr

Jede Transaktion zahlt eine kleine Netzwerkgebühr. Auf Solana beträgt die Grundgebühr 5.000
Lamports — das sind 0,000005 SOL, ein Bruchteil eines Cents — plus eine optionale Prioritätsgebühr,
wenn das Netzwerk ausgelastet ist. Die Gebühr geht ans Netzwerk, nicht an Payvand und nicht an den
Verkäufer.

Du brauchst dafür ein wenig SOL im Wallet. Ein Wallet mit viel Stablecoin-Guthaben und null SOL kann
nichts bewegen, was viele beim ersten Mal überrascht.

## Was „Bezahlt“ nicht ist

- **Es ist nicht umkehrbar.** Es gibt keine Rückbuchung. Hat das Netzwerk bestätigt, gilt die
  Überweisung — der Bestätigungsbildschirm verdient also deine Aufmerksamkeit.
- **Es ist nicht überall sofort.** Solana bestätigt in Sekunden, andere Netzwerke können Minuten
  brauchen, und eine Zahlung ist erst endgültig, wenn sie bestätigt ist.
- **Es ist nicht anonym.** Die Überweisung ist für jeden sichtbar, der die beiden Adressen kennt.

## Die Abkürzung von Payvand: ein monatliches Budget

Jede Zahlung einzeln zu bestätigen wird im Café lästig. Payvand lässt dich **eine** Transaktion
bestätigen, die ein monatliches Budget in einem Token wie USDC beiseitelegt, und registriert einen
Sitzungsschlüssel, den die App auf deinem Gerät hält. Danach werden Zahlungen innerhalb des Budgets
vom Sitzungsschlüssel signiert, an der Theke gibt es also kein Wallet-Fenster.

Was dieser Schlüssel kann, ist bewusst eng: Er kann Rechnungen bis zum gesetzten Budget bezahlen,
diesen Monat, und sonst nichts ([das Budget in einfachen Worten](/learn/monthly-allowance-explained)).
Er kann kein Geld irgendwohin schicken, wohin er will, nicht über das Limit hinaus, und du kannst das
Budget jederzeit schließen, um den Rest zurück in dein Wallet zu holen. Wenn dir dieser Tausch nicht
gefällt, zahle jedes Mal aus deinem Wallet — beide Wege bezahlen dieselben Rechnungen.
