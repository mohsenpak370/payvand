---
title: Les stablecoins, en mots simples
description: Pourquoi un commerce peut afficher ses prix en USDC ou en EURC sans craindre que le prix du jeton bouge, et ce qui les garantit réellement.
category: basics
order: 5
---

La plupart des cryptos dont on entend parler ont un prix qui bouge toute la journée. C'est une
très mauvaise propriété pour un café : si la boutique vous annonce un SOL et que le prix du SOL
chute de 5 % avant que vous payiez, elle vient de perdre 5 %.

Un **stablecoin** est un jeton conçu pour valoir toujours une unité d'une monnaie ordinaire. Les
deux que vous verrez sur Payvand sont **USDC** (un dollar chacun) et **EURC** (un euro chacun),
tous deux émis par Circle. Ils vivent sur le réseau Solana, qui est la plomberie sous chaque
paiement ici — [ce qu'est Solana, en mots simples](/learn/what-is-solana).

## Ce que signifie « garanti »

Pour chaque USDC en circulation, Circle détient environ un dollar en liquidités et en dette
publique à court terme en réserve, et publie régulièrement des attestations de ces réserves. EURC
fonctionne de la même façon face à l'euro. Vous pouvez rendre vos USDC à Circle et recevoir des
dollars, et c'est ce qui maintient le prix collé au dollar : s'il se négociait à 97 centimes,
quelqu'un l'achèterait pour le faire racheter au pair.

C'est aussi la limite honnête de la promesse. Un stablecoin n'est solide que comme son émetteur et
ses réserves. C'est une créance sur une entreprise, pas de la monnaie de banque centrale : « stable »
veut donc dire « indexé », pas « sans risque ».

## Comment ils arrivent sur Solana

USDC et EURC sur Solana sont des **jetons SPL** : des jetons standard émis par un programme précis
du réseau. Chaque jeton a une adresse d'émission qui l'identifie, et chaque détenteur a un compte
de jetons qui porte un solde pour cette émission. Votre portefeuille crée ce compte pour vous la
première fois que vous recevez le jeton, ce qui coûte un tout petit peu de SOL en loyer.

Cela compte pour une raison très concrète : **il existe de faux jetons aux noms familiers**. Un
jeton nommé « USDC » n'est pas USDC. Les portefeuilles et les apps évitent le piège en vérifiant
l'adresse d'émission, pas le nom. Payvand fixe les deux émissions qu'il accepte, donc une facture
ne peut être réglée qu'avec le vrai USDC ou le vrai EURC.

## Pourquoi les commerces les aiment

- **Le prix ne dérive pas.** Une facture de 4,50 EURC vaut 4,50 euros au moment où elle est émise
  et au moment où elle est payée : inutile de réajuster au milieu de la conversation.
- **Ils se règlent comme n'importe quel jeton.** En secondes, pour une fraction de centime, sans
  frais de carte et sans remboursements.
- **Ils sont ennuyeux, et c'est le but.** Un commerce ne veut pas spéculer avec sa propre caisse.

## Pourquoi les acheteurs les aiment

Si vous détenez déjà des euros ou des dollars en stablecoin, payer avec ne touche pas votre
exposition au prix du SOL ni d'aucun autre jeton. Cela se comporte comme un portefeuille en devise
étrangère : le solde est bien ce qu'il annonce.

## Les jetons de test ne sont pas de l'argent

Sur un réseau de test (l'en-tête de Payvand affiche **devnet**), USDC et EURC sont des jetons de
test gratuits, distribués par un robinet. Ils ont le même nom et le même comportement d'émission,
et ils ne valent exactement rien. C'est voulu : c'est ainsi qu'on apprend les gestes — alimenter
une autorisation, payer une facture, retirer le reste — sans risquer un centime.
