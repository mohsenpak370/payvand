---
title: Comment le registre est écrit et partagé
description: Slots, shreds, racines de Merkle, codage à effacement, et une mempool qui n'existe pas. La machinerie entre « signé » et « tout le monde est d'accord ».
category: technical
order: 3
level: technical
---

Une transaction signée n'est qu'une prétention tant qu'un producteur de blocs ne l'a pas incluse.
Cette page porte sur la plomberie qui transforme les prétentions en une histoire partagée et
vérifiable.

## Slots et blocs

Le temps sur Solana se divise en **slots** de **200 ms**, chacun avec un leader désigné, et les
leaders sont planifiés une époque entière à l'avance. Un slot est une possibilité de produire un
bloc, pas une garantie : des slots peuvent être sautés, et un leader peut produire plus d'un bloc
candidat.

La conséquence pratique est que « le bloc courant » reste flou pendant quelques centaines de
millisecondes. Les transactions ne font pas la queue dans une mempool en attendant d'être incluses
— il n'y a pas de mempool. Elles sont transmises au leader censé produire le slot suivant et soit
atterrissent dans ce bloc, soit expirent.

## Pourquoi les blocs voyagent en shreds

Un bloc est trop gros pour être remis directement à chaque validateur. Le leader le **découpe en
shreds** :

- Les shreds de données portent des fragments d'entrées sérialisées, de 1 280 octets environ chacun.
- Les shreds de codage sont générés par **codage à effacement de Reed-Solomon** sur des groupes de
  shreds de données, appelés ensembles FEC. Un ensemble peut contenir jusqu'à 67 shreds de données ;
  32 est la configuration recommandée.
- Tous les shreds sont signés par le producteur du bloc.
- Chaque ensemble FEC est merklisé : les feuilles sont hachées deux à deux jusqu'à une **racine de
  Merkle**, et le leader signe cette racine. Comme un shred porte un chemin de preuve, n'importe
  quel nœud peut vérifier un morceau sans détenir le bloc entier, et les morceaux manquants peuvent
  être reconstruits à partir des shreds de codage.

## Turbine, l'arbre de diffusion

Turbine distribue ces shreds dans un arbre pondéré par l'enjeu plutôt qu'à tout le monde en même
temps. Le leader diffuse vers un nœud racine, qui éclate vers la couche 1, dont les nœuds éclatent
vers la couche 2, et ainsi de suite. La racine tourne **à chaque shred**, et la liste des pairs est
remélangée à chaque shred pour rendre les attaques ciblées plus difficiles.

Deux autres rôles sont faciles à confondre avec celui-ci :

- **Gossip** est le plan de contrôle : découverte des pairs, informations de contact et métadonnées
  des validateurs. Il ne transporte pas de transactions.
- **Gulf Stream** est le chemin entrant : les clients et les validateurs transmettent les
  transactions au prochain leader qu'ils connaissent déjà, ce qui explique que Solana n'ait pas
  besoin de mempool et que le blockhash d'un client doive être récent.

## La preuve d'histoire, après Alpenglow

La PoH était un délai vérifiable : une chaîne de hachages qui permettait à chacun de vérifier
l'ordre et le passage du temps sans faire confiance à une horloge. Pendant des années, elle a aussi
**cadencé** la production de blocs.

Ce rôle de cadence a disparu. Depuis Alpenglow, ce sont les délais et les certificats de Votor qui
décident quand les blocs sont finalisés, et les entrées de tick qui portaient le rythme de la PoH ont
quitté le flux de shreds. La construction de hachage de la PoH existe toujours dans l'histoire du
registre, mais ce n'est plus elle qui fait tourner le réseau.

## Les fourches, brièvement

Comme des slots peuvent être sautés et que des leaders peuvent produire des blocs concurrents, le
réseau peut temporairement détenir plus d'une vue de l'histoire récente. Les validateurs convergent
vers une branche et — depuis Alpenglow — la finalisation est un certificat explicite plutôt que
l'accumulation des verrouillages de TowerBFT.

Ce qui compte pour une app : une transaction vue dans un bloc est *traitée*, ce qui n'est pas
définitif. Seul un bloc finalisé est irréversible. Sur Solana moderne, cette distinction se résout
en environ 150 ms, mais c'est toujours la différence entre « probablement » et « certainement ».

## Ce que cela explique

- **Pourquoi il y a une limite de 1 232 octets par transaction** : c'est un paquet réseau, et un
  bloc est un flux de paquets.
- **Pourquoi un paiement échoue parfois avec une erreur de blockhash** : la transaction a survécu à
  sa fenêtre de validité d'environ 150 slots avant que quiconque ne l'inclue.
- **Pourquoi la finalité est une propriété de consensus et non de réseau** : la propagation est
  ainsi qu'un bloc se répand, le consensus est ainsi qu'il devient permanent. Ce sont des systèmes
  séparés par conception, et c'est précisément ce qui a permis à Votor de remplacer le consensus
  sans toucher au voyage des blocs.

## Sources

- [Anza : propagation des blocs par Turbine](https://docs.anza.xyz/consensus/turbine-block-propagation)
- [Spécification des shreds](https://raw.githubusercontent.com/solana-foundation/specs/main/p2p/shred.md)
- [Solana : temps de slot réduits](https://solana.com/upgrades/reduced-slot-times)
- [Alpenglow : PoH et entrées de tick](https://solana.com/upgrades/alpenglow)
- [Gulf Stream](https://solana.com/news/8-innovations-that-make-solana-the-first-web-scale-blockchain)
