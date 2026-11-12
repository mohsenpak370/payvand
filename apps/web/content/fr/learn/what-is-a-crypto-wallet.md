---
title: Ce qu'est vraiment un portefeuille crypto
description: Un portefeuille n'est pas une bourse. C'est une paire de clés qui prouve que l'argent sur le réseau est à vous — d'où le fait que personne ne puisse le réinitialiser pour vous.
category: basics
order: 3
---

Si vous n'avez jamais connu qu'une banque, le mot « portefeuille » trompe. Un portefeuille crypto
ne contient pas votre argent comme une bourse contient des billets. L'argent vit sur un réseau
public, sous forme de solde rattaché à une adresse. Ce que le portefeuille détient est l'unique
chose qui peut le déplacer : la **clé privée**.

## Les trois éléments que vous allez rencontrer

- **Votre adresse** — une longue chaîne comme `8D8GowHJyQRpPqEtAZy4H1DhjpbC…`. Elle est publique.
  Vous pouvez la partager dans un message, l'imprimer sur une facture ou la publier en ligne. On
  s'en sert pour vous envoyer de l'argent.
- **Votre clé privée** — le secret qui signe les paiements pour cette adresse. Qui l'a contrôle le
  solde, et il n'y a personne à appeler si elle fuit.
- **Votre phrase de récupération** — douze ou vingt-quatre mots ordinaires qui *sont* votre clé
  privée, écrits sous une forme que des humains peuvent sauvegarder. On en parle plus loin dans
  [Votre phrase de récupération, c'est tout le portefeuille](/learn/protecting-your-seed-phrase).

Une application de portefeuille est un trousseau plus une fenêtre : elle range les clés sur votre
appareil et vous montre ce que le réseau dit de votre adresse.

## Pourquoi personne ne peut réinitialiser votre mot de passe

Une banque garde votre solde dans sa propre base, elle peut donc vérifier votre identité et
réinitialiser ce que vous avez oublié. Un portefeuille crypto n'a ni base de ce genre ni compte. Le
réseau ne vérifie qu'une chose : **ce paiement a-t-il été signé par la clé qui possède cette
adresse ?** Pas de signature, pas de paiement — et aucun service client, aucun gouvernement, aucune
entreprise ne peut produire cette signature pour vous.

C'est tout le sens de l'auto-garde : personne ne peut geler votre solde, et personne ne peut non
plus annuler une erreur à votre place. Traitez les clés comme l'acte de propriété d'une maison,
pas comme un mot de passe qu'on réinitialise.

## Ce que cela donne au quotidien

- **Votre argent n'est pas « dans l'app ».** Supprimez l'app, réinstallez-la sur un nouveau
  téléphone, restaurez la phrase de récupération : le même solde réapparaît. L'app n'a jamais été
  le portefeuille.
- **Les paiements sont définitifs.** Aucun remboursement, aucun bouton « annuler le virement ».
  Vérifiez le montant et le destinataire avant de valider — voyez
  [Lisez avant de signer](/learn/checking-before-you-sign).
- **Le réseau est public.** Tout le monde peut voir un solde et l'historique d'une adresse.
  Personne ne voit votre nom, sauf si vous le rattachez quelque part à l'adresse : une adresse est
  donc un pseudonyme, pas l'anonymat.
- **Il existe de nombreux réseaux.** Une adresse sur Solana n'est pas la même qu'une adresse sur
  une autre chaîne. Envoyer sur le mauvais réseau signifie en général que l'argent est perdu, car
  personne ne peut le déplacer.

## Une façon sûre de commencer

La plupart des gens apprennent mieux avec de petits montants. Un ordre pratique :

1. Installez un seul portefeuille en lequel vous avez confiance, et rien de plus, depuis le lien
   officiel de la boutique d'applications figurant sur le site du projet.
2. Écrivez la phrase de récupération sur papier. Ne la photographiez pas.
3. Recevez un petit montant, renvoyez-en un petit à vous-même, et voyez à quoi ressemblent les
   frais.
4. Augmentez les montants seulement ensuite, et seulement quand vous êtes à l'aise pour lire
   l'écran de validation.

Si vous êtes sur le réseau de test de Payvand, c'est exactement à cela que servent les jetons : ils
sont gratuits, ils ne valent rien, et les dépenser vous apprend les mêmes gestes que vous
utiliserez plus tard avec de vrais fonds — [il y a un court guide sur l'argent d'entraînement](/learn/test-money-on-devnet).

L'étape suivante, c'est ce qui se passe quand vous payez : [ce qui se passe quand vous payez avec
un portefeuille](/learn/paying-with-a-wallet).
