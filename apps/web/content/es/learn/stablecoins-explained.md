---
title: Las stablecoins, en palabras llanas
description: Por qué un comercio puede poner precios en USDC o EURC sin preocuparse de que el token se mueva, y qué los respalda de verdad.
category: basics
order: 5
---

Casi todo el cripto del que oyes hablar tiene un precio que se mueve todo el día. Es una propiedad
terrible para un café: si el comercio te cobra un SOL y el precio del SOL cae un 5 % antes de que
pagues, el comercio acaba de perder un 5 %.

Una **stablecoin** es un token diseñado para valer siempre una unidad de una moneda corriente. Las dos
que verás en Payvand son **USDC** (un dólar cada uno) y **EURC** (un euro cada uno), ambas emitidas por
Circle. Viven en la red de Solana, que es la fontanería debajo de todos los pagos de aquí —
[qué es Solana, en palabras llanas](/learn/what-is-solana).

## Qué significa «respaldado»

Por cada USDC en circulación, Circle mantiene en reserva alrededor de un dólar en efectivo y deuda
pública a corto plazo, y publica certificaciones periódicas de esas reservas. EURC funciona igual
respecto al euro. Puedes devolver USDC a Circle y recibir dólares, y eso es lo que mantiene el precio
pegado al dólar: si cotizara a 97 céntimos, alguien lo compraría y lo canjearía a la par.

Ahí está también el límite honesto de la promesa. Una stablecoin es tan sólida como su emisor y sus
reservas. Es un derecho sobre una empresa, no dinero de banco central, así que «estable» significa
«anclada», no «sin riesgo».

## Cómo llegan a Solana

USDC y EURC en Solana son **tokens SPL**: tokens estándar emitidos por un programa concreto de la red.
Cada token tiene una dirección de mint que lo identifica, y cada titular tiene una cuenta de token con
un saldo para ese mint. Tu cartera te crea esa cuenta la primera vez que recibes el token, y eso cuesta
una cantidad minúscula de SOL en renta.

Esto importa por una razón práctica: **hay tokens falsos con nombres conocidos**. Un token llamado
«USDC» no es USDC. Las carteras y las apps lo evitan comprobando la dirección del mint, no el nombre.
Payvand fija los dos mints que acepta, así que una factura solo se puede saldar con USDC o EURC de
verdad.

## Por qué gustan a los comercios

- **El precio no se desplaza.** Una factura de 4,50 EURC vale 4,50 euros cuando se emite y cuando se
  paga, así que no hay que recalcular a mitad de conversación.
- **Se liquidan como cualquier otro token.** Segundos, por una fracción de céntimo, sin comisiones de
  tarjeta y sin devoluciones.
- **Son aburridas, y eso es el punto.** Un comercio no quiere especular con su propia caja.

## Por qué gustan a los compradores

Si ya tienes euros o dólares en una stablecoin, pagar con ella no toca tu exposición al precio del SOL
ni de ningún otro token. Se comporta como una cartera de divisa extranjera: el saldo es lo que dice
ser.

## Los tokens de prueba no son dinero

En una red de prueba (la cabecera de Payvand muestra **devnet**), USDC y EURC son tokens de prueba
gratis de un faucet. Tienen el mismo nombre y el mismo comportamiento de mint, y no valen absolutamente
nada. Es a propósito: así aprendes los gestos — financiar un presupuesto, pagar una factura, retirar el
resto — sin arriesgar un céntimo.
