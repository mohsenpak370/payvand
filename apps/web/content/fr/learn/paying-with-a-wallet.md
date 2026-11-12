---
title: Ce qui se passe quand vous payez avec un portefeuille
description: Du scan du code à l'arrivée de l'argent — les six étapes de tout paiement crypto, et ce que vous validez réellement.
category: basics
order: 4
---

Un paiement crypto donne l'impression d'un paiement par carte, parce que l'écran de la boutique se
comporte pareil. Sous le capot, c'est bien plus court : aucun réseau de cartes, aucun acquéreur,
aucun lot de compensation. Voici tout le trajet, dans l'ordre.

## 1. Vous recevez une demande de paiement

Dans Payvand, le vendeur saisit un montant et vous montre un QR code. Le code n'est pas l'image
d'un paiement — il *est* le paiement : l'adresse du vendeur, le montant, la devise et une courte
référence qui distingue cette facture de toutes les autres factures du même vendeur. Quand vous le
scannez, votre portefeuille lit directement ces informations.

Rien dans la demande du vendeur n'a été « envoyé à un serveur » qui pourrait ensuite changer le
montant. Le montant est inscrit dans l'adresse que l'argent doit atteindre.

## 2. Votre portefeuille vous montre ce que vous allez valider

La fenêtre du portefeuille liste le paiement. C'est l'écran le plus important de la crypto, alors
lisez-le : **montant, devise, destinataire, frais de réseau**. Si l'écran dit autre chose que
l'écran du vendeur, arrêtez-vous et vérifiez pourquoi avant de signer. L'habitude est détaillée
dans [Lisez avant de signer](/learn/checking-before-you-sign).

## 3. Vous validez, et le portefeuille signe

Valider ne « dit pas à l'app de payer ». Cela déverrouille la clé privée sur votre appareil juste
le temps de signer cette transaction, puis la reverrouille. L'app que vous utilisez ne voit jamais
la clé ; elle reçoit une signature, valable uniquement pour cette transaction précise.

## 4. La transaction part sur le réseau

La transaction signée est envoyée au réseau et incluse dans un bloc. Sur Solana, cela prend
normalement quelques secondes pour apparaître, et le paiement devient *définitivement final* en
bien moins d'une seconde — depuis la mise à jour Alpenglow d'octobre 2026, typiquement environ
150 millisecondes ([ce que veut dire la finalité](/learn/when-is-a-payment-final)). Vous pouvez
suivre ce qui se passe depuis le lien de confirmation affiché par le portefeuille ou l'app.

## 5. Le côté vendeur se met à jour

Comme le réseau est public, la page du vendeur peut surveiller l'adresse du vendeur et voir le
paiement arriver sans que personne ne le lui dise. Dans Payvand, la facture passe à **Payé** à ce
moment-là — la seconde où votre portefeuille confirme.

## 6. Les frais

Chaque transaction paie de petits frais de réseau. Sur Solana, les frais de base sont de
5 000 lamports — soit 0,000005 SOL, une fraction de centime — auxquels s'ajoutent des frais de
priorité facultatifs quand le réseau est chargé. Les frais vont au réseau, pas à Payvand ni au
vendeur.

Il vous faut tout de même un peu de SOL dans le portefeuille pour les payer. Un portefeuille avec
un gros solde de stablecoins et zéro SOL ne peut rien déplacer, ce qui surprend la première fois.

## Ce que « Payé » n'est pas

- **Ce n'est pas réversible.** Aucun remboursement. Une fois que le réseau confirme, le transfert
  tient : l'écran de validation mérite donc votre attention.
- **Ce n'est pas instantané partout.** Solana confirme en secondes, mais d'autres réseaux peuvent
  prendre des minutes, et un paiement n'est définitif qu'une fois confirmé.
- **Ce n'est pas anonyme.** Le transfert est visible par quiconque connaît les deux adresses.

## Le raccourci de Payvand : une autorisation mensuelle

Valider chaque paiement devient lassant au café. Payvand vous permet de valider **une** transaction
qui met de côté un budget mensuel dans un jeton comme USDC, et qui enregistre une clé de session
détenue par l'app sur votre appareil. Ensuite, les paiements qui tiennent dans le budget sont
signés par la clé de session : plus de fenêtre de portefeuille au comptoir.

Ce que cette clé peut faire est volontairement étroit : payer des factures jusqu'au budget que vous
avez fixé, ce mois-ci, et rien d'autre
([l'autorisation en mots simples](/learn/monthly-allowance-explained)). Elle ne peut pas envoyer de
fonds où elle veut, ni dépasser la limite, et vous pouvez fermer l'autorisation à tout moment pour
ramener le solde restant dans votre portefeuille. Si ce compromis ne vous plaît pas, payez depuis
votre portefeuille à chaque fois — les deux chemins paient les mêmes factures.
