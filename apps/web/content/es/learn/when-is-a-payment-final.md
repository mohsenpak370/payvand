---
title: ¿Cuándo está realmente hecho un pago?
description: Un pago con tarjeta se puede revertir durante semanas. Un pago en Solana se vuelve permanente en menos de un segundo. Qué significa «final» y qué cambió en octubre de 2026.
category: solana
order: 2
---

Aquí va la versión cotidiana de una pregunta técnica. Acercas el móvil para pagar un café. ¿Cuándo
puede el comercio darte la taza con tranquilidad y dejarte salir?

Con tarjeta, la respuesta honesta es «en unos días», y por eso los pagos con tarjeta se pueden
reclamar y los comercios pagan ese riesgo. En Solana la respuesta es ahora **menos de un segundo**, y
lo es desde que una actualización llamada **Alpenglow** entró en la red principal el 21 de octubre
de 2026.

## Tres estados en los que puede estar un pago

- **Visto (processed).** Un ordenador tiene el pago a la vista. Todavía podría ser sustituido si la red
  se reorganiza en torno a otra versión de los hechos. No es seguro fiarse.
- **Confirmado (confirmed).** Suficiente parte de la red está de acuerdo. Antes de Alpenglow esto era
  rápido pero más débil que final — un pago podía quedar confirmado y, en una reorganización rara,
  perderse igualmente.
- **Finalizado (finalized).** Escrito de forma permanente. No se puede deshacer ni reordenar, ni
  siquiera por los validadores que lo escribieron.

Antes de Alpenglow la finalidad tardaba unos **13 segundos** en Solana. Eso ponía a los comercios en un
aprieto: esperar 13 segundos en el mostrador o aceptar «confirmado» y cargar con un pequeño riesgo.
Ninguna de las dos es una buena forma de vender café.

## Qué cambió

Alpenglow sustituyó el sistema de votos antiguo por uno en el que los validadores votan directamente y
sus votos se recogen en un **certificado**. Cuando los validadores con el 80 % del SOL apostado se
ponen de acuerdo en una sola ronda, el pago es final al instante; si no, una segunda ronda lo resuelve.

Los números prácticos:

- **Normalmente unos 150 milisegundos** — alrededor de una séptima parte de segundo — desde que el pago
  se incluye hasta que es final.
- A veces más rápido (unos 100 ms), a veces más lento. El panel en vivo de Solana suele mostrar la
  finalidad en torno a 200 milisegundos.
- Varía según cómo esté repartido el stake, y no se publica un máximo garantizado. Lo que se puede
  afirmar con seguridad es que está **muy por debajo de un segundo**.

Una consecuencia que conviene saber: con Alpenglow, **«confirmed» y «finalized» significan lo mismo**.
Las dos etiquetas siguen existiendo, pero ahora ambas quieren decir confirmado de forma permanente. Solo
«processed» sigue siendo un estado temprano e inseguro.

## Por qué importa en un mostrador

El comercio puede entregar el café en el momento en que la app dice confirmado, porque no queda nada
que esperar. Sin limbo pendiente, sin «ya veremos si entra», sin una pausa de 13 segundos con cola
detrás de ti. Eso es lo que hace que un pago cripto se sienta como acercar una tarjeta en vez de como
una transferencia bancaria.

## La otra cara de «final»

El mismo mecanismo que hace que un pago sea para siempre dinero en manos de un comercio hace que un
error sea un error para siempre. Dirección equivocada, importe equivocado, un sitio que te engañó — una
vez final, es final, y no hay nadie a quien recurrir.

Así que la velocidad y la irreversibilidad son una sola característica, no dos. Merece la pena leer
[qué comprobar antes de firmar](/learn/checking-before-you-sign) justo por eso.

## Qué significa para ti

Cuando Payvand muestra un pago como confirmado, el dinero ha llegado de verdad y sigue llegado — en
menos tiempo del que tardas en volver a guardar el móvil en el bolsillo. La parte que aún merece tu
atención es el medio segundo *antes* de aprobarlo.
