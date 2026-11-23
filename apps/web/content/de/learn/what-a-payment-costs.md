---
title: Was eine Zahlung kostet, und warum
description: Wohin das Geld bei einer Zahlung mit Payvand geht — Netzwerkgebühr, Protokollgebühr, der dauerhafte Beleg auf der Blockchain und die Kosten, die den Dienst am Laufen halten.
category: payvand
order: 3
---

Nichts davon ist kostenlos im Betrieb, und fast nichts an einer Zahlung ist Gewinn. Diese Seite zeigt
jeden Kostenposten und wer ihn trägt, damit die Zahl auf dem Bildschirm nie überrascht.

## Was der Kunde zahlt

Genau den Preis auf dem Code, plus die Netzwerkgebühr von Solana für die Transaktion — Bruchteile
eines Cents, an das Netzwerk selbst und nicht an uns. Es gibt keinen Aufschlag, keine Servicegebühr
und keine Bearbeitungsgebühr. Steht auf einem Code 4 EURC, zeigt die Wallet 4 EURC.

## Was der Verkäufer zahlt

Zwei Dinge, beide vom Betrag abgezogen statt aufgeschlagen, und beide sichtbar, bevor ein Code
geteilt wird:

1. **Die Protokollgebühr**, ein Prozentsatz des Preises. Sie ist gestaffelt statt pauschal: ein Satz
   gilt für den ersten Teil des Preises, ein niedrigerer für alles über einer Grenze, damit eine
   große Rechnung pro Einheit nie teurer ist als eine kleine. Die Grenze unterscheidet sich für SOL
   und für Stablecoins, weil das nicht dieselbe Art von Geld ist. Dazu kommt eine Obergrenze: eine
   Zahlung zahlt davon nie mehr als 0,1 SOL, 10 USDC oder 10 EURC, sodass eine sehr große Rechnung eine
   Gebühr in der Größe eines Satzes zahlt statt eines Anteils an sich selbst, der immer weiter wächst.
2. **Die Beleggebühr** bei Zahlungen in einem Token. Jede abgerechnete Zahlung hinterlässt einen
   kleinen, dauerhaften Beleg auf der Blockchain. Platz dort wird im Voraus in SOL bezahlt und lässt
   sich nie zurückholen. Die Beleggebühr deckt diesen Posten, und sie wird zum aktuellen SOL-Kurs
   berechnet, folgt also dem, was der Platz heute kostet, statt dem, was SOL wert war, als die Regel
   geschrieben wurde. Sie kauft eine feste Menge Platz und ist deshalb bei einer kleinen Rechnung ein
   größerer Anteil. Eine SOL-Rechnung trägt sie nicht separat, weil der Verkäufer diese Miete ohnehin
   aus dem Preis abgibt.

Ist eine Zahlung so klein, dass der Prozentsatz unter dieser Beleggebühr liegt, wird der Prozentsatz
gar nicht erhoben: Der Verkäufer zahlt den Beleg und sonst nichts. Das Protokoll nimmt bei so einer
Zahlung keine Gebühr, statt einer, die den eigenen Beleg nicht deckt.

Der Bildschirm des Verkäufers zeigt den genauen Betrag, den er erhält, und die Aufteilung der Gebühr
in diese zwei Teile — bevor ein Kunde den Code sieht.

## Was der Betrieb uns kostet

Eine Zahlung geht von Wallet zu Wallet. Wir halten das Geld nie, haben also keine Einlage, an der wir
verdienen. Wir haben eine Rechnung:

- **Server**: die Website und die API, die Codes baut, Zahlungsstatus liest und Shops ausliefert.
- **Eine Datenbank**: Konten, Shops, Produkte, gespeicherte Codes und Abos.
- **Zugang zum Netzwerk**: der RPC-Dienst, von dem die App Kontostände und Zahlungsstatus liest — bei
  echtem Volumen eine bezahlte Infrastruktur.
- **Preisinformationen**: Wechselkurse, damit ein Euro-Preis zu einem fairen Kurs in SOL oder Dollar
  bezahlbar ist.
- **E-Mail**: die wenigen Nachrichten eines Kontos — Anmeldehinweise, Passwort-Zurücksetzen.
- **Überwachung, Backups und Sicherheitsarbeit**: die weniger glänzende Hälfte jedes Dienstes, der
  Geld berührt.
- **Menschen**: bauen, reparieren, Fragen beantworten.

Das Shop-Abo und die Protokollgebühr decken einen Teil davon. Die Gebühr ist nicht auf maximalen
Ertrag ausgelegt, sondern darauf, den Dienst am Leben zu halten, während eine Zahlung günstiger bleibt
als ein Kartenterminal.

## Warum die Gebühr zwei Teile hat

Ein Prozentsatz allein deckt keine festen Kosten. Der Beleg auf der Blockchain kostet gleich viel, ob
die Zahlung eine Einheit oder tausend betrifft. Nur ein Prozentsatz wäre bei kleinen Zahlungen zu
klein und bei großen ein unnötig großer Anteil. Die Aufteilung — ein Prozentsatz für die Arbeit, eine
feste Beleggebühr für den Platz, und der Beleg als Untergrenze für beides — hält kleine Zahlungen
möglich und große günstig.

## Was sich ändern kann

Sätze und Beleggebühr legt die Plattform fest; sie folgen den Kosten des Betriebs, und die
Beleggebühr wird zum aktuellen SOL-Kurs neu berechnet, statt eine SOL-Bewegung selbst zu tragen. Der
Preis eines
Codes ändert sich nie, die Gebühr ist aber die, die beim Bezahlen gilt — eine Änderung trifft also
auch schon geteilte, noch unbezahlte Codes. Die App zeigt die Aufteilung zu den gerade gültigen
Sätzen.
