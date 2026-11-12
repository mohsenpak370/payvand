---
title: Cómo se escribe y se comparte el registro
description: Slots, shreds, raíces de Merkle, codificación de borrado y una mempool que no existe. La maquinaria entre «firmado» y «todos de acuerdo».
category: technical
order: 3
level: technical
---

Una transacción firmada solo es una afirmación hasta que un productor de bloques la incluye. Esta página
trata de la fontanería que convierte afirmaciones en una historia compartida y verificable.

## Slots y bloques

El tiempo en Solana se divide en **slots** de **200 ms**, cada uno con un líder designado, y los líderes
se programan con una época entera de antelación. Un slot es una oportunidad de producir un bloque, no una
garantía: los slots se pueden saltar y un líder puede producir más de un bloque candidato.

La consecuencia práctica es que «el bloque actual» es difuso durante unos cientos de milisegundos. Las
transacciones no hacen cola en una mempool esperando inclusión — no hay mempool. Se reenvían al líder que
se espera que produzca el siguiente slot y o entran en ese bloque o caducan.

## Por qué los bloques viajan como shreds

Un bloque es demasiado grande para entregarlo entero a cada validador. El líder lo **divide en shreds**:

- Los data shreds llevan fragmentos de entradas serializadas, de unos 1.280 bytes cada uno.
- Los coding shreds se generan con **codificación de borrado Reed-Solomon** sobre grupos de data shreds,
  conocidos como FEC sets. Un set puede contener hasta 67 data shreds; 32 es la configuración
  recomendada.
- Todos los shreds los firma el productor del bloque.
- Cada FEC set se merkeliza: las hojas se hashean por pares hasta una **raíz de Merkle**, y el líder firma
  esa raíz. Como un shred lleva un camino de prueba, cualquier nodo puede verificar una pieza sin tener el
  bloque completo, y las piezas que faltan se pueden reconstruir desde los coding shreds.

## Turbine, el árbol de difusión

Turbine distribuye esos shreds por un árbol ponderado por stake en lugar de a todos a la vez. El líder
emite a un nodo raíz, que se abre a la capa 1, cuyos nodos se abren a la capa 2, y así sucesivamente. La
raíz rota **en cada shred**, y la lista de pares se rebaraja por shred para dificultar los ataques
dirigidos.

Otros dos papeles se confunden fácilmente con este:

- **Gossip** es el plano de control: descubrimiento de pares, información de contacto y metadatos de
  validadores. No lleva transacciones.
- **Gulf Stream** es la vía de entrada: clientes y validadores reenvían transacciones al próximo líder que
  ya conocen, y por eso Solana no necesita mempool y por eso el blockhash de un cliente tiene que ser
  reciente.

## Proof of History, después de Alpenglow

PoH era un retardo verificable: una cadena de hashes que permitía a cualquiera comprobar el orden y el
paso del tiempo sin fiarse de un reloj. Durante años también **marcó el ritmo** de la producción de
bloques.

Ese papel de marcapasos ha desaparecido. Desde Alpenglow, los timeouts y certificados de Votor deciden
cuándo se finalizan los bloques, y las tick entries que llevaban el ritmo de PoH han salido del flujo de
shreds. La construcción de hashing de PoH sigue existiendo en la historia del registro, pero ya no es lo
que hace funcionar la red.

## Bifurcaciones, en breve

Como los slots se pueden saltar y los líderes pueden producir bloques competidores, la red puede mantener
temporalmente más de una visión de la historia reciente. Los validadores convergen en una rama y, desde
Alpenglow, la finalización es un certificado explícito en lugar de los lockouts acumulados de TowerBFT.

Lo que importa para una app: una transacción vista en un bloque está *processed*, que no es final. Solo un
bloque finalizado es irreversible. En Solana moderna esa distinción se resuelve en unos 150 ms, pero
sigue siendo la diferencia entre «probablemente» y «con certeza».

## Qué explica esto

- **Por qué hay un límite de 1.232 bytes por transacción**: es un paquete de red, y un bloque es un flujo
  de paquetes.
- **Por qué un pago falla de vez en cuando con un error de blockhash**: la transacción sobrevivió a su
  ventana de validez de unos 150 slots antes de que alguien la incluyera.
- **Por qué la finalidad es una propiedad del consenso y no de la red**: la propagación es cómo se
  extiende un bloque, el consenso es cómo se vuelve permanente. Son sistemas separados por diseño, y
  precisamente eso permitió que Votor sustituyera el consenso sin tocar cómo viajan los bloques.

## Fuentes

- [Anza: Turbine block propagation](https://docs.anza.xyz/consensus/turbine-block-propagation)
- [Shred specification](https://raw.githubusercontent.com/solana-foundation/specs/main/p2p/shred.md)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
- [Alpenglow: PoH and tick entries](https://solana.com/upgrades/alpenglow)
- [Gulf Stream](https://solana.com/news/8-innovations-that-make-solana-the-first-web-scale-blockchain)
