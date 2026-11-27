---
title: L'autorisation mensuelle, en mots simples
description: Valider une fois par mois au lieu d'une fois par café. Ce que l'autorisation permet à l'app, ce qu'elle ne pourra jamais faire, et ce qui se passe vraiment à la fin d'un mois.
category: payvand
order: 1
---

Payer par portefeuille signifie une fenêtre et une vérification d'empreinte à chaque fois. C'est
acceptable une fois. C'est pénible au comptoir d'un café avec une file derrière vous.

L'autorisation mensuelle est la sortie de ce problème. Vous validez **un** paiement par mois, et
chaque petit paiement suivant passe sans aucune fenêtre.

## Ce que vous acceptez

Vous choisissez une limite — disons 50 USDC. Quand vous la validez, deux choses se produisent :

1. **Ce montant sort de votre portefeuille** vers une cagnotte qui appartient à votre portefeuille.
   Il n'est pas envoyé à Payvand, ni à aucun commerce. Voyez cela comme retirer des espèces de
   votre compte et les mettre dans la poche de votre propre manteau.
2. **Une clé est donnée à l'app**, et elle vit sur votre appareil. Cette clé peut payer des
   factures depuis la cagnotte, et rien d'autre.

## Ce que la clé peut faire — et ce qu'elle ne peut pas

Elle peut payer un commerce, jusqu'à la limite, ce mois-ci.

Elle ne peut pas envoyer d'argent à un inconnu, ni déplacer la cagnotte ailleurs, ni dépasser la
limite, ni toucher au reste de votre portefeuille. Si quelqu'un volait l'appareil, le maximum qu'il
pourrait faire est de dépenser ce qui reste de la limite du mois — et vous le verriez, parce que
chaque paiement est public.

Cette étroitesse est tout l'argument de sécurité. Ce n'est pas « l'app peut dépenser votre
argent » ; c'est « l'app peut payer des factures, avec l'argent de poche du mois, jusqu'à
épuisement ».

## Ce que le mois remet à zéro — et ce qu'il ne fait pas

Le premier jour de chaque mois, le compteur revient à zéro : vous pouvez de nouveau dépenser la
limite entière. **Aucun argent ne bouge.** Rien ne quitte votre portefeuille, rien n'est débité,
rien n'est rechargé automatiquement. La cagnotte garde exactement ce qui restait : une limite de
50 USDC sur une cagnotte de 3 USDC peut donc payer 3, pas 50, tant que vous ne rechargez pas.

Cette retenue est volontaire. Un programme ne peut pas prendre d'argent dans votre portefeuille sans
votre signature ; une recharge automatique chaque mois serait donc une autorisation permanente de
puiser dans votre portefeuille quand bon lui semble. L'autorisation existe précisément pour éviter
cela. Recharger est votre décision, avec la même validation unique que pour fixer la limite, et cela
ne déplace que l'écart entre la limite et ce que la cagnotte contient déjà.

C'est pourquoi l'app affiche ce qui est **dépensable** — le plus petit entre ce que le mois laisse et
ce que la cagnotte contient — et non la limite elle-même. Quand un mois change avec une cagnotte non
rechargée, ou quand la cagnotte approche de zéro, elle le dit et propose la recharge. Il n'y a aucun
abonnement ni prélèvement récurrent là-dedans.

## Les boutons

- **Définir / enregistrer la limite** — la seule validation. Elle déplace l'argent dans la
  cagnotte, enregistre cet appareil et démarre le mois. Enregistrer une autre limite en cours de mois
  fait repartir le mois à cette limite.
- **Recharger** — remet la cagnotte à la limite pleine en cours de mois, par exemple quand il
  reste 20 sur 50. Même validation unique. Elle ne déplace que ce qui manque, et ce que vous avez
  déjà dépensé ce mois-ci compte toujours : une recharge ne rend pas l'argent dépensé. Le compteur
  repart de lui-même au changement de mois calendaire.
- **Utiliser cet appareil à la place** — si l'autorisation a été créée sur un autre téléphone ou
  ordinateur portable, ce bouton la reprend en une validation et empêche l'ancien appareil de
  dépenser.
- **Fermer le budget** — le coupe-circuit. Il rend tout ce qui reste à votre portefeuille et
  annule la clé partout.

## Pourquoi la limite doit rester basse

La limite est le maximum qu'un appareil perdu ou volé pourrait vous coûter : une limite basse fait
donc son travail au lieu d'être un compromis. Associez-la à un portefeuille réservé aux dépenses —
[voici comment séparer un portefeuille de dépenses de votre épargne](/learn/two-wallets).

## Si le téléphone est perdu

Passez sur un autre appareil, ouvrez la page budget et **fermez le budget** avec votre
portefeuille : ce qui reste revient, et la clé de l'appareil perdu cesse de fonctionner
immédiatement. Suivez ensuite
[la liste de contrôle en cas de téléphone perdu](/learn/if-your-phone-is-lost).

## Quand payer plutôt depuis votre portefeuille

Utilisez l'autorisation pour le quotidien : un café, un étal de marché, un fournisseur habituel.
Payez directement depuis votre portefeuille pour un achat unique, un montant plus élevé, ou un
commerce que vous ne connaissez pas — l'écran de validation est alors devant vous, ce qui est
souhaitable. Les deux chemins paient les mêmes factures.
