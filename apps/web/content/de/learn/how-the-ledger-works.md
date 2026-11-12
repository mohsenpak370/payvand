---
title: Wie das Register geschrieben und geteilt wird
description: Slots, Shreds, Merkle-Wurzeln, Fehlerkorrektur und eine Mempool, die es nicht gibt. Die Mechanik zwischen „signiert“ und „alle sind sich einig“.
category: technical
order: 3
level: technical
---

Eine signierte Transaktion ist nur eine Behauptung, bis ein Blockproduzent sie aufnimmt. Diese Seite
handelt von der Installation, die aus Behauptungen eine geteilte, überprüfbare Geschichte macht.

## Slots und Blöcke

Die Zeit auf Solana ist in **Slots** von **200 ms** geteilt, jeder mit einem zugewiesenen Leader, und
Leader werden eine ganze Epoche im Voraus eingeplant. Ein Slot ist eine Gelegenheit, einen Block zu
erzeugen, keine Garantie: Slots können ausgelassen werden, und ein Leader kann mehr als einen
Kandidatenblock erzeugen.

Praktisch heißt das: „der aktuelle Block“ ist für ein paar hundert Millisekunden unscharf. Transaktionen
warten nicht in einer Mempool auf Aufnahme — es gibt keine Mempool. Sie werden an den Leader
weitergeleitet, der den nächsten Slot erzeugen soll, und landen entweder in diesem Block oder laufen
ab.

## Warum Blöcke als Shreds reisen

Ein Block ist zu groß, um ihn jedem Validator direkt zu geben. Der Leader **shredded** ihn:

- Daten-Shreds tragen Bruchstücke serialisierter Einträge, je etwa auf 1.280 Bytes begrenzt.
- Coding-Shreds entstehen mit **Reed-Solomon-Fehlerkorrektur** über Gruppen von Daten-Shreds, bekannt
  als FEC-Sets. Ein Set kann bis zu 67 Daten-Shreds halten; 32 ist die empfohlene Konfiguration.
- Alle Shreds werden vom Blockproduzenten signiert.
- Jedes FEC-Set wird merkelisiert: Die Blätter werden paarweise bis zu einer **Merkle-Wurzel** gehasht,
  und der Leader signiert diese Wurzel. Weil ein Shred einen Beweispfad trägt, kann jeder Knoten ein
  Teil prüfen, ohne den ganzen Block zu haben, und fehlende Teile lassen sich aus den Coding-Shreds
  rekonstruieren.

## Turbine, der Verteilbaum

Turbine verteilt diese Shreds über einen stake-gewichteten Baum statt an alle gleichzeitig. Der Leader
sendet an einen Wurzelknoten, der zu Ebene 1 auffächert, deren Knoten zu Ebene 2 auffächern, und so
weiter. Die Wurzel rotiert **bei jedem Shred**, und die Peer-Liste wird pro Shred neu gemischt, um
gezielte Angriffe zu erschweren.

Zwei andere Rollen werden leicht damit verwechselt:

- **Gossip** ist die Steuerungsebene: Peer-Erkennung, Kontaktinformationen und Validator-Metadaten. Es
  trägt keine Transaktionen.
- **Gulf Stream** ist der eingehende Weg: Clients und Validatoren leiten Transaktionen an den
  kommenden Leader weiter, den sie bereits kennen. Deshalb braucht Solana keine Mempool und deshalb muss
  der Blockhash eines Clients aktuell sein.

## Proof of History, nach Alpenglow

PoH war eine überprüfbare Verzögerung: eine Kette von Hashes, mit der jeder die Reihenfolge und den
Verlauf der Zeit prüfen konnte, ohne einer Uhr zu vertrauen. Jahrelang hat sie auch die
Blockproduktion **getaktet**.

Diese Taktrolle ist weg. Seit Alpenglow entscheiden Votors Timeouts und Zertifikate, wann Blöcke
endgültig werden, und die Tick-Einträge, die den PoH-Takt trugen, haben den Shred-Strom verlassen. Die
Hash-Konstruktion von PoH existiert weiter in der Geschichte des Registers, aber sie ist nicht mehr das,
was das Netzwerk antreibt.

## Gabelungen, kurz

Weil Slots ausgelassen werden können und Leader konkurrierende Blöcke erzeugen können, kann das Netzwerk
kurzzeitig mehr als eine Sicht auf die jüngste Geschichte halten. Validatoren laufen auf einen Zweig
zusammen, und seit Alpenglow ist die Finalisierung ein ausdrückliches Zertifikat statt der
angesammelten Lockouts von TowerBFT.

Was für eine App zählt: Eine Transaktion in einem Block ist *processed*, also nicht endgültig. Nur ein
finalisierter Block ist unumkehrbar. Auf modernem Solana löst sich dieser Unterschied in etwa 150 ms
auf, aber er bleibt der Unterschied zwischen „wahrscheinlich“ und „sicher“.

## Was das erklärt

- **Warum es ein Limit von 1.232 Bytes pro Transaktion gibt**: Es ist ein Netzwerkpaket, und ein Block
  ist ein Strom von Paketen.
- **Warum eine Zahlung gelegentlich mit einem Blockhash-Fehler scheitert**: Die Transaktion hat ihr
  Gültigkeitsfenster von etwa 150 Slots überlebt, bevor jemand sie aufnahm.
- **Warum Endgültigkeit eine Konsenseigenschaft ist und keine Netzwerkeigenschaft**: Verbreitung ist,
  wie ein Block sich ausbreitet, Konsens ist, wie er dauerhaft wird. Es sind getrennte Systeme, und
  genau deshalb konnte Votor den Konsens ersetzen, ohne zu berühren, wie Blöcke reisen.

## Quellen

- [Anza: Turbine block propagation](https://docs.anza.xyz/consensus/turbine-block-propagation)
- [Shred specification](https://raw.githubusercontent.com/solana-foundation/specs/main/p2p/shred.md)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
- [Alpenglow: PoH and tick entries](https://solana.com/upgrades/alpenglow)
- [Gulf Stream](https://solana.com/news/8-innovations-that-make-solana-the-first-web-scale-blockchain)
