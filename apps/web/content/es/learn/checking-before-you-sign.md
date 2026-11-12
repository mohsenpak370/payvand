---
title: Lee antes de firmar
description: La pantalla de aprobación es el último momento en que alguien puede parar un pago. Qué comprobar y la diferencia entre enviar dinero y dejar que un programa lo gaste después.
category: safety
order: 3
---

Todo pago en cripto pasa por un momento: una pantalla en tu cartera que te pide firmar. Hasta ahí no se
ha movido nada y todo se puede cancelar gratis. Después, el pago es definitivo — en Solana unos 150
milisegundos más tarde, sin deshacer disponible para ti ni para nadie
([por qué es tan rápido](/learn/when-is-a-payment-final)). Trata esa pantalla como la decisión real,
no como un trámite que se pasa de largo.

## Qué contiene de verdad una transacción

Una transacción es una lista de instrucciones más una comisión. En un pago simple la lista es corta:
mover este importe de este token desde mi dirección a aquella. Tu cartera lo convierte en algo que tu
clave puede firmar, y la red lo ejecuta.

Lee cuatro cosas siempre:

1. **El importe** — incluidos los decimales. `0,5` y `5` se parecen a primera vista.
2. **La moneda o el token** — un token puede llamarse como quiera. Si una app dice USDC pero la cartera
   muestra un token que no conoces, algo va mal.
3. **El destinatario** — comprueba el principio y el final de la dirección y, más importante, que sea
   quien esperas. Si le pagas a un comercio, debe ser la dirección de la factura que acabas de escanear,
   no una que pegaste de un chat.
4. **La comisión** — pequeña en Solana, pero una comisión desproporcionada es una señal de que la
   transacción no es lo que crees.

## Enviar no es lo mismo que aprobar

Hay dos cosas muy distintas que una firma puede hacer:

- **Una transferencia** mueve un importe concreto, una vez, a una dirección concreta. Eso es un pago.
- **Una aprobación** da permiso a un programa para mover tokens de tu cuenta más adelante, a veces
  hasta un importe mucho mayor que el pago que tienes delante. Es un permiso permanente, no un pago.

Las aprobaciones existen por buenas razones — cambiar tokens necesita una —, pero también son la
herramienta favorita de los drainers, porque una sola aprobación puede vaciar un saldo mucho después de
que cerraras la pestaña. Si no esperabas una aprobación, cancélala. Si tienes que darla, da un importe
concreto y pequeño en vez de uno ilimitado, y revócala después.

## Por qué aquí te puedes fiar del importe

Las facturas de Payvand meten el importe y el destinatario *dentro de la dirección a la que debe llegar
el pago*. No hay un campo «importe» aparte que un servidor pueda cambiar entre el código QR y tu
aprobación: si pagas otro importe, el pago va a otra dirección, que no es la que vigila el vendedor. Así
el vendedor no puede subir el precio después de que escanees, y una página no puede desviar los fondos
a escondidas.

Cuando pagues, aprueba exactamente lo que muestra la factura. Si la ventana de la cartera no coincide
con la pantalla que escaneaste, para y pregunta — una de las dos no es lo que crees.

## Hábitos que merecen quedarse

- **Firma una cosa cada vez.** Una cola de aprobaciones está diseñada para que dejes de leer.
- **No firmes en un dispositivo que no controlas.** Los ordenadores compartidos y las sesiones de
  soporte remoto son la vía por la que desconocidos «serviciales» vacían carteras.
- **Mejor una cartera hardware para importes altos.** Muestra el importe y el destinatario en su propia
  pantalla, así que un ordenador comprometido no puede cambiarlos a tus espaldas.
- **Ante la duda, rechaza.** Rechazar no cuesta nada y el pago se puede reintentar en un minuto.

Si se firmó algo que no debía, [la lista para un móvil perdido](/learn/if-your-phone-is-lost) es la
misma secuencia de pasos: mover lo que queda, parar la clave antigua y denunciarlo.
