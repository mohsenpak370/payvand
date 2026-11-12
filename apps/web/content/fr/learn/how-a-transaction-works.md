---
title: Ce que contient vraiment une transaction Solana
description: Des signatures, un message, des drapeaux de compte, un blockhash qui expire, des frais en micro-lamports et une boîte de 1 232 octets. L'anatomie, avec les vraies constantes.
category: technical
order: 1
level: technical
---

Une transaction n'est pas une requête envoyée à un serveur. C'est un objet signé qui s'exécute
complètement ou ne change absolument rien. Voici ce qu'elle contient, et pourquoi chaque partie
existe.

## Deux moitiés : des signatures et un message

Une transaction Solana, c'est `signatures` plus un `message`.

- Le **message** contient un en-tête, la liste des adresses de comptes qu'il touche, un blockhash
  récent et une ou plusieurs instructions compilées.
- Les **signatures** portent sur le message sérialisé. La première appartient au payeur des frais,
  et sa signature est l'identifiant de la transaction.

Comme les signatures couvrent tout le message, toute modification d'une liste de comptes, d'une
instruction ou du blockhash les invalide. C'est toute l'histoire de l'inviolabilité à ce niveau.

## La boîte dans laquelle il faut tenir

- Les transactions legacy et v0 sont limitées à **1 232 octets** — la MTU minimale IPv6 (1 280)
  moins 48 octets d'en-têtes. Ce n'est pas une coquetterie de conception : c'est ce qui tient dans
  un paquet réseau.
- Une transaction porte au plus **12 signatures**.
- Elle peut référencer au plus **64 comptes**.
- Le format plus récent **v1** porte la limite à **4 096 octets**.

C'est la limite d'octets, et non le nombre de comptes, qui mord en général en premier. Un message
legacy dépense 32 octets par compte : quelques instructions avec beaucoup de comptes débordent des
1 232 octets bien avant d'atteindre 64.

## Messages versionnés et tables de consultation

Les messages **v0** peuvent pointer vers une **table de consultation d'adresses** : une liste
embarquée sur la chaîne, de 256 clés publiques au maximum. Le message désigne alors ces comptes par
un index d'un octet au lieu de transporter chaque clé de 32 octets.

Le compromis est volontaire : les comptes d'une table de consultation peuvent être modifiables ou en
lecture seule, mais **ne peuvent pas être signataires**. Les signataires et le payeur des frais
restent toujours en clair dans le message, parce qu'une signature doit couvrir la clé qui l'a
produite.

## Instructions et métadonnées de comptes

Chaque instruction nomme un programme et porte :

- une liste ordonnée de **comptes**, chacun avec les drapeaux `is_signer` et `is_writable`, et
- un blob de **données** opaque dont le programme définit la structure.

Les programmes peuvent appeler d'autres programmes pendant l'exécution ; ce sont les
**instructions internes**, enregistrées dans les métadonnées mais jamais sérialisées comme
instructions de premier niveau.

Les drapeaux signataire/modifiable ne sont pas des indications. L'exécuteur les applique : écrire
dans un compte non marqué modifiable échoue, et utiliser un compte comme signataire sans signature
correspondante échoue.

## Le blockhash, et son expiration

Chaque transaction référence un blockhash récent. Il joue deux rôles : il empêche le rejeu et donne
une échéance à la transaction. Un blockhash est utilisable pendant **150 slots** (151 âges, avec
l'indexation à partir de zéro).

Avec le temps de slot actuel de **200 ms**, cela fait environ **30 secondes** — mais la production
de slots peut sauter ou prendre du retard, ce qui explique que les clients suivent
`lastValidBlockHeight` renvoyé par `getLatestBlockhash` au lieu de fixer leur propre minuterie.

## Les frais : une base et un pourboire

- **Frais de base :** 5 000 lamports par signature, soit 0,000005 SOL pour un paiement typique à
  une signature. Ils sont partagés 50/50 entre la destruction et le validateur, et ils sont
  prélevés **même si l'exécution échoue**.
- **Frais de priorité :** fixés par un prix en unités de calcul, en **micro-lamports par UC**, le
  total valant `ceil(prix × limite d'UC demandée / 1 000 000)`. Notez bien *demandée* : vous payez
  la limite que vous demandez, pas le calcul que vous utilisez réellement. Tout va au validateur.

## Le calcul : le vrai budget

- Une instruction non intégrée reçoit **200 000 unités de calcul** par défaut ; une instruction
  intégrée en reçoit 3 000.
- Le plafond pour toute la transaction est de **1 400 000 UC**.
- La limite par bloc de l'ordonnanceur est de **60 000 000 UC** — un chiffre de capacité, pas une
  allocation par transaction.

Les programmes peuvent demander une autre limite avec les instructions de budget de calcul, et c'est
cette demande qui sert de base aux frais de priorité.

## Simulation, prévol, et ce que « envoyé » veut dire

`sendTransaction` vérifie les signatures et **simule** la transaction à un niveau d'engagement donné
avant de la transmettre, sauf si `skipPreflight` est activé. Un échec de prévol est un signal fort
que la transaction échouerait sur la chaîne — d'où le fait que les apps simulent avant d'afficher
une fenêtre de portefeuille.

Mais un prévol réussi n'est pas une confirmation. `sendTransaction` renvoie une signature dès qu'un
RPC accepte la soumission. La transaction n'est réelle qu'une fois dans un bloc et finale.

## L'engagement après Alpenglow

Depuis l'activation d'Alpenglow sur mainnet à l'**époque 1064 (21 octobre 2026)**, `confirmed` et
`finalized` signifient tous deux inscrit définitivement ; `processed` signifie exécuté dans une
banque mais pas encore final. Les notes de mise à jour de Solana recommandent au nouveau code
d'utiliser `finalized`, et indiquent que `confirmed` doit être retiré dans une version ultérieure.
Voyez [l'explication de la finalité](/learn/when-is-a-payment-final) pour ce que cela change au
comptoir.

## Sources

- [Structure d'une transaction](https://solana.com/docs/core/transactions/transaction-structure)
- [Transactions versionnées](https://solana.com/docs/core/transactions/versioned-transactions)
- [Référence des constantes](https://solana.com/docs/core/constants-reference)
- [Structure des frais](https://solana.com/docs/core/fees/fee-structure) · [Budget de calcul](https://solana.com/docs/core/fees/compute-budget)
- [sendTransaction](https://solana.com/docs/rpc/http/sendtransaction) · [simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)
- [Anza : statuts d'engagement](https://docs.anza.xyz/consensus/commitments)
