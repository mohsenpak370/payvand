---
title: Lies, bevor du signierst
description: Der Bestätigungsbildschirm ist der letzte Moment, in dem irgendjemand eine Zahlung stoppen kann. Was du prüfen solltest und der Unterschied zwischen Geld senden und ein Programm später ausgeben lassen.
category: safety
order: 3
---

Jede Zahlung in Krypto läuft durch einen Moment: ein Fenster in deinem Wallet, das dich um eine
Signatur bittet. Bis dahin hat sich nichts bewegt und alles lässt sich kostenlos abbrechen. Danach ist
die Zahlung endgültig — auf Solana etwa 150 Millisekunden später, ohne Rückgängig für dich oder
irgendjemand anderen ([warum das so schnell ist](/learn/when-is-a-payment-final)). Behandle diesen
Bildschirm als die eigentliche Entscheidung, nicht als Formalität zum Wegklicken.

## Was eine Transaktion wirklich enthält

Eine Transaktion ist eine Liste von Anweisungen plus eine Gebühr. Bei einer einfachen Zahlung ist die
Liste kurz: beweg diesen Betrag dieses Tokens von meiner Adresse zu jener Adresse. Dein Wallet macht
daraus etwas, das dein Schlüssel signieren kann, und das Netzwerk führt es aus.

Lies jedes Mal vier Dinge:

1. **Den Betrag** — inklusive Nachkommastellen. `0,5` und `5` sehen auf den ersten Blick ähnlich aus.
2. **Die Währung oder das Token** — ein Token kann heißen, wie es will. Wenn eine App USDC anzeigt,
   das Wallet aber ein unbekanntes Token, stimmt etwas nicht.
3. **Den Empfänger** — prüf Anfang und Ende der Adresse und, wichtiger, ob es der ist, den du
   erwartest. Wenn du einem Laden zahlst, sollte es die Adresse aus der gerade gescannten Rechnung
   sein, nicht eine, die du aus einem Chat kopiert hast.
4. **Die Gebühr** — auf Solana klein, aber eine absurd hohe Gebühr ist ein Signal, dass die
   Transaktion nicht das ist, was du denkst.

## Senden ist nicht dasselbe wie Bestätigen

Eine Signatur kann zwei sehr verschiedene Dinge tun:

- **Eine Überweisung** bewegt einen bestimmten Betrag, einmal, an eine bestimmte Adresse. Das ist eine
  Zahlung.
- **Eine Freigabe (approval)** gibt einem Programm die Erlaubnis, später Token von deinem Konto zu
  bewegen, manchmal bis zu einem Betrag weit über der Zahlung, die vor dir liegt. Das ist eine
  dauerhafte Erlaubnis, keine Zahlung.

Freigaben gibt es aus guten Gründen — Token zu tauschen braucht eine —, aber sie sind auch das
Lieblingswerkzeug von Drainern, weil eine einzige Freigabe ein Guthaben lange nach dem Schließen des
Tabs leerräumen kann. Hast du keine Freigabe erwartet, brich ab. Musst du eine geben, gib einen
bestimmten kleinen Betrag statt eines unbegrenzten und widerruf sie danach.

## Warum du dem Betrag hier trauen kannst

Die Rechnungen von Payvand legen Betrag und Empfänger *in die Adresse, bei der die Zahlung ankommen
muss*. Es gibt kein separates Feld „Betrag“, das ein Server zwischen QR-Code und deiner Bestätigung
ändern könnte: Zahlst du einen anderen Betrag, geht die Zahlung an eine andere Adresse, und die ist
nicht die, die der Verkäufer beobachtet. Der Verkäufer kann den Preis nach dem Scannen also nicht
erhöhen, und eine Seite kann das Geld nicht heimlich umleiten.

Wenn du zahlst, bestätige genau das, was in der Rechnung steht. Widerspricht das Wallet-Fenster dem
Bildschirm, den du gescannt hast, halt an und frag nach — eines von beiden ist nicht das, was du
denkst.

## Gewohnheiten, die sich lohnen

- **Signiere eines nach dem anderen.** Eine Warteschlange von Bestätigungen ist dazu gebaut, dich mit
  dem Lesen aufhören zu lassen.
- **Signiere nicht auf einem Gerät, das du nicht kontrollierst.** Gemeinschaftscomputer und
  Fernwartungssitzungen sind der Weg, auf dem „hilfsbereite“ Fremde Wallets leeren.
- **Für größere Beträge lieber ein Hardware-Wallet.** Es zeigt Betrag und Empfänger auf seinem eigenen
  Bildschirm, ein kompromittierter Computer kann sie also nicht hinter deinem Rücken tauschen.
- **Im Zweifel ablehnen.** Ablehnen kostet nichts, und die Zahlung lässt sich in einer Minute
  wiederholen.

Wurde etwas signiert, das nicht hätte sein sollen, ist [die Checkliste für ein verlorenes
Handy](/learn/if-your-phone-is-lost) dieselbe Abfolge: bewegen, was übrig ist, den alten Schlüssel
stoppen und es melden.
