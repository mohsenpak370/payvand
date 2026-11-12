---
title: Was wirklich in einer Solana-Transaktion steckt
description: Signaturen, eine Nachricht, Konto-Flags, ein Blockhash mit Ablaufdatum, eine Gebühr in Mikro-Lamports und eine Box von 1.232 Bytes. Die Anatomie, mit den echten Konstanten.
category: technical
order: 1
level: technical
---

Eine Transaktion ist keine Anfrage an einen Server. Sie ist ein signiertes Objekt, das entweder
vollständig ausgeführt wird oder gar nichts ändert. Hier steht, was darin ist und warum jeder Teil
existiert.

## Zwei Hälften: Signaturen und eine Nachricht

Eine Solana-Transaktion besteht aus `signatures` plus einer `message`.

- Die **message** enthält einen Kopf, die Liste der berührten Konto-Adressen, einen aktuellen
  Blockhash und eine oder mehrere kompilierte Anweisungen.
- Die **signatures** gelten über die serialisierte message. Die erste gehört dem Zahler der Gebühr,
  und ihre Signatur ist die ID der Transaktion.

Weil die Signaturen die ganze message abdecken, macht jede Änderung an einer Kontoliste, einer
Anweisung oder am Blockhash sie ungültig. Das ist die gesamte Manipulationssicherheit auf dieser
Ebene.

## Die Box, in die es passen muss

- Legacy- und v0-Transaktionen sind auf **1.232 Bytes** begrenzt — die minimale IPv6-MTU (1.280)
  minus 48 Bytes Kopfzeilen. Kein Designschmuck: Es ist das, was in ein Netzwerkpaket passt.
- Eine Transaktion trägt höchstens **12 Signaturen**.
- Sie kann höchstens **64 Konten** referenzieren.
- Das neuere **v1**-Format hebt das Limit auf **4.096 Bytes**.

Meist stößt man zuerst ans Byte-Limit, nicht an die Kontozahl. Eine Legacy-message verbraucht 32 Bytes
pro Konto, ein paar Anweisungen mit vielen Konten sprengen 1.232 Bytes also weit vor 64 Konten.

## Versionierte Nachrichten und Lookup-Tabellen

**v0**-Nachrichten können auf eine **Address Lookup Table** zeigen: eine On-Chain-Liste mit bis zu 256
Public Keys. Die Nachricht referenziert diese Konten dann über einen Ein-Byte-Index, statt jeden
32-Byte-Schlüssel mitzuführen.

Der Kompromiss ist Absicht: Konten aus Lookup-Tabellen dürfen beschreibbar oder nur lesbar sein, aber
**keine Signer**. Signer und der Zahler der Gebühr bleiben immer direkt in der Nachricht, weil eine
Signatur den Schlüssel abdecken muss, der sie erzeugt hat.

## Anweisungen und Konto-Metadaten

Jede Anweisung nennt ein Programm und trägt:

- eine geordnete Liste von **Konten**, jeweils mit `is_signer`- und `is_writable`-Flag, und
- einen undurchsichtigen **data**-Block, dessen Aufbau das Programm definiert.

Programme können während der Ausführung andere Programme aufrufen; das sind **innere Anweisungen**,
die in Metadaten festgehalten, aber nie als Anweisungen oberster Ebene serialisiert werden.

Die Signer- und Writability-Flags sind keine Hinweise. Die Laufzeitumgebung erzwingt sie: In ein Konto
zu schreiben, das nicht als beschreibbar markiert ist, scheitert, und ein Konto als Signer zu
verwenden, ohne passende Signatur, scheitert ebenfalls.

## Der Blockhash und sein Ablauf

Jede Transaktion referenziert einen aktuellen Blockhash. Er hat zwei Aufgaben: Er verhindert
Wiedereinspielung und gibt der Transaktion eine Frist. Ein Blockhash ist **150 Slots** lang gültig
(151 Alter, wegen Null-Indizierung).

Bei der aktuellen Slot-Zeit von **200 ms** sind das etwa **30 Sekunden** — allerdings kann die
Slotproduktion springen oder nachhängen, deshalb verfolgen Clients `lastValidBlockHeight` aus
`getLatestBlockhash`, statt einen eigenen Timer zu setzen.

## Gebühren: eine Grundgebühr und ein Trinkgeld

- **Grundgebühr:** 5.000 Lamports pro Signatur, also 0,000005 SOL für eine typische Zahlung mit einer
  Signatur. Sie wird 50/50 zwischen Verbrennung und Validator aufgeteilt und **auch dann berechnet,
  wenn die Ausführung scheitert**.
- **Prioritätsgebühr:** bestimmt durch einen Compute-Unit-Preis in **Mikro-Lamports pro CU**, wobei
  die Summe `ceil(Preis × angefragtes CU-Limit / 1.000.000)` beträgt. Beachte *angefragt*: Du zahlst
  für das Limit, das du verlangst, nicht für den Compute, den du tatsächlich nutzt. Sie geht
  vollständig an den Validator.

## Compute: das eigentliche Budget

- Eine nicht eingebaute Anweisung bekommt standardmäßig **200.000 Compute Units**; eine eingebaute
  3.000.
- Die Obergrenze pro Transaktion liegt bei **1.400.000 CU**.
- Das Limit des Schedulers pro Block liegt bei **60.000.000 CU** — eine Kapazitätszahl, keine
  Erlaubnis pro Transaktion.

Programme können mit den Compute-Budget-Anweisungen ein anderes Limit anfordern, und gegen diese
Anfrage wird die Prioritätsgebühr berechnet.

## Simulation, Preflight und was „gesendet“ heißt

`sendTransaction` prüft die Signaturen und **simuliert** die Transaktion gegen eine
Commitment-Stufe, bevor es sie weiterleitet, außer `skipPreflight` ist gesetzt. Ein fehlschlagender
Preflight ist ein starkes Signal, dass die Transaktion on-chain scheitern würde — deshalb simulieren
Apps, bevor sie ein Wallet-Fenster zeigen.

Ein erfolgreicher Preflight ist aber keine Bestätigung. `sendTransaction` gibt eine Signatur zurück,
sobald ein RPC die Einreichung annimmt. Wirklich ist die Transaktion erst, wenn sie in einem Block
steht und endgültig ist.

## Commitment nach Alpenglow

Seit Alpenglow am **21. Oktober 2026 in Epoche 1064** im mainnet aktiviert wurde, bedeuten `confirmed`
und `finalized` beide dauerhaft festgeschrieben; `processed` heißt in einer Bank ausgeführt, aber noch
nicht endgültig. Die Upgrade-Hinweise von Solana empfehlen neuem Code, `finalized` zu nutzen, und
sagen, `confirmed` sei für eine spätere Version zur Entfernung vorgesehen. Was das an einer Theke
ändert, steht in [der Erklärung zur Endgültigkeit](/learn/when-is-a-payment-final).

## Quellen

- [Transaction structure](https://solana.com/docs/core/transactions/transaction-structure)
- [Versioned transactions](https://solana.com/docs/core/transactions/versioned-transactions)
- [Constants reference](https://solana.com/docs/core/constants-reference)
- [Fee structure](https://solana.com/docs/core/fees/fee-structure) · [Compute budget](https://solana.com/docs/core/fees/compute-budget)
- [sendTransaction](https://solana.com/docs/rpc/http/sendtransaction) · [simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
