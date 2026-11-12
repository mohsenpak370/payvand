---
title: Qué pasa cuando pagas con una cartera
description: Desde escanear el código hasta que llega el dinero — los seis pasos de todo pago cripto y qué estás aprobando en realidad.
category: basics
order: 4
---

Un pago cripto se siente como un pago con tarjeta porque la pantalla del comercio se comporta igual.
Por dentro es mucho más corto: no hay red de tarjetas, ni adquiriente, ni lote de liquidación. Aquí
está el camino completo, en el orden en que ocurre.

## 1. Recibes una solicitud de pago

En Payvand, el vendedor escribe un importe y te enseña un código QR. El código no es una imagen de un
pago — *es* el pago: la dirección del vendedor, el importe, la moneda y una referencia corta que
distingue esta factura de cualquier otra del mismo vendedor. Cuando lo escaneas, tu cartera lee esos
datos directamente.

Nada de la solicitud del vendedor se ha «enviado a un servidor» que pudiera cambiar el importe después.
El importe está metido en la dirección a la que debe llegar el dinero.

## 2. Tu cartera te enseña lo que estás a punto de aprobar

La ventana de la cartera lista el pago. Es la pantalla más importante de cripto, así que léela:
**importe, moneda, destinatario, comisión de red**. Si la pantalla dice algo distinto de la del
vendedor, para y averigua por qué antes de firmar. El hábito está en
[Lee antes de firmar](/learn/checking-before-you-sign).

## 3. Apruebas y la cartera firma

Aprobar no significa «decirle a la app que pague». Desbloquea la clave privada en tu dispositivo el
tiempo justo para firmar esta transacción y la vuelve a cerrar. La app que usas nunca ve la clave;
recibe una firma que solo vale para esa transacción exacta.

## 4. La transacción va a la red

La transacción firmada se envía a la red y se incluye en un bloque. En Solana normalmente tarda un par
de segundos en aparecer, y el pago queda *definitivamente final* en bastante menos de un segundo —
desde la actualización Alpenglow de octubre de 2026, normalmente unos 150 milisegundos
([qué significa la finalidad](/learn/when-is-a-payment-final)). Puedes verlo por el enlace de
confirmación que te muestra la cartera o la app.

## 5. El lado del vendedor se actualiza

Como la red es pública, la página del vendedor puede vigilar su dirección y ver llegar el pago sin que
nadie se lo cuente. En Payvand la factura pasa a **Pagado** en ese momento — el mismo segundo en que tu
cartera te confirma.

## 6. La comisión

Toda transacción paga una comisión pequeña de red. En Solana la comisión base es de 5.000 lamports — es
decir, 0,000005 SOL, una fracción de céntimo — más una comisión de prioridad opcional cuando la red
está cargada. La comisión va a la red, no a Payvand ni al vendedor.

Sí necesitas un poco de SOL en la cartera para pagarla. Una cartera con mucho saldo en stablecoins y
cero SOL no puede mover nada, cosa que sorprende la primera vez.

## Qué no es «Pagado»

- **No es reversible.** No hay devolución. Una vez que la red confirma, la transferencia se queda, así
  que la pantalla de aprobación merece tu atención.
- **No es instantáneo en todas partes.** Solana confirma en segundos, pero otras redes pueden tardar
  minutos, y un pago solo es definitivo cuando está confirmado.
- **No es anónimo.** La transferencia es visible para cualquiera que conozca las dos direcciones.

## El atajo de Payvand: un presupuesto mensual

Aprobar cada pago cansa en una cafetería. Payvand te deja aprobar **una** transacción que aparta un
presupuesto mensual en un token como USDC y registra una clave de sesión que guarda la app en tu
dispositivo. A partir de ahí, los pagos que caben en el presupuesto los firma la clave de sesión, así
que en el mostrador no hay ninguna ventana de cartera.

Lo que esa clave puede hacer es deliberadamente estrecho: puede pagar facturas hasta el presupuesto que
fijaste, este mes, y nada más ([el presupuesto en palabras llanas](/learn/monthly-allowance-explained)).
No puede enviar fondos a donde quiera, no puede pasarse del límite y puedes cerrar el presupuesto
cuando quieras para devolver el saldo restante a tu cartera. Si ese intercambio no te convence, paga
cada vez desde tu cartera — los dos caminos pagan las mismas facturas.
