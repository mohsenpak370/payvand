---
title: Staking, validateurs et époques
description: Comptes de mise, comptes de vote, montée en charge, commissions et un calendrier de leaders fixé un jour à l'avance. Comment la preuve d'enjeu de Solana répartit réellement le droit d'écrire des blocs.
category: technical
order: 5
level: technical
---

Le budget de sécurité de Solana, c'est du SOL misé. Comprendre la mécanique du staking explique
pourquoi le réseau se comporte comme il le fait sous charge, et ce que « validateur » veut vraiment
dire.

## Deux comptes, deux rôles

- Un **compte de mise** détient le SOL d'un délégateur et note à quel **compte de vote** de
  validateur il est délégué. C'est là que vit le poids économique.
- Un **compte de vote** appartient à un validateur et enregistre ses votes et ses crédits de vote.
  Il ne détient pas la mise déléguée.

Un compte de mise se délègue à un seul validateur à la fois ; répartir sur plusieurs validateurs
signifie plusieurs comptes de mise. La délégation n'est pas une garde — le validateur ne gagne
jamais la possibilité de déplacer votre mise, seulement de voter avec son poids.

## Montée en charge, descente et époques

Les changements de délégation ne prennent pas effet instantanément. La mise **monte en charge** pour
devenir active et **redescend** avant de pouvoir être retirée, et le réseau plafonne ce qui peut
s'activer ou se désactiver par époque. Le délai exact dépend donc de la quantité de mise qui bouge
en même temps.

Une **époque** fait **432 000 slots**. Avec le temps de slot actuel de 200 ms, cela représente
nominalement **24 heures** ; les slots sautés font varier le temps réel écoulé. Les frontières
d'époque sont les moments où s'appliquent les poids de mise, les calendriers de leaders et la
distribution des récompenses.

Deux chiffres pratiques :

- La **délégation minimale** est une valeur de cluster ; l'exemple documenté de
  `getStakeMinimumDelegation` est **1 SOL**. Interrogez le cluster au lieu de la coder en dur.
- La **commission** est le pourcentage que prend le validateur sur les récompenses, fixé sur le
  compte de vote. Elle porte sur les récompenses, jamais sur le principal.

## Qui produit quel bloc

Le **calendrier de leaders** est calculé avant chaque époque. L'algorithme amorce une sélection
pseudo-aléatoire à partir d'une hauteur de tick PoH, échantillonne l'ensemble des validateurs actifs
et leur mise, et choisit les leaders pondérés par la mise. Le calendrier couvre 432 000 slots avec un
décalage d'anticipation de 8 192 slots exposé sur RPC.

C'est pourquoi la concentration de l'enjeu compte aussi sur le plan opérationnel : plus de mise
signifie plus de slots, et un validateur bien géré avec beaucoup d'enjeu produit une part
disproportionnée des blocs.

## Récompenses, inflation et pénalités

Les récompenses d'une époque sont réparties entre la mise participante au prorata de
**mise × crédits de vote du validateur**, moins la commission. Les paramètres publiés du
*calendrier* d'inflation sont 8 % d'inflation annuelle initiale, une désinflation de 15 % par an et
un taux de long terme de 1,5 % — ils décrivent la courbe, pas le taux du jour.

**Solana ne confisque pas actuellement la mise déléguée** en cas de faute d'un validateur. Un
validateur qui tombe hors ligne ou signe deux fois perd des récompenses et de la réputation, pas le
principal. C'est une différence significative avec les réseaux où l'équivoque brûle la mise, et cela
mérite d'être dit clairement dans toute comparaison : la sécurité repose ici sur le vote pondéré par
l'enjeu et sur les délégateurs qui retirent leur mise, pas sur une combustion.

Confirmer un bloc demande **deux tiers de la mise active** ; la finalité sous Alpenglow s'exprime
par les certificats de Votor (voir [la page Alpenglow](/learn/alpenglow-internals)), où le chemin
rapide exige 80 % et les chemins de repli des seuils de 60 %.

## Ce que cela signifie pour une application

- **La concentration des leaders est une variable de latence.** Un slot dont le leader est loin de
  vos utilisateurs, ou mal connecté, est un paiement plus lent — invisible à 200 ms, visible sous
  contrainte.
- **La mise bouge lentement.** Le poids d'un validateur ne peut pas être emprunté pour un seul slot ;
  la montée et la descente en charge rendent les attaques par force brute coûteuses en temps autant
  qu'en capital.
- **Les récompenses ne sont pas un rendement gratuit.** C'est de l'inflation plus des frais,
  partagés avec un validateur qui prend une commission. Quiconque annonce un rendement de staking
  sans nommer la commission et la source d'inflation ne raconte que la moitié de l'histoire.

## Sources

- [Solana : staking](https://solana.com/docs/references/staking) · [comptes de mise](https://solana.com/docs/references/staking/stake-accounts)
- [Anza : délégation de mise et récompenses](https://docs.anza.xyz/consensus/stake-delegation-and-rewards) · [rotation des leaders](https://docs.anza.xyz/consensus/leader-rotation)
- [getStakeMinimumDelegation](https://solana.com/docs/rpc/http/getstakeminimumdelegation) · [getEpochSchedule](https://solana.com/docs/rpc/http/getepochschedule)
- [Solana : temps de slot réduits](https://solana.com/upgrades/reduced-slot-times)
