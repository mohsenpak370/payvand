---
title: Quand un paiement est-il vraiment terminé ?
description: Un paiement par carte peut être annulé pendant des semaines. Un paiement Solana devient définitif en moins d'une seconde. Ce que veut dire « final », et ce qui a changé en octobre 2026.
category: solana
order: 2
---

Voici la version quotidienne d'une question technique. Vous payez un café sans contact. Quand la
boutique peut-elle vous tendre la tasse et vous laisser partir sans risque ?

Avec une carte, la réponse honnête est « dans quelques jours », et c'est pourquoi les paiements par
carte peuvent être repris et pourquoi les commerces paient pour ce risque. Sur Solana, la réponse
est désormais **moins d'une seconde**, depuis qu'une mise à jour appelée **Alpenglow** est passée
en production sur le réseau principal le 21 octobre 2026.

## Trois états possibles pour un paiement

- **Vu (traité).** Un ordinateur a le paiement en vue. Il peut encore être remplacé si le réseau se
  réorganise autour d'une autre version des faits. On ne peut pas s'y fier.
- **Confirmé.** Assez de réseau s'est mis d'accord. Avant Alpenglow, c'était rapide mais moins
  solide que définitif — un paiement pouvait être confirmé et, lors d'une réorganisation rare,
  quand même perdu.
- **Finalisé.** Définitivement inscrit. Il ne peut être ni annulé ni réordonné, même par les
  validateurs qui l'ont écrit.

Avant Alpenglow, la finalité prenait environ **13 secondes** sur Solana. Les commerces étaient donc
dans une position inconfortable : attendre 13 secondes au comptoir, ou accepter le « confirmé » et
porter un petit risque. Ni l'un ni l'autre n'est une bonne façon de vendre du café.

## Ce qui a changé

Alpenglow a remplacé l'ancien système de vote par un système où les validateurs votent directement
et où leurs votes sont rassemblés dans un **certificat**. Quand les validateurs détenant 80 % du
SOL misé s'accordent en un seul tour, le paiement est final immédiatement ; sinon, un second tour
le règle.

Les chiffres concrets :

- **En général environ 150 millisecondes** — à peu près un septième de seconde — entre l'inclusion
  du paiement et sa finalité.
- Parfois plus vite (environ 100 ms), parfois plus lentement. Le tableau de bord public de Solana
  affiche en général une finalité autour de 200 millisecondes.
- Cela varie selon la répartition de l'enjeu, et aucun maximum garanti n'est publié. Ce qu'on peut
  affirmer avec assurance, c'est que c'est **bien moins d'une seconde**.

Une conséquence à connaître : avec Alpenglow, **« confirmé » et « finalisé » veulent dire la même
chose**. Les deux libellés existent toujours, mais les deux signifient maintenant inscrit
définitivement. Seul « traité » reste un état précoce, peu sûr.

## Pourquoi cela compte au comptoir

La boutique peut remettre le café dès que l'app indique confirmé, car il n'y a plus rien à
attendre. Pas de zone d'attente, pas de « on verra si ça passe », pas de pause de 13 secondes avec
une file derrière vous. C'est ce qui fait qu'un paiement crypto ressemble à un passage de carte
plutôt qu'à un virement bancaire.

## L'autre face du définitif

Le même mécanisme qui rend un paiement définitivement acquis à une boutique rend une erreur
définitivement une erreur. Mauvaise adresse, mauvais montant, un site qui vous a trompé — une fois
que c'est définitif, c'est définitif, et il n'y a personne à qui faire appel.

La rapidité et l'irréversibilité sont donc une seule propriété, pas deux. C'est précisément pour
cela qu'il vaut la peine de lire
[ce qu'il faut vérifier avant de signer](/learn/checking-before-you-sign).

## Ce que cela signifie pour vous

Quand Payvand affiche un paiement comme confirmé, l'argent est réellement arrivé et reste arrivé —
en moins de temps qu'il n'en faut pour remettre votre téléphone dans votre poche. La partie qui
mérite encore votre attention est la demi-seconde *avant* de valider.
