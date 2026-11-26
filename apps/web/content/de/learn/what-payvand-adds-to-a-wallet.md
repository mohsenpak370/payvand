---
title: Was Payvand einer Wallet hinzufügt
description: Eine Wallet kann schon einen QR-Code zeigen und USDC senden. Was sie dir überlässt, ist das Prüfen – falsche Coins mit echten Namen, Anfragen, die mehr wollen als eine Zahlung, und die Frage, zu welchem Verkauf eine Überweisung gehört.
category: payvand
order: 0
---

Eine Wallet wie Phantom oder Solflare kann schon USDC, EURC oder SOL senden, und die meisten können
einen QR-Code mit Betrag anzeigen. „Es macht QR-Codes“ ist also kein Grund für Payvand. Selbst das
Geld bewegt sich genauso: direkt von der Wallet des Käufers zu der des Verkäufers, nie über uns.

Der Unterschied liegt in allem rund um die Überweisung. Eine Wallet bewegt Geld und überlässt das
Prüfen den beiden Menschen, die sie benutzen. Payvand übernimmt dieses Prüfen. Das heißt konkret:

## Nur der echte Coin zählt

Jeder kann in ein paar Minuten einen neuen Coin erstellen, ihn „USDC“ nennen und ihm dasselbe Logo
geben. In der Wallet sieht er genau aus wie der echte. Wert ist er nichts.

Sicher unterscheiden lassen sie sich nur an der **Mint-Adresse** des Coins: eine Zeichenkette aus 44
Buchstaben und Ziffern, die den Coin kennzeichnet wie eine Seriennummer. Der echte USDC auf Solana
beginnt mit `EPjFWdd5…`, ein falscher kann mit allem beginnen. Um sicher zu sein, dass du echtes Geld
bekommen hast, müsstest du jedes Mal die Überweisung öffnen und diese Zeichenkette Zeichen für Zeichen
vergleichen. Das macht fast niemand, und genau deshalb funktioniert der Betrug.

Mit Payvand musst du das nie. Die echten Adressen von USDC und EURC, aus Circles eigener
veröffentlichter Liste, sind in Payvand fest eingebaut, und der Coin ist Teil der Adresse, die die
Zahlung erreichen muss. Eine Zahlung in einem Doppelgänger-Coin geht an eine andere Adresse, die
dieser Code nicht überwacht, und die Rechnung bleibt **unbezahlt**. Steht dort Bezahlt, ist der echte
Coin angekommen. Mehr dazu, wie Coins wie USDC funktionieren, in
[Stablecoins, einfach erklärt](/learn/stablecoins-explained).

## Der Kunde bestätigt eine einfache Zahlung

Jede Krypto-Zahlung endet mit einem Fenster in der Wallet, das den Kunden um Bestätigung bittet.
Meist ist das in Ordnung. Aber eine Signatur kann mehr als Geld senden. Sie kann einem Programm auch
eine **dauerhafte Erlaubnis** geben, später Geld zu bewegen, und genau so arbeiten „Wallet-Drainer“:
Eine Anfrage, die harmlos aussieht, lässt jemand anderen die Wallet danach leer räumen.

Bei einer einfachen Wallet-Überweisung muss der Kunde außerdem Adresse und Betrag selbst eintippen
oder einfügen, und ein falsches Zeichen schickt das Geld für immer woandershin.

Eine Payvand-Anfrage ist immer nur eines: **zahle diesen Betrag, in diesem Coin, an diesen
Verkäufer**. Betrag, Coin und Empfänger sind aus dem Code schon ausgefüllt, es gibt also nichts zu
tippen, einzufügen oder falsch zu lesen, und keine zusätzliche Erlaubnis ist darin versteckt. Was der
Kunde in der Wallet sieht, ist das, was passiert.

Zwei ehrliche Grenzen:

- Payvand hält die eigene Anfrage einfach und klar. Es kann nicht verhindern, dass ein Kunde auf einer
  anderen Seite etwas Schädliches signiert. Die Gewohnheit aus [Lies, bevor du signierst](/learn/checking-before-you-sign)
  bleibt also wichtig, ebenso die Warnzeichen in [Krypto-Betrug erkennen](/learn/spotting-crypto-scams).
- Das optionale [Monatsbudget](/learn/monthly-allowance-explained) ist bewusst eine Erlaubnis. Der
  Kunde öffnet es selbst, mit einem Limit, das er festlegt; es kann nur Rechnungen bezahlen und
  jederzeit geschlossen werden.

## Preis und Währung können sich nicht ändern

Der Verkäufer legt Preis, Währung und eine kurze Notiz einmal fest, wenn er den Code erstellt. Der
Betrag ist Teil der Zahlungsadresse selbst. Ein Kunde kann also nicht weniger zahlen, und eine Seite
kann den Preis nach dem Scannen nicht ändern. Ein anderer Betrag ginge an eine Adresse, die dieser
Code nicht überwacht. [So läuft eine Zahlung aus der Wallet](/learn/paying-with-a-wallet) zeigt es
Schritt für Schritt.

## Der Verkäufer weiß, wer gezahlt hat und wofür

Eine Wallet zeigt eingehendes Geld als Liste von Beträgen von fremden Adressen. Sie Verkäufen
zuzuordnen geht nur mit dem Auge: Welche 4,50 waren der Kaffee, welche das Croissant?

Ein Payvand-Code überwacht seine eigene Zahlung. Er springt von selbst auf **Bezahlt**, sobald das
Geld ankommt, auf dem Bildschirm des Verkäufers und auf dem des Kunden, und bewahrt die Adresse des
Zahlers und die Transaktion als Beleg. Es gibt keinen „Schau, ich hab’s geschickt“-Screenshot, dem
man glauben muss. Ein Screenshot lässt sich fälschen, ein Code, der auf Bezahlt gesprungen ist, nicht.

## Der Kunde braucht kein SOL

USDC aus einer Wallet zu senden braucht ein wenig SOL für die Netzwerkgebühr. Ein Kunde mit viel USDC
und ohne SOL kann gar nicht zahlen, was die meisten beim ersten Mal überrascht. Bei USDC oder EURC über
Payvand zahlt unser Relayer die Netzwerkgebühr und wird aus der Gebühr im Preis erstattet. Der Kunde
braucht also nur den Coin, in dem er zahlt. [Was eine Zahlung kostet](/learn/what-a-payment-costs)
zeigt, wohin jeder Teil davon geht.

## Und es bleibt deine Wallet

Für all das muss Payvand nichts verwahren. Es gibt kein Payvand-Guthaben und keine Seed-Phrase, die du
aufbewahren musst, und die App hat keinen Schlüssel, der dein Geld bewegen kann. Jede Zahlung geht
direkt in die eigene Wallet des Verkäufers. Payvand ist die Schicht über der Wallet, der du schon
vertraust: Es fügt die Prüfungen und die Buchführung hinzu, und das Geld bleibt, wo es war.
