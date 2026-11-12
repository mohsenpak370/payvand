---
title: Stablecoins, einfach erklärt
description: Warum ein Laden Preise in USDC oder EURC auszeichnen kann, ohne dass ihm der Preis des Tokens davonläuft, und was sie wirklich deckt.
category: basics
order: 5
---

Das meiste Krypto, von dem man hört, hat einen Preis, der den ganzen Tag schwankt. Für einen Kaffee
ist das eine furchtbare Eigenschaft: Wenn der Laden dir einen SOL berechnet und der Preis von SOL vor
deiner Zahlung um 5 % fällt, hat der Laden gerade 5 % verloren.

Ein **Stablecoin** ist ein Token, das immer eine Einheit einer gewöhnlichen Währung wert sein soll.
Die zwei, die du auf Payvand siehst, sind **USDC** (je ein Dollar) und **EURC** (je ein Euro), beide
von Circle ausgegeben. Sie leben im Solana-Netzwerk, das die Installation unter jeder Zahlung hier
ist — [was Solana ist, einfach gesagt](/learn/what-is-solana).

## Was „gedeckt“ heißt

Für jeden USDC im Umlauf hält Circle etwa einen Dollar in Bargeld und kurzfristigen Staatsanleihen
als Reserve und veröffentlicht regelmäßig Prüfberichte dazu. EURC funktioniert genauso gegenüber dem
Euro. Du kannst USDC an Circle zurückgeben und Dollar erhalten — das hält den Preis am Dollar
fest: Würde er bei 97 Cent gehandelt, würde jemand ihn kaufen und zum Nennwert einlösen.

Darin liegt auch die ehrliche Grenze des Versprechens. Ein Stablecoin ist nur so solide wie sein
Emittent und dessen Reserven. Es ist ein Anspruch an eine Firma, kein Zentralbankgeld, „stabil“
heißt also „gekoppelt“, nicht „risikofrei“.

## Wie sie auf Solana kommen

USDC und EURC auf Solana sind **SPL-Token**: Standard-Token, die von einem bestimmten Programm im
Netzwerk ausgegeben werden. Jedes Token hat eine Mint-Adresse, die es identifiziert, und jeder
Inhaber hat ein Token-Konto, das ein Guthaben für diese Mint hält. Dein Wallet legt dieses Konto für
dich an, wenn du das Token zum ersten Mal empfängst, was eine winzige Menge SOL als Miete kostet.

Das ist aus einem praktischen Grund wichtig: **Es gibt gefälschte Token mit vertrauten Namen.** Ein
Token namens „USDC“ ist nicht USDC. Wallets und Apps vermeiden das, indem sie die Mint-Adresse
prüfen, nicht den Namen. Payvand legt die zwei Mints fest, die es akzeptiert, eine Rechnung kann also
nur mit echtem USDC oder EURC beglichen werden.

## Warum Läden sie mögen

- **Der Preis driftet nicht.** Eine Rechnung über 4,50 EURC ist 4,50 Euro wert, wenn sie ausgestellt
  wird, und wenn sie bezahlt wird — es muss also nichts mitten im Gespräch neu gerechnet werden.
- **Sie setzen ab wie jedes andere Token.** Sekunden, für einen Bruchteil eines Cents, ohne
  Kartengebühren und ohne Rückbuchungen.
- **Sie sind langweilig, und das ist der Punkt.** Ein Laden will nicht auf seine eigene Kasse
  spekulieren.

## Warum Käufer sie mögen

Wenn du Euro oder Dollar bereits als Stablecoin hältst, berührt eine Zahlung damit nicht dein
Engagement im Preis von SOL oder einem anderen Token. Es verhält sich wie ein Fremdwährungs-Wallet:
Das Guthaben ist, was draufsteht.

## Test-Token sind kein Geld

In einem Testnetz (der Kopf von Payvand zeigt **devnet**) sind USDC und EURC kostenlose Test-Token
aus einem Faucet. Sie haben denselben Namen und dasselbe Mint-Verhalten, und sie sind exakt nichts
wert. Das ist Absicht: So lernst du die Bewegungen — ein Budget auffüllen, eine Rechnung bezahlen,
den Rest abheben —, ohne einen Cent zu riskieren.
