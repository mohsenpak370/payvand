---
title: Cuentas, propiedad y PDA
description: En Solana todo es una cuenta con cinco campos y un dueño. Por qué los programas no tienen estado, por qué un PDA no tiene clave privada y dónde vive de verdad un saldo de tokens.
category: technical
order: 2
level: technical
---

Solana no tiene un objeto de estado global ni almacenamiento por contrato. Tiene **cuentas**, y cada dato
que la red recuerda es una.

## Los cinco campos

Toda cuenta se identifica por una dirección de 32 bytes y tiene la misma forma:

| Campo | Significado |
|---|---|
| `lamports` | Saldo en lamports (1 SOL = 1.000.000.000) |
| `data` | Bytes arbitrarios — estado del programa o código del programa |
| `owner` | El programa autorizado a modificar `data` o a debitar `lamports` |
| `executable` | Si esta cuenta contiene un programa |
| `rent_epoch` | Obsoleto; las cuentas exentas de renta usan `u64::MAX` |

Dos reglas explican la mayor parte del diseño de Solana:

1. **Solo el programa propietario puede modificar los datos de una cuenta o debitar sus lamports.**
   Cualquiera puede *abonar* lamports a una cuenta escribible, y así funciona un pago sin que intervenga
   el programa del destinatario.
2. **El tamaño de una cuenta es fijo cuando se crea.** Ampliarla significa reasignar con lamports
   suficientes para seguir exenta de renta, hasta un máximo de **10 MiB** por cuenta.

## Renta, y por qué hay un depósito

Guardar datos cuesta recursos reales a los validadores, así que una cuenta debe mantener un saldo
reembolsable: `(data_size + 128) × 3.480 lamports × 2 años`. Una cuenta de token clásica de 165 bytes
necesita **2.039.280 lamports** (≈0,00204 SOL); una mint de 82 bytes necesita **1.461.600**.

El cobro continuo de renta está desactivado; hoy esto es un depósito de almacenamiento reembolsable.
Cierra la cuenta y los lamports vuelven a ti. Para la cifra actual, consulta
`getMinimumBalanceForRentExemption`.

## Los programas no tienen estado

Un programa es una cuenta con `executable = true`, escrita en un bytecode que el runtime puede ejecutar
(sBPF). Se ejecuta solo sobre las cuentas que le pasa una instrucción, y solo puede cambiar aquellas que
posee.

Por eso aquí no existe el «almacenamiento de contrato inteligente». La memoria de un programa es el
conjunto de cuentas que recibe; su estado persistente vive en **cuentas de datos** separadas que el
programa posee. Cuando se dice que un programa no tiene estado, se dice exactamente eso.

Los programas se despliegan y actualizan a través de un programa cargador que posee la cuenta ejecutable;
la autoridad de actualización es una clave aparte, y perder su control significa perder la capacidad de
arreglar el programa.

## PDAs, direcciones por las que nadie puede firmar

Una **program derived address** se calcula a partir de `seeds + program_id + bump` y luego se comprueba
que quede **fuera de la curva Ed25519**. Hasta 16 seeds, cada uno como máximo de 32 bytes.

Estar fuera de la curva es el punto: no hay ninguna clave privada correspondiente a esa dirección, así
que no puede existir nunca una firma ordinaria para ella. En su lugar, durante una invocación entre
programas, el programa que la deriva pasa las seeds de vuelta y el runtime verifica que hashean al PDA,
concediéndole privilegio de firmante **solo para esa invocación**. Eso es `invoke_signed`.

Así controla un programa fondos sin tener una clave. Los presupuestos de Payvand funcionan así: la
reserva es un PDA propiedad del programa, de modo que ni una persona ni un servidor tienen una clave
para ella, y gastar exige que se cumplan las reglas del propio programa.

Fíjate en los dos pasos que la gente confunde: *derivar* una dirección es pura aritmética, y *crear* una
cuenta en ella cuesta lamports y una instrucción del programa del sistema.

## Dónde vive de verdad un saldo de tokens

Hay tres cosas distintas:

- **Una cuenta mint** describe un token: suministro, `decimals` y autoridades opcionales de mint y de
  congelación. Los importes se guardan en unidades base; los decimales solo afectan a la visualización.
- **Una cuenta de token** mantiene unidades de exactamente una mint. Su propio campo `owner` nombra la
  autoridad que puede moverlas. La cuenta en sí pertenece al Token Program (o a Token-2022).
- **Una dirección de cartera** suele ser solo una cuenta de par de claves en la curva que mantiene SOL.
  Ahí no está tu USDC.

La **associated token account** (ATA) es la cuenta de token convencional para una combinación de
cartera, mint y programa de tokens, derivada de forma determinista para que cualquiera pueda encontrarla.
El programa Token-2022 amplía este modelo con extensiones opcionales de cuenta y de mint, que cambian el
tamaño de la cuenta y añaden funciones como comisiones de transferencia o intereses.

## Por qué los indicadores de escritura importan para el rendimiento

El runtime usa **Sealevel** para ejecutar transacciones en paralelo. Las transacciones que solo *leen*
una cuenta la comparten libremente; las que *escriben* la misma cuenta deben serializarse. Una cuenta
escribible muy solicitada — un contador global, por ejemplo — es por tanto un cuello de botella de
rendimiento, mientras que cuentas de datos fragmentadas no lo son.

Esto es una restricción de diseño, no un detalle de ajuste. Es la razón por la que verás el estado
repartido por usuario, por mint y por mes en lugar de guardado en una sola cuenta grande.

## Fuentes

- [Accounts](https://solana.com/docs/core/accounts) · [Account structure](https://solana.com/docs/core/accounts/account-structure) · [Modification rules](https://solana.com/docs/core/accounts/modification-rules)
- [Programs](https://solana.com/docs/core/programs) · [Builtin programs](https://solana.com/docs/core/programs/builtin-programs)
- [PDAs](https://solana.com/docs/core/pda) · [PDA derivation](https://solana.com/docs/core/pda/pda-derivation) · [CPI](https://solana.com/docs/core/cpi)
- [Tokens](https://solana.com/docs/tokens) · [Token accounts](https://solana.com/docs/tokens/basics/create-token-account) · [Token-2022 extensions](https://solana.com/docs/tokens/extensions)
- [Anza: read-only accounts and Sealevel](https://docs.anza.xyz/implemented-proposals/readonly-accounts)
