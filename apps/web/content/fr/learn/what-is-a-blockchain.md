---
title: Qu'est-ce qu'une blockchain ?
description: Un carnet partagé avec des milliers de copies, où modifier une vieille ligne vous trahit. C'est pourquoi un paiement en crypto ne peut pas être annulé en douce.
category: basics
order: 2
---

Oubliez un instant le mot « blockchain ». Imaginez un carnet.

Chaque fois que quelqu'un paie quelqu'un, une nouvelle ligne est écrite dans le carnet : *cette
adresse a envoyé tant à telle adresse*. Des milliers de personnes gardent une copie identique du
même carnet, et chaque nouvelle page doit être vérifiée par toutes avant de compter.

Voilà une blockchain. « Block » veut simplement dire une page. « Chain » signifie que chaque page
porte une empreinte de la page précédente, si bien que les pages sont verrouillées ensemble dans
l'ordre.

## Pourquoi personne ne peut tricher avec le carnet

Chaque page est estampillée d'un long code — une **empreinte** de tout ce qu'elle contient.
Changez un caractère sur une page, et son empreinte change complètement. L'empreinte de la page
suivante ne correspond alors plus, ni celle d'après.

Donc si quelqu'un modifie discrètement une vieille ligne dans sa copie, toutes les pages qui
suivent cessent de s'additionner. Les autres copies ne sont pas d'accord, la modification est
rejetée, et c'est le tricheur qui a l'air d'avoir tort.

Pour réécrire l'histoire, il faudrait refaire toutes les pages depuis, sur plus de la moitié des
copies à la fois, plus vite que les copies honnêtes n'ajoutent de nouvelles pages. Sur un réseau
sérieux, ce n'est pas une ambition réaliste, c'est une chimère.

## Ce que signifie « on-chain »

**On-chain** veut dire inscrit dans ce carnet : visible par tous, immuable ensuite. **Off-chain**
veut dire enregistré dans la base privée de quelqu'un — une banque, une caisse de commerce, le
serveur d'une application — où cette personne peut le modifier.

Quand un commerce dit que votre paiement est « on-chain », cela signifie que le carnet contient la
ligne, et que le commerce ne se fie à la parole de personne.

## Pourquoi un paiement est définitif

Il n'y a pas de bouton d'annulation dans le carnet et, surtout, personne avec l'autorité de
l'enfoncer. Aucune entreprise à appeler, aucun émetteur de carte à qui ouvrir un litige, aucune
banque qui peut retirer l'argent.

Cela joue dans les deux sens. Un commerce n'a jamais à craindre qu'un paiement soit annulé des
semaines plus tard — c'est pourquoi les paiements en crypto peuvent être bon marché au lieu de
supporter des coûts d'assurance. Et vous ne pouvez pas récupérer votre argent depuis une mauvaise
adresse, ni depuis quelqu'un qui vous a trompé. Les
[guides de sécurité](/learn/checking-before-you-sign) portent entièrement sur cette moitié du
contrat.

## Qui écrit dans le carnet

Personne ne le possède, alors qui fait le travail d'ajouter des pages ? Sur Solana, un ensemble
d'ordinateurs appelés **validateurs** prennent les tours. Ils reçoivent les frais minuscules
attachés à chaque paiement, et ils ont de l'argent à eux bloqué en dépôt qu'ils perdent s'ils
trichent. Le guide suivant explique le réseau lui-même :
[Qu'est-ce que Solana ?](/learn/what-is-solana)

## Qui peut le lire

Tout le monde. Les soldes et les historiques de paiement sont publics — une adresse, pas un nom,
mais tout est visible par quiconque regarde. C'est utile (un commerce peut surveiller un paiement
sans qu'on le lui dise) et bon à retenir (une adresse utilisée publiquement est une adresse très
publique).

## Ce que cela signifie pour vous

Une blockchain est un reçu que personne ne peut falsifier et que personne ne peut reprendre. Cela
vaut beaucoup au comptoir d'un commerce, et cela signifie que l'instant avant votre validation est
le seul instant où quelqu'un peut arrêter un paiement. Sur Solana, cet instant est aujourd'hui
suivi de « terminé » en bien moins d'une seconde —
[voici ce que veut dire la finalité](/learn/when-is-a-payment-final).
