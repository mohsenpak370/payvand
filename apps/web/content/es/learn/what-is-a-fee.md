---
title: Por qué cada pago cuesta una fracción de céntimo
description: Quién cobra la comisión, por qué el precio del código es el coste completo y por qué un pago fallido cuesta algo de todas formas.
category: basics
order: 6
---

Cada pago es una línea nueva escrita en el cuaderno compartido, y escribirla cuesta trabajo real de un
ordenador. La **comisión** es lo que paga ese trabajo.

En Solana la comisión base es de **5.000 lamports**, es decir, 0,000005 SOL — una fracción de céntimo.
Se cobra por firma, así que un pago sencillo es una comisión.

## Cómo se compara con una tarjeta

Con tarjeta, el comercio paga aproximadamente entre un 1 % y un 3 % de cada venta a la red de tarjetas,
más un alquiler mensual del datáfono. En un pago de 4,50 son unos céntimos que desaparecen del lado del
comercio, y por eso los comercios pequeños a veces no aceptan tarjeta por debajo de un mínimo.

Una comisión cripto es distinta en dos cosas:

- **Es plana.** Pagar 2 o pagar 2.000 cuesta la misma cantidad minúscula, porque el trabajo es el
  mismo.
- **La paga el vendedor, con cargo al precio.** Sin alquiler de datáfono, sin contrato mensual y sin
  nada añadido: quien paga entrega exactamente el precio del código, y la comisión de red y la renta
  del recibo salen de la parte del vendedor.

## Por qué no necesitas nada de SOL aunque pagues en USDC

La comisión se paga siempre en la moneda propia de la red, en SOL. Piénsalo como el sello: la carta
puede llevar dólares, pero el sello tiene que ser del servicio postal.

Comprar ese sello no tiene por qué correr de tu cuenta. El relé de Payvand paga la comisión de red y
adelanta las pequeñas rentas del pago y de su recibo en la cadena, y la misma transacción se las
devuelve con cargo al precio. Una cartera llena de USDC y con cero SOL puede pagar, y su saldo baja
exactamente el precio.

La comisión se cuenta por firma: una liquidación lleva una cuando la cartera paga por su cuenta y dos
cuando la envía un relé. El precio se cobra por exactamente ese número de firmas, así que un relé
nunca se queda con más de lo que gastó. Solo cuando ningún relé acepta el pago adelanta la cartera
esos costes, y la misma transacción se los devuelve.

## Cuando la red está cargada

Solana permite además una **comisión de prioridad** opcional — una propina pequeña para entrar antes,
parecido a pagar un envío urgente. Casi siempre no hace falta, y Payvand no la añade a los pagos
normales.

## Por qué un pago fallido cuesta algo de todas formas

Si una transacción llega a la red y falla, los cambios se deshacen pero la comisión se cobra igual,
porque el trabajo ya se hizo. Es una cantidad minúscula, pero es una razón para no machacar un botón
que falla siempre — arregla primero la causa.

## Otro pequeño coste: el depósito de una cuenta

Crear una cuenta nueva en la red requiere un pequeño depósito en SOL que se mantiene mientras la cuenta
existe. Es **reembolsable** — cerrar la cuenta lo devuelve. La reserva mensual de Payvand lo hace una
vez, y por eso abrir un presupuesto cuesta algo más que el propio límite.

## Qué significa para ti

Las comisiones no son algo que preocuparte ni presupuestar. El precio del código es el coste
completo: la comisión de red y la renta del recibo ya están dentro, y siguen siendo tan pequeños que
es más fácil ignorarlos que calcularlos.

La comisión de red es solo la parte más pequeña de lo que cuesta un pago. A dónde va el resto — la parte del protocolo, el recibo permanente en la cadena y los gastos que mantienen el servicio — está en [lo que cuesta un pago](/learn/what-a-payment-costs).
