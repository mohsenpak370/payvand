---
title: Staking, validadores y épocas
description: Cuentas de stake, cuentas de voto, warmup, comisiones y un calendario de líderes fijado con un día de antelación. Cómo reparte de verdad el proof of stake de Solana el derecho a escribir bloques.
category: technical
order: 5
level: technical
---

El presupuesto de seguridad de Solana es SOL apostado. Entender la mecánica del staking explica por qué
la red se comporta como se comporta bajo carga, y qué significa de verdad «validador».

## Dos cuentas, dos trabajos

- Una **cuenta de stake** guarda el SOL de quien delega y registra a qué **cuenta de voto** de qué
  validador está delegado. Ahí vive el peso económico.
- Una **cuenta de voto** pertenece a un validador y registra sus votos y sus créditos de voto. No guarda
  el stake delegado.

Una cuenta de stake delega a un validador cada vez; repartir entre varios significa varias cuentas de
stake. Delegar no es custodiar — el validador nunca gana la capacidad de mover tu stake, solo de votar con
su peso.

## Warmup, enfriamiento y épocas

Los cambios de delegación no surten efecto al instante. El stake **entra en warmup** para volverse activo
y **se enfría** antes de poder retirarse, y la red limita cuánto se puede activar o desactivar por época.
El retraso exacto depende por tanto de cuánto stake se está moviendo a la vez.

Una **época** son **432.000 slots**. Con la duración actual de slot de 200 ms eso son **24 horas**
nominales; los slots saltados hacen que el tiempo real varíe. Las fronteras de época son cuando entran en
vigor los pesos del stake, los calendarios de líderes y el reparto de recompensas.

Dos cifras prácticas:

- La **delegación mínima** es un valor del clúster; el ejemplo documentado de
  `getStakeMinimumDelegation` es **1 SOL**. Consulta el clúster en lugar de fijarlo en el código.
- La **comisión** es el porcentaje que se lleva el validador de las recompensas, fijado en la cuenta de
  voto. Se cobra sobre las recompensas, nunca sobre el principal.

## Quién produce cada bloque

El **calendario de líderes** se calcula antes de cada época. El algoritmo siembra una selección
pseudaleatoria a partir de una altura de tick de PoH, muestrea el conjunto de validadores activos y su
stake y elige líderes ponderados por stake. El calendario cubre 432.000 slots con un desfase de
anticipación de 8.192 slots expuesto por RPC.

Por eso la concentración del stake importa tanto operativamente como económicamente: más stake significa
más slots, y un validador bien operado con mucho stake produce una parte desproporcionada de los bloques.

## Recompensas, inflación y slashing

Las recompensas de una época se reparten entre el stake participante en proporción a
**stake × créditos de voto del validador**, menos la comisión. Los parámetros publicados del *calendario*
de inflación son 8 % de inflación anual inicial, una reducción anual del 15 % y una tasa a largo plazo del
1,5 % — describen la curva, no la tasa de hoy.

**Solana no aplica slashing hoy al stake delegado** por mal comportamiento de un validador. Un validador
que se cae o firma dos veces pierde recompensas y reputación, no el principal. Es una diferencia
significativa respecto a redes donde la equivocación quema stake, y merece decirse claro en cualquier
comparación: aquí la seguridad se apoya en la votación ponderada por stake y en que quien delega retire su
stake, no en una quema.

Confirmar un bloque requiere **dos tercios del stake activo**; la finalidad bajo Alpenglow se expresa
mediante los certificados de Votor (mira [la página de Alpenglow](/learn/alpenglow-internals)), donde la
vía rápida necesita el 80 % y las vías de respaldo usan umbrales del 60 %.

## Qué significa para una aplicación

- **La concentración de líderes es una variable de latencia.** Un slot cuyo líder está lejos de tus
  usuarios, o mal interconectado, es un pago más lento — invisible a 200 ms, visible bajo tensión.
- **El stake se mueve despacio.** El peso de un validador no se puede pedir prestado para un slot; el
  warmup y el enfriamiento encarecen los ataques por repetición tanto en tiempo como en capital.
- **Las recompensas no son rendimiento gratis.** Son inflación más comisiones, compartidas con un
  validador que se lleva una comisión. Quien cita una rentabilidad de staking sin nombrar la comisión y la
  fuente de inflación está contando media historia.

## Fuentes

- [Solana: staking](https://solana.com/docs/references/staking) · [stake accounts](https://solana.com/docs/references/staking/stake-accounts)
- [Anza: stake delegation and rewards](https://docs.anza.xyz/consensus/stake-delegation-and-rewards) · [leader rotation](https://docs.anza.xyz/consensus/leader-rotation)
- [getStakeMinimumDelegation](https://solana.com/docs/rpc/http/getstakeminimumdelegation) · [getEpochSchedule](https://solana.com/docs/rpc/http/getepochschedule)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
