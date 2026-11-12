---
title: Lisez avant de signer
description: L'écran de validation est le dernier moment où quelqu'un peut arrêter un paiement. Ce qu'il faut vérifier, et la différence entre envoyer de l'argent et laisser un programme en dépenser plus tard.
category: safety
order: 3
---

Chaque paiement en crypto passe par un moment : un écran dans votre portefeuille qui vous demande de
signer. Jusque-là, rien n'a bougé et tout peut être annulé gratuitement. Après, le paiement est
définitif — sur Solana, environ 150 millisecondes plus tard, sans annulation possible ni pour vous
ni pour personne ([pourquoi c'est aussi rapide](/learn/when-is-a-payment-final)). Traitez cet
écran comme la vraie décision, pas comme une formalité à cliquer.

## Ce que contient vraiment une transaction

Une transaction est une liste d'instructions plus des frais. Pour un paiement simple, la liste est
courte : déplacer ce montant de ce jeton de mon adresse vers telle adresse. Votre portefeuille en
fait quelque chose que votre clé peut signer, et le réseau l'exécute.

Lisez quatre choses à chaque fois :

1. **Le montant** — décimales comprises. `0,5` et `5` se ressemblent au premier coup d'œil.
2. **La devise ou le jeton** — un jeton peut s'appeler n'importe comment. Si une app annonce de
   l'USDC mais que le portefeuille affiche un jeton inconnu, quelque chose ne va pas.
3. **Le destinataire** — vérifiez le début et la fin de l'adresse, et surtout qu'il s'agit de qui
   vous attendez. Si vous payez un commerce, ce doit être l'adresse du commerce figurant sur la
   facture que vous venez de scanner, pas une adresse collée depuis une conversation.
4. **Les frais** — faibles sur Solana, mais des frais démesurés sont le signe que la transaction
   n'est pas ce que vous croyez.

## Envoyer n'est pas la même chose qu'autoriser

Une signature peut faire deux choses très différentes :

- **Un transfert** déplace un montant précis, une fois, vers une adresse précise. C'est ce qu'est
  un paiement.
- **Une autorisation** donne à un programme la permission de déplacer des jetons de votre compte
  plus tard, parfois jusqu'à un montant bien plus élevé que le paiement affiché. C'est une
  permission permanente, pas un paiement.

Les autorisations existent pour de bonnes raisons — un échange de jetons en a besoin — mais elles
sont aussi l'outil préféré des videurs de portefeuille, parce qu'une seule autorisation peut vider
un solde longtemps après que vous avez fermé l'onglet. Si vous n'attendiez pas d'autorisation,
annulez. Si vous devez en accorder une, accordez un petit montant précis plutôt qu'un montant
illimité, et révoquez-la ensuite.

## Pourquoi le montant est digne de confiance ici

Les factures de Payvand placent le montant et le destinataire *à l'intérieur de l'adresse que le
paiement doit atteindre*. Il n'y a pas de champ « montant » séparé qu'un serveur pourrait modifier
entre le QR code et votre validation : payer un autre montant reviendrait à payer une autre
adresse, qui n'est pas celle que le vendeur surveille. Le vendeur ne peut donc pas augmenter le
prix après votre scan, et une page ne peut pas rediriger les fonds en douce.

Quand vous payez, validez exactement ce que la facture indique. Si la fenêtre du portefeuille
contredit l'écran que vous avez scanné, arrêtez-vous et demandez — l'un des deux n'est pas ce que
vous croyez.

## Des habitudes à garder

- **Signez une chose à la fois.** Une file de validations est conçue pour vous faire cesser de
  lire.
- **Ne signez pas sur un appareil que vous ne contrôlez pas.** Les ordinateurs partagés et les
  sessions d'assistance à distance sont ainsi que des inconnus « serviables » vident des
  portefeuilles.
- **Préférez un portefeuille matériel pour les gros montants.** Il affiche le montant et le
  destinataire sur son propre écran, si bien qu'un ordinateur compromis ne peut pas les échanger
  à votre insu.
- **Dans le doute, refusez.** Refuser ne coûte rien et le paiement peut être retenté dans une
  minute.

Si quelque chose a été signé qui n'aurait pas dû l'être,
[la liste de contrôle en cas de téléphone perdu](/learn/if-your-phone-is-lost) est la même série
d'étapes à suivre : déplacez ce qui reste, arrêtez l'ancienne clé et signalez-le.
