---
title: Ce que coûte un paiement, et pourquoi
description: Où va l’argent quand quelqu’un paie avec Payvand — les frais de réseau, les frais du protocole, le reçu permanent sur la chaîne et les coûts qui font tourner le service.
category: payvand
order: 3
---

Rien de tout cela n’est gratuit à exploiter, et presque rien de ce que coûte un paiement n’est du
bénéfice. Cette page expose chaque coût et qui le paie, pour que le chiffre à l’écran ne surprenne
jamais.

## Ce que paie le client

Exactement le prix du code, plus les frais de réseau Solana pour la transaction — des fractions de
centime, versés au réseau lui-même, pas à nous. Aucune majoration, aucun frais de service, aucun frais
de traitement. Si un code indique 4 EURC, le portefeuille affiche 4 EURC.

## Ce que paie le vendeur

Deux choses, toutes deux déduites du montant plutôt qu’ajoutées, et toutes deux visibles avant qu’un
code soit partagé :

1. **Les frais du protocole**, un pourcentage du prix. Ils sont marginaux et non forfaitaires : un
   taux s’applique à la première tranche du prix et un taux plus bas à tout ce qui dépasse un seuil,
   afin qu’une grosse facture ne coûte jamais plus par unité qu’une petite. Le seuil diffère entre SOL
   et stablecoins, parce que ce n’est pas la même monnaie.
2. **Les frais de reçu**, pour les paiements en token. Chaque paiement réglé laisse un petit reçu
   permanent sur la chaîne. L’espace sur la chaîne se paie d’avance, en SOL, et ne se récupère jamais.
   Les frais de reçu couvrent ce coût, et ils sont calculés au cours du SOL en direct : ils suivent
   donc ce que l’espace coûte aujourd’hui, et non ce que valait le SOL quand la règle a été écrite. Ils
   achètent une quantité fixe d’espace, donc plus lourds sur une petite facture. Une facture en SOL ne
   les porte pas séparément, le vendeur cédant déjà ce loyer sur le prix.

Quand un paiement est assez petit pour que le pourcentage tombe sous ce reçu, le pourcentage n’est pas
prélevé du tout : le vendeur paie le reçu et rien d’autre. Le protocole ne prend pas de frais sur un
tel paiement plutôt que des frais qui ne couvriraient pas leur propre reçu.

L’écran du vendeur montre le montant exact qu’il recevra et la décomposition des frais en ces deux
parties, avant qu’un client voie le code.

## Ce que son exploitation nous coûte

Un paiement va de portefeuille à portefeuille. Nous ne détenons jamais l’argent : aucun encours à
faire fructifier. En revanche, il y a une facture :

- **Des serveurs** : le site et l’API qui construit les codes, lit l’état des paiements et sert les
  boutiques.
- **Une base de données** : comptes, boutiques, produits, codes enregistrés et abonnements.
- **L’accès au réseau** : le service RPC d’où l’app lit les soldes et les états de paiement, une
  infrastructure payante à tout volume réel.
- **L’information de prix** : les taux de change, pour qu’un prix en euros se paie en SOL ou en
  dollars à un taux juste.
- **L’e-mail** : les quelques messages dont un compte a besoin — avis de connexion, réinitialisation.
- **Supervision, sauvegardes et travail de sécurité** : la moitié moins brillante de tout service qui
  touche à l’argent.
- **Des personnes** : construire, réparer, répondre.

L’abonnement boutiques et les frais du protocole en couvrent une partie. Les frais ne sont pas fixés
pour maximiser ce que nous prenons, mais pour garder le service en vie alors qu’un paiement reste moins
cher qu’un terminal de carte.

## Pourquoi les frais ont deux parties

Un pourcentage seul ne couvre pas un coût fixe. Le reçu sur la chaîne coûte pareil pour un paiement
d’une unité ou de mille : des frais uniquement proportionnels seraient trop faibles sur les petits
paiements et une part inutilement forte sur les gros. Les scinder — un pourcentage pour le travail, des
frais fixes pour l’espace, et le reçu comme plancher des deux — garde les petits paiements possibles et
les gros bon marché.

## Ce qui peut changer

Les taux et les frais de reçu sont fixés par la plateforme et suivent le coût d’exploitation ; les
frais de reçu sont recalculés au cours du SOL en direct, pour suivre une hausse du SOL plutôt que
l’absorber. Le prix
d’un code ne change jamais, mais les frais sont ceux en vigueur au moment du paiement : un changement
de taux s’applique donc aussi aux codes déjà partagés et pas encore payés. L’app montre la décomposition
aux taux en vigueur.
