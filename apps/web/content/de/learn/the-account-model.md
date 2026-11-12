---
title: Konten, Eigentum und PDAs
description: Auf Solana ist alles ein Konto mit fünf Feldern und einem Eigentümer. Warum Programme zustandslos sind, warum ein PDA keinen privaten Schlüssel hat und wo ein Token-Guthaben wirklich liegt.
category: technical
order: 2
level: technical
---

Solana hat kein globales Zustandsobjekt und keinen Speicher pro Vertrag. Es gibt **Konten**, und jedes
Datum, an das sich das Netzwerk erinnert, ist eines.

## Die fünf Felder

Jedes Konto hängt an einer 32-Byte-Adresse und hat dieselbe Form:

| Feld | Bedeutung |
|---|---|
| `lamports` | Guthaben in Lamports (1 SOL = 1.000.000.000) |
| `data` | Beliebige Bytes — Programmzustand oder Programmcode |
| `owner` | Das Programm, das `data` ändern oder `lamports` abbuchen darf |
| `executable` | Ob dieses Konto ein Programm enthält |
| `rent_epoch` | Veraltet; von der Miete befreite Konten nutzen `u64::MAX` |

Zwei Regeln erklären den größten Teil von Solanas Design:

1. **Nur das Eigentümerprogramm darf die Daten eines Kontos ändern oder seine Lamports abbuchen.**
   Jeder darf Lamports einem beschreibbaren Konto *gutschreiben*, so funktioniert eine Zahlung, ohne
   dass das Programm des Empfängers beteiligt ist.
2. **Die Größe eines Kontos steht bei der Anlage fest.** Es zu vergrößern heißt, mit genug Lamports
   für die Mietbefreiung neu zu allokieren, bis zu einem Maximum von **10 MiB** pro Konto.

## Miete, und warum es eine Einlage gibt

Daten zu speichern kostet Validatoren echte Ressourcen, ein Konto muss also ein rückzahlbares Guthaben
halten: `(data_size + 128) × 3.480 Lamports × 2 Jahre`. Ein klassisches Token-Konto mit 165 Bytes
braucht **2.039.280 Lamports** (≈0,00204 SOL); eine Mint mit 82 Bytes **1.461.600**.

Laufende Mietzahlungen sind abgeschaltet; das ist heute eine rückzahlbare Speichereinlage. Schließ das
Konto, und die Lamports kommen zurück. Für den aktuellen Wert frag
`getMinimumBalanceForRentExemption`.

## Programme sind zustandslos

Ein Programm ist ein Konto mit `executable = true`, geschrieben in einem Bytecode, den die
Laufzeitumgebung ausführen kann (sBPF). Es läuft nur über die Konten, die ihm eine Anweisung übergibt,
und kann nur jene ändern, die ihm gehören.

Deshalb gibt es hier keinen „Smart-Contract-Speicher“. Der Speicher eines Programms ist die Menge der
übergebenen Konten; sein dauerhafter Zustand liegt in separaten **Datenkonten**, die dem Programm
gehören. Wenn man sagt, ein Programm sei zustandslos, ist genau das gemeint.

Programme werden über ein Loader-Programm ausgeliefert und aktualisiert, dem das ausführbare Konto
gehört; die Upgrade-Befugnis ist ein eigener Schlüssel, und sie zu verlieren heißt, die Möglichkeit zu
verlieren, das Programm zu reparieren.

## PDAs, Adressen, für die niemand signieren kann

Eine **program derived address** wird aus `seeds + program_id + bump` berechnet und dann geprüft, ob sie
**außerhalb der Ed25519-Kurve** liegt. Bis zu 16 Seeds, jeder höchstens 32 Bytes.

Off-curve ist der Punkt: Zu einer solchen Adresse gehört kein privater Schlüssel, es kann also nie eine
gewöhnliche Signatur dafür geben. Stattdessen übergibt das ableitende Programm während eines Aufrufs
in ein anderes Programm die Seeds zurück, und die Laufzeitumgebung prüft, dass sie zu dem PDA hashen,
und gewährt ihm Signer-Rechte **nur für diesen Aufruf**. Das ist `invoke_signed`.

So kontrolliert ein Programm Geld, ohne einen Schlüssel zu halten. Die Budgets von Payvand
funktionieren so: Der Topf ist ein PDA, der dem Programm gehört, also hält weder eine Person noch ein
Server einen Schlüssel dazu, und Ausgeben verlangt, dass die eigenen Regeln des Programms erfüllt sind.

Beachte die zwei Schritte, die oft verwechselt werden: Eine Adresse *abzuleiten* ist reine Arithmetik,
ein Konto dort *anzulegen* kostet Lamports und eine Anweisung des System-Programms.

## Wo ein Token-Guthaben wirklich liegt

Es gibt drei verschiedene Dinge:

- **Ein Mint-Konto** beschreibt ein Token: Umlauf, `decimals` und optionale Mint- und
  Freeze-Befugnisse. Beträge werden in Basiseinheiten gespeichert; Nachkommastellen betreffen nur die
  Anzeige.
- **Ein Token-Konto** hält Einheiten genau einer Mint. Sein eigenes `owner`-Feld nennt die Befugnis,
  die sie bewegen darf. Das Konto selbst gehört dem Token-Programm (oder Token-2022).
- **Eine Wallet-Adresse** ist meist nur ein on-curve-Schlüsselpaar-Konto, das SOL hält. Dort liegt dein
  USDC nicht.

Das **associated token account** (ATA) ist das übliche Token-Konto für eine Kombination aus Wallet,
Mint und Token-Programm, deterministisch abgeleitet, sodass jeder es finden kann. Das Token-2022-
Programm erweitert dieses Modell um optionale Konto- und Mint-Erweiterungen, die die Kontogröße ändern
und Funktionen wie Überweisungsgebühren oder Zinsen hinzufügen.

## Warum Schreib-Flags den Durchsatz bestimmen

Die Laufzeitumgebung nutzt **Sealevel**, um Transaktionen parallel auszuführen. Transaktionen, die ein
Konto nur *lesen*, teilen es frei; Transaktionen, die dasselbe Konto *schreiben*, müssen serialisiert
werden. Ein heißes beschreibbares Konto — etwa ein globaler Zähler — ist deshalb ein Engpass im
Durchsatz, aufgeteilte Datenkonten sind es nicht.

Das ist eine Designvorgabe, kein Tuning-Detail. Deshalb wird Zustand pro Nutzer, pro Mint und pro Monat
aufgeteilt statt in einem großen Konto gehalten.

## Quellen

- [Accounts](https://solana.com/docs/core/accounts) · [Account structure](https://solana.com/docs/core/accounts/account-structure) · [Modification rules](https://solana.com/docs/core/accounts/modification-rules)
- [Programs](https://solana.com/docs/core/programs) · [Builtin programs](https://solana.com/docs/core/programs/builtin-programs)
- [PDAs](https://solana.com/docs/core/pda) · [PDA derivation](https://solana.com/docs/core/pda/pda-derivation) · [CPI](https://solana.com/docs/core/cpi)
- [Tokens](https://solana.com/docs/tokens) · [Token accounts](https://solana.com/docs/tokens/basics/create-token-account) · [Token-2022 extensions](https://solana.com/docs/tokens/extensions)
- [Anza: read-only accounts and Sealevel](https://docs.anza.xyz/implemented-proposals/readonly-accounts)
