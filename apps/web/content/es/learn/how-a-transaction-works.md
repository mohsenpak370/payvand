---
title: Qué hay realmente dentro de una transacción de Solana
description: Firmas, un mensaje, indicadores de cuenta, un blockhash con caducidad, una comisión en micro-lamports y una caja de 1.232 bytes. La anatomía, con las constantes reales.
category: technical
order: 1
level: technical
---

Una transacción no es una petición a un servidor. Es un objeto firmado que o se ejecuta del todo o no
cambia absolutamente nada. Aquí está lo que lleva dentro y por qué existe cada parte.

## Dos mitades: firmas y un mensaje

Una transacción de Solana es `signatures` más una `message`.

- El **message** contiene una cabecera, la lista de direcciones de cuenta que toca, un blockhash
  reciente y una o más instrucciones compiladas.
- Las **signatures** cubren el message serializado. La primera pertenece a quien paga la comisión, y su
  firma es el ID de la transacción.

Como las firmas cubren el message entero, cualquier cambio en una lista de cuentas, en una instrucción o
en el blockhash las invalida. Eso es toda la resistencia a la manipulación en esta capa.

## La caja en la que tiene que caber

- Las transacciones legacy y v0 están limitadas a **1.232 bytes** — la MTU mínima de IPv6 (1.280) menos
  48 bytes de cabeceras. No es un adorno de diseño: es lo que cabe en un paquete de red.
- Una transacción lleva como máximo **12 firmas**.
- Puede referenciar como máximo **64 cuentas**.
- El formato más nuevo, **v1**, eleva el límite a **4.096 bytes**.

Normalmente lo que aprieta primero es el límite de bytes, no el número de cuentas. Un message legacy gasta
32 bytes por cuenta, así que unas pocas instrucciones con muchas cuentas desbordan 1.232 bytes mucho antes
de llegar a 64.

## Mensajes versionados y tablas de consulta

Los mensajes **v0** pueden apuntar a una **address lookup table**: una lista on-chain de hasta 256 claves
públicas. El mensaje se refiere entonces a esas cuentas con un índice de un byte en lugar de llevar cada
clave de 32 bytes.

El compromiso es deliberado: las cuentas de una tabla de consulta pueden ser escribibles o de solo
lectura, pero **no pueden ser firmantes**. Los firmantes y quien paga la comisión se quedan siempre
inline en el mensaje, porque una firma tiene que cubrir la clave que la produjo.

## Instrucciones y metadatos de cuenta

Cada instrucción nombra un programa y lleva:

- una lista ordenada de **cuentas**, cada una con los indicadores `is_signer` e `is_writable`, y
- un bloque **data** opaco cuyo formato define el programa.

Los programas pueden invocar otros programas durante la ejecución; esas son las **instrucciones
internas**, registradas en metadatos pero nunca serializadas como instrucciones de primer nivel.

Los indicadores de firmante y de escritura no son sugerencias. El runtime los impone: escribir en una
cuenta que no estaba marcada como escribible falla, y usar una cuenta como firmante sin la firma
correspondiente falla.

## El blockhash y su caducidad

Toda transacción referencia un blockhash reciente. Cumple dos funciones: evita la repetición y le da a la
transacción una fecha límite. Un blockhash sirve durante **150 slots** (151 edades, por la indexación
desde cero).

Con la duración actual de slot de **200 ms** eso son unos **30 segundos** — aunque la producción de slots
puede saltarse o retrasarse, y por eso los clientes siguen `lastValidBlockHeight` de
`getLatestBlockhash` en lugar de poner su propio temporizador.

## Comisiones: una base y una propina

- **Comisión base:** 5.000 lamports por firma, es decir, 0,000005 SOL para un pago típico de una firma.
  Se reparte 50/50 entre quema y validador, y se cobra **aunque la ejecución falle**.
- **Comisión de prioridad:** se fija con un precio por unidad de cómputo en
  **micro-lamports por CU**, con el total igual a `ceil(precio × límite de CU solicitado / 1.000.000)`.
  Ojo a *solicitado*: pagas por el límite que pides, no por el cómputo que usas. Va entera al validador.

## Cómputo: el presupuesto real

- Una instrucción que no es del sistema recibe **200.000 unidades de cómputo** por defecto; una del
  sistema, 3.000.
- El tope por transacción es de **1.400.000 CU**.
- El límite del planificador por bloque es de **60.000.000 CU** — una cifra de capacidad, no un permiso
  por transacción.

Los programas pueden pedir otro límite con las instrucciones de compute budget, y contra esa petición se
calcula la comisión de prioridad.

## Simulación, preflight y qué significa «enviado»

`sendTransaction` verifica las firmas y **simula** la transacción contra un nivel de commitment antes de
reenviarla, salvo que se ponga `skipPreflight`. Que el preflight falle es una señal fuerte de que la
transacción fallaría on-chain — por eso las apps simulan antes de mostrar una ventana de cartera.

Pero un preflight correcto no es una confirmación. `sendTransaction` devuelve una firma en cuanto un RPC
acepta el envío. La transacción solo es real cuando está en un bloque y es final.

## Commitment después de Alpenglow

Desde que Alpenglow se activó en mainnet en la **época 1064 (21 de octubre de 2026)**, `confirmed` y
`finalized` significan ambos confirmado de forma permanente; `processed` significa ejecutado en un banco
pero aún no final. Las notas de la actualización recomiendan que el código nuevo use `finalized`, y dicen
que `confirmed` está previsto para eliminarse en una versión posterior. Mira
[la explicación de la finalidad](/learn/when-is-a-payment-final) para saber qué cambia eso en un
mostrador.

## Fuentes

- [Transaction structure](https://solana.com/docs/core/transactions/transaction-structure)
- [Versioned transactions](https://solana.com/docs/core/transactions/versioned-transactions)
- [Constants reference](https://solana.com/docs/core/constants-reference)
- [Fee structure](https://solana.com/docs/core/fees/fee-structure) · [Compute budget](https://solana.com/docs/core/fees/compute-budget)
- [sendTransaction](https://solana.com/docs/rpc/http/sendtransaction) · [simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
