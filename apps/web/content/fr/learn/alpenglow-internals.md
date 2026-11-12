---
title: Alpenglow, de l'intérieur
description: Les certificats de Votor, le chemin rapide à 80 %, le repli à 60 %, et ce que « final en 150 ms » coûte en hypothèses. Le changement de consensus qui a remplacé TowerBFT.
category: technical
order: 4
level: technical
---

Alpenglow est le plus grand changement du consensus de Solana depuis le lancement : il a remplacé
**TowerBFT** par un nouveau mécanisme de finalisation appelé **Votor**. Il est en production sur
mainnet, activé au début de l'**époque 1064, le 21 octobre 2026**, après que SIMD-0326 a été adopté
avec 98,27 % de l'enjeu participant en septembre 2025.

Sa seconde phase, **Rotor**, qui remplace le protocole de propagation Turbine, n'est ni livrée ni
planifiée. Ne confondez pas les deux — la page de la mise à jour est explicite sur l'ordre.

## Ce que faisait TowerBFT, et pourquoi il devait partir

TowerBFT atteignait la finalité en accumulant des **verrouillages** : chaque vote sur un bloc
ultérieur augmentait le verrouillage d'un validateur, et la finalité demandait environ
**12,8 secondes** de confirmations accumulées. Les votes eux-mêmes étaient des transactions,
incluses dans des blocs — le trafic de consensus rivalisait avec les paiements des utilisateurs pour
le même espace de bloc.

C'est supportable pour du règlement. C'est désespéré pour une file de café.

## Comment Votor finalise

Les validateurs échangent désormais leurs votes **directement**, en dehors du flux de blocs, et les
agrègent en **certificats de quorum** — des preuves pondérées par l'enjeu attestant qu'un seuil a
voté d'une certaine façon. SIMD-0326 décrit cinq types de certificats :

| Certificat | Seuil | Rôle |
|---|---|---|
| Notarisation | 60 % | Les validateurs s'accordent sur un bloc |
| Saut | 60 % | Les validateurs s'accordent pour sauter un slot |
| Finalisation | 60 % | Finalisation au second tour |
| Finalisation rapide | 80 % | Finalité en un seul tour |
| Repli de notarisation | 60 % | Chemin quand le tour rapide ne se remplit pas |

Le chemin heureux est donc : si **80 % de l'enjeu** notarise un bloc en un seul tour, un certificat
de Finalisation rapide existe et le bloc est final — pas de second tour, pas d'accumulation de
verrouillages.

Si cela n'arrive pas, le protocole retombe sur un chemin plus lent à deux tours utilisant les seuils
de 60 % : un certificat de notarisation plus le certificat de second tour approprié. La finalité
arrive quand même, avec un aller-retour de plus.

## Le modèle de fautes

Alpenglow est conçu pour **20 % d'enjeu adverse plus 20 % d'enjeu hors ligne**, combinés. Ce n'est
pas l'affirmation que 40 % d'enjeu byzantin est toléré — la répartition 20+20 est l'hypothèse, et
c'est elle que le seuil de 80 % du chemin rapide encode.

Cela compte quand on lit des chiffres marketing. « 80 % de l'enjeu » n'est pas une supermajorité au
sens classique des deux tiers ; c'est le seuil qui permet à un seul tour de suffire *étant donné* le
modèle de fautes.

## D'où viennent les 150 ms

La note de conception d'Anza cite une **médiane d'environ 150 ms**, avec des cas autour de
**100 ms**, à partir de simulations utilisant la répartition de l'enjeu d'alors, en excluant
explicitement les surcoûts de calcul. La page de mise à jour de Solana présente ~150 ms comme
l'attente, et le tableau de bord en direct affiche en général autour de 200 ms.

Aucun maximum n'est publié. L'énoncé d'ingénierie honnête est : un aller-retour entre validateurs,
plus la transmission, plus la variabilité de la répartition de l'enjeu et du calendrier des slots.

## Ce qui a changé au-delà du chiffre

- **Les transactions de vote ont disparu des blocs**, donc le consensus ne consomme plus l'espace
  de bloc dont les paiements ont besoin.
- **La preuve d'histoire a cessé de cadencé la production de blocs.** Les délais et certificats de
  Votor ont pris ce rôle, et les entrées de tick ont quitté le flux de shreds.
- **`confirmed` et `finalized` ont convergé.** Anza documente les deux comme inscrits
  définitivement ; `processed` reste l'état précoce et faible. Le nouveau code devrait préférer
  `finalized`, et Solana indique que `confirmed` sera retiré dans une version ultérieure.
- **L'exécution n'est pas touchée.** La SVM, les programmes, le modèle de comptes et le format des
  transactions n'ont pas changé. Une app de paiement n'a pas à être réécrite ; elle doit faire
  confiance à un autre libellé d'engagement.

## Comment vérifier un cluster

`getAgGenesisCert` indique si un cluster a migré, et la genèse Alpenglow de mainnet est le slot
**459 653 000**. Sur devnet, l'activation a eu lieu à l'époque **1167, le 25 septembre 2026**. Pour
tout ce qui est visible par l'utilisateur, préférez interroger l'engagement plutôt que supposer une
date.

## Ce que cela signifie pour une app comme celle-ci

Un parcours de point de vente peut traiter « finalisé » comme « remettez la marchandise », car il ne
reste plus de période de verrouillage à attendre. Le risque qu'il ne supprime pas est humain : la
finalité rend une mauvaise adresse mauvaise pour toujours, plus vite que jamais. C'est pourquoi
l'écran de validation compte toujours plus que le badge de confirmation.

## Sources

- [Solana : mise à jour Alpenglow et état](https://solana.com/upgrades/alpenglow) · [tableau de bord](https://solana.com/upgrades/alpenglow/dashboard)
- [SIMD-0326 : Alpenglow](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md)
- [Anza : Alpenglow, un nouveau consensus pour Solana](https://www.anza.xyz/blog/alpenglow-a-new-consensus-for-solana)
- [Anza : statuts d'engagement](https://docs.anza.xyz/consensus/commitments)
