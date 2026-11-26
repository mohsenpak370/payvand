---
title: Ce que Payvand ajoute à un portefeuille
description: Un portefeuille sait déjà afficher un QR code et envoyer des USDC. Ce qu’il vous laisse, ce sont les vérifications : les faux jetons aux vrais noms, les demandes qui réclament plus qu’un paiement, et savoir à quelle vente correspond un virement.
category: payvand
order: 0
---

Un portefeuille comme Phantom ou Solflare sait déjà envoyer des USDC, des EURC ou des SOL, et la
plupart savent afficher un QR code avec un montant. « Il fait des QR codes » n’est donc pas la raison
d’utiliser Payvand. L’argent circule même de la même façon : directement du portefeuille de
l’acheteur à celui du vendeur, jamais par nous.

La différence tient à tout ce qui entoure le virement. Un portefeuille déplace de l’argent et laisse
les vérifications aux deux personnes qui l’utilisent. Payvand fait ces vérifications à leur place.
Voici ce que cela change concrètement.

## Seul le vrai jeton compte

N’importe qui peut créer un nouveau jeton en quelques minutes, l’appeler « USDC » et lui donner le
même logo. Dans un portefeuille, il ressemble exactement au vrai. Il ne vaut rien.

La seule façon fiable de les distinguer est l’**adresse de mint** du jeton : une suite de 44 lettres
et chiffres qui l’identifie, comme un numéro de série. Le vrai USDC sur Solana commence par
`EPjFWdd5…`, un faux peut commencer par n’importe quoi. Pour être sûr d’avoir été payé en vrai argent,
il faudrait ouvrir le virement et comparer cette suite, caractère par caractère, à chaque fois.
Presque personne ne le fait, et c’est pour cela que l’arnaque fonctionne.

Avec Payvand, vous n’avez jamais à le faire. Les vraies adresses d’USDC et d’EURC, tirées de la liste
publiée par Circle elle-même, sont intégrées à Payvand, et le jeton fait partie de l’adresse que le
paiement doit atteindre. Un paiement dans un jeton imité part vers une autre adresse, que ce code ne
surveille pas, et la facture reste **impayée**. S’il est écrit Payé, c’est le vrai jeton qui est
arrivé. Pour comprendre comment fonctionnent des jetons comme l’USDC, voir
[les stablecoins, en mots simples](/learn/stablecoins-explained).

## Le client approuve un paiement tout simple

Tout paiement crypto se termine par un écran du portefeuille qui demande au client d’approuver. En
général, rien d’inquiétant. Mais une signature peut faire plus qu’envoyer de l’argent. Elle peut
aussi donner à un programme une **autorisation permanente** de déplacer des fonds plus tard, et c’est
ainsi que fonctionnent les « drainers » : une demande qui semble banale permet à quelqu’un d’autre de
vider le portefeuille ensuite.

Avec un virement ordinaire depuis un portefeuille, le client doit en plus saisir ou coller lui-même
l’adresse et le montant, et un seul caractère faux envoie l’argent ailleurs, pour de bon.

Une demande Payvand n’est jamais qu’une seule chose : **payer ce montant, dans ce jeton, à ce
vendeur**. Le montant, le jeton et le destinataire sont déjà remplis à partir du code : rien à
saisir, à coller ou à mal lire, et aucune autorisation supplémentaire cachée dedans. Ce que le client
voit dans son portefeuille, c’est ce qui se passe.

Deux limites, en toute honnêteté :

- Payvand rend sa propre demande simple et claire. Il ne peut pas empêcher un client de signer
  quelque chose de nuisible sur un autre site : le réflexe de [lisez avant de signer](/learn/checking-before-you-sign)
  reste utile, tout comme les signaux de [comment reconnaître une arnaque crypto](/learn/spotting-crypto-scams).
- L’[autorisation mensuelle](/learn/monthly-allowance-explained), facultative, est une autorisation,
  volontairement. C’est le client qui l’ouvre, avec une limite qu’il fixe ; elle ne peut que payer des
  factures et se ferme à tout moment.

## Le prix et la devise ne peuvent pas changer

Le vendeur fixe le prix, la devise et une courte note une seule fois, en créant le code. Le montant
fait partie de l’adresse de paiement elle-même : un client ne peut pas payer moins, et une page ne
peut pas changer le prix une fois le code scanné. Un autre montant partirait vers une adresse que ce
code ne surveille pas. [Ce qui se passe quand vous payez avec un portefeuille](/learn/paying-with-a-wallet)
le détaille étape par étape.

## Le vendeur sait qui a payé, et pour quoi

Un portefeuille affiche l’argent reçu comme une liste de montants venant d’adresses inconnues. Les
rapprocher des ventes se fait à l’œil : quel 4,50 était le café, et lequel le croissant ?

Un code Payvand surveille son propre paiement. Il passe seul à **Payé** dès que l’argent arrive, sur
l’écran du vendeur comme sur celui du client, et conserve l’adresse du payeur et la transaction comme
reçu. Pas de capture d’écran « regardez, je l’ai envoyé » à croire sur parole. Une capture se
falsifie ; un code passé à Payé, non.

## Le client n’a pas besoin de SOL

Envoyer des USDC depuis un portefeuille demande un peu de SOL pour les frais de réseau. Un client qui
a beaucoup d’USDC et aucun SOL ne peut pas payer du tout, ce qui surprend presque tout le monde la
première fois. En USDC ou en EURC avec Payvand, notre relayer paie les frais de réseau et se rembourse
sur les frais inclus dans le prix : le client n’a besoin que du jeton dans lequel il paie.
[Ce que coûte un paiement](/learn/what-a-payment-costs) montre où va chaque part.

## Et c’est toujours votre portefeuille

Rien de tout cela n’exige que Payvand détienne quoi que ce soit. Il n’y a pas de solde Payvand ni de
phrase de récupération à garder, et l’application n’a aucune clé capable de déplacer vos fonds.
Chaque paiement va directement dans le portefeuille du vendeur. Payvand est la couche au-dessus du
portefeuille auquel vous faites déjà confiance : il ajoute les vérifications et la comptabilité, et
l’argent reste où il était.
