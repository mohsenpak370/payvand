---
title: Was ist eine Blockchain?
description: Ein gemeinsames Heft mit tausenden Kopien, in dem eine geänderte alte Zeile dich verrät. Deshalb lässt sich eine Krypto-Zahlung nicht heimlich rückgängig machen.
category: basics
order: 2
---

Vergiss das Wort „Blockchain“ für einen Moment. Stell dir ein Heft vor.

Jedes Mal, wenn jemand jemandem etwas zahlt, wird eine neue Zeile ins Heft geschrieben: *diese
Adresse hat so viel an jene Adresse geschickt*. Tausende Menschen halten eine identische Kopie
desselben Hefts, und jede neue Seite muss von allen geprüft werden, bevor sie zählt.

Das ist eine Blockchain. „Block“ heißt einfach Seite. „Chain“ heißt, jede Seite trägt einen
Fingerabdruck der Seite davor, sodass die Seiten in ihrer Reihenfolge fest verschlossen sind.

## Warum niemand das Heft betrügen kann

Jede Seite ist mit einem langen Code gestempelt — einem **Fingerabdruck** von allem, was darauf
steht. Ändere ein einziges Zeichen auf einer Seite, und ihr Fingerabdruck ändert sich vollständig.
Dann passt der Fingerabdruck der nächsten Seite nicht mehr, und der übernächsten auch nicht.

Wenn also jemand heimlich eine alte Zeile in seiner Kopie ändert, geht ab da keine Seite mehr auf.
Die anderen Kopien widersprechen, die Änderung wird abgelehnt, und der Betrüger ist derjenige, der
falsch dasteht.

Um die Geschichte umzuschreiben, müsstest du jede Seite seither neu machen, auf mehr als der Hälfte
aller Kopien gleichzeitig, und schneller als die ehrlichen Kopien neue Seiten anfügen. In einem
ernsthaften Netzwerk ist das keine praktische Absicht, sondern eine Fantasie.

## Was „on-chain“ bedeutet

**On-chain** heißt in diesem Heft eingetragen: für alle sichtbar, danach unveränderbar.
**Off-chain** heißt in der privaten Datenbank von jemandem festgehalten — einer Bank, einer
Ladenkasse, dem Server einer App — wo man es ändern kann.

Wenn ein Laden sagt, deine Zahlung sei „on-chain“, heißt das: Das Heft hat die Zeile, und der Laden
verlässt sich nicht auf jemandes Wort.

## Warum eine Zahlung endgültig ist

Es gibt keine Rückgängig-Taste im Heft, und wichtiger noch: niemanden mit der Befugnis, sie zu
drücken. Es gibt keine Firma zum Anrufen, keine Kartenausgeberin für einen Widerspruch, keine Bank,
die das Geld zurückholen kann.

Das schneidet in beide Richtungen. Ein Laden muss nie befürchten, dass eine Zahlung Wochen später
zurückgedreht wird — deshalb können Krypto-Zahlungen günstig sein, statt Versicherungskosten zu
tragen. Und du bekommst dein Geld nicht zurück, wenn du an die falsche Adresse geschickt hast oder
jemand dich reingelegt hat. Die [Sicherheitsanleitungen](/learn/checking-before-you-sign) handeln
genau von dieser Hälfte des Geschäfts.

## Wer ins Heft schreibt

Niemand besitzt es, wer macht dann die Arbeit des Anfügens? Auf Solana sind es Computer, die
**Validatoren** heißen, sie sind abwechselnd dran. Sie bekommen die kleine Gebühr, die an jeder
Zahlung hängt, und haben eigenes Geld als Pfand hinterlegt, das sie verlieren, wenn sie betrügen.
Die nächste Anleitung erklärt das Netzwerk selbst: [Was ist Solana?](/learn/what-is-solana)

## Wer es lesen kann

Jeder. Guthaben und Zahlungsverläufe sind öffentlich — eine Adresse, kein Name, aber alles sichtbar
für jeden, der hinschaut. Das ist nützlich (ein Laden kann auf eine Zahlung warten, ohne dass sie
ihm gesagt wird) und gut zu merken (eine öffentlich benutzte Adresse ist eine sehr öffentliche
Adresse).

## Was das für dich bedeutet

Eine Blockchain ist ein Beleg, den niemand fälschen und niemand zurücknehmen kann. An einer
Ladentheke ist das sehr viel wert, und es heißt: Der Moment, bevor du eine Zahlung bestätigst, ist
der einzige, in dem jemand sie noch stoppen kann. Auf Solana folgt diesem Moment in deutlich unter
einer Sekunde ein „fertig“ — [hier steht, was Finalität bedeutet](/learn/when-is-a-payment-final).
