---
title: Comptes, propriété et PDA
description: Sur Solana, tout est un compte à cinq champs avec un propriétaire. Pourquoi les programmes sont sans état, pourquoi un PDA n'a pas de clé privée, et où vit vraiment un solde de jetons.
category: technical
order: 2
level: technical
---

Solana n'a pas d'objet d'état global ni de stockage par contrat. Il a des **comptes**, et chaque
donnée que le réseau retient est un compte.

## Les cinq champs

Chaque compte est identifié par une adresse de 32 octets et a la même forme :

| Champ | Signification |
|---|---|
| `lamports` | Solde en lamports (1 SOL = 1 000 000 000) |
| `data` | Octets arbitraires — état du programme ou code du programme |
| `owner` | Le programme autorisé à modifier `data` ou à débiter `lamports` |
| `executable` | Indique si ce compte contient un programme |
| `rent_epoch` | Obsolète ; les comptes exonérés de loyer valent `u64::MAX` |

Deux règles expliquent l'essentiel de la conception de Solana :

1. **Seul le programme propriétaire peut modifier les données d'un compte ou débiter ses
   lamports.** N'importe qui peut *créditer* des lamports sur un compte modifiable, et c'est
   comment un paiement fonctionne sans que le programme du destinataire soit impliqué.
2. **La taille d'un compte est fixée à sa création.** L'agrandir suppose une réallocation avec
   assez de lamports pour rester exonéré de loyer, dans la limite de **10 Mio** par compte.

## Le loyer, et pourquoi il y a un dépôt

Stocker des données coûte de vraies ressources aux validateurs : un compte doit donc détenir un
solde remboursable de `(taille_données + 128) × 3 480 lamports × 2 ans`. Un compte de jetons
classique de 165 octets demande **2 039 280 lamports** (≈0,00204 SOL) ; une émission de 82 octets
en demande **1 461 600**.

Le prélèvement continu du loyer est désactivé ; il s'agit désormais d'un dépôt de stockage
remboursable. Fermez le compte et les lamports vous reviennent. Interrogez
`getMinimumBalanceForRentExemption` pour le chiffre du moment.

## Les programmes sont sans état

Un programme est un compte avec `executable = true`, écrit dans un bytecode que l'exécuteur sait
lancer (sBPF). Il ne s'exécute que sur les comptes qu'une instruction lui fournit, et il ne peut
modifier que ceux qu'il possède.

C'est pourquoi le « stockage de contrat intelligent » n'existe pas ici. La mémoire d'un programme,
c'est l'ensemble des comptes qu'on lui passe ; son état persistant vit dans des **comptes de
données** séparés qu'il possède. Quand on dit qu'un programme est sans état, c'est exactement cela.

Les programmes sont déployés et mis à jour via un programme de chargement qui possède le compte
exécutable ; l'autorité de mise à jour est une clé séparée, et en perdre le contrôle, c'est perdre la
possibilité de corriger le programme.

## Les PDA, des adresses que personne ne peut signer

Une **adresse dérivée de programme** est calculée à partir de `seeds + program_id + bump`, puis
vérifiée comme **hors de la courbe Ed25519**. Jusqu'à 16 graines, de 32 octets chacune au maximum.

Le point, c'est le « hors courbe » : aucune clé privée ne correspond à une telle adresse, donc
aucune signature ordinaire ne peut exister pour elle. À la place, lors d'un appel inter-programmes,
le programme qui dérive l'adresse renvoie les graines et l'exécuteur vérifie qu'elles produisent
bien le PDA, ce qui lui donne le privilège de signataire **pour cet appel seulement**. C'est
`invoke_signed`.

C'est ainsi qu'un programme contrôle des fonds sans détenir de clé. Les autorisations de Payvand
fonctionnent de cette façon : la cagnotte est un PDA possédé par le programme, donc aucune personne
et aucun serveur n'en détient la clé, et dépenser exige que les règles du programme soient
respectées.

Notez les deux étapes distinctes qu'on confond souvent : *dériver* une adresse est du calcul pur,
et *créer* un compte à cette adresse coûte des lamports et une instruction du programme système.

## Où vit réellement un solde de jetons

Il y a trois choses distinctes :

- **Un compte d'émission** décrit un jeton : la masse en circulation, `decimals`, et
  éventuellement les autorités d'émission et de gel. Les montants sont stockés en unités de base ;
  les décimales ne servent qu'à l'affichage.
- **Un compte de jetons** détient des unités d'une seule émission. Son propre champ `owner` nomme
  l'autorité qui peut les déplacer. Le compte lui-même appartient au programme de jetons (ou à
  Token-2022).
- **Une adresse de portefeuille** n'est en général qu'un compte de paire de clés sur courbe qui
  détient des SOL. Ce n'est pas là que se trouvent vos USDC.

Le **compte de jetons associé** (ATA) est le compte de jetons conventionnel pour une combinaison
donnée de portefeuille + émission + programme de jetons, dérivé de façon déterministe pour que
chacun puisse le retrouver. Le programme Token-2022 étend ce modèle avec des extensions de compte et
d'émission facultatives, qui changent la taille des comptes et ajoutent des fonctions comme les
frais de transfert ou les intérêts.

## Pourquoi les drapeaux d'écriture comptent pour le débit

L'exécuteur utilise **Sealevel** pour exécuter les transactions en parallèle. Les transactions qui
ne font que *lire* un compte le partagent librement ; celles qui *écrivent* le même compte doivent
être sérialisées. Un compte modifiable très sollicité — un compteur global, par exemple — est donc
un goulot d'étranglement, tandis que des comptes de données fragmentés ne le sont pas.

C'est une contrainte de conception, pas un réglage. C'est pourquoi vous verrez l'état découpé par
utilisateur, par émission et par mois plutôt que gardé dans un seul grand compte.

## Sources

- [Comptes](https://solana.com/docs/core/accounts) · [Structure d'un compte](https://solana.com/docs/core/accounts/account-structure) · [Règles de modification](https://solana.com/docs/core/accounts/modification-rules)
- [Programmes](https://solana.com/docs/core/programs) · [Programmes intégrés](https://solana.com/docs/core/programs/builtin-programs)
- [PDA](https://solana.com/docs/core/pda) · [Dérivation de PDA](https://solana.com/docs/core/pda/pda-derivation) · [CPI](https://solana.com/docs/core/cpi)
- [Jetons](https://solana.com/docs/tokens) · [Comptes de jetons](https://solana.com/docs/tokens/basics/create-token-account) · [Extensions Token-2022](https://solana.com/docs/tokens/extensions)
- [Anza : comptes en lecture seule et Sealevel](https://docs.anza.xyz/implemented-proposals/readonly-accounts)
