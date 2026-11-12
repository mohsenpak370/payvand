---
title: Alpenglow, desde dentro
description: Los certificados de Votor, la vía rápida del 80 %, el respaldo del 60 % y lo que «final en 150 ms» cuesta en supuestos. El cambio de consenso que sustituyó a TowerBFT.
category: technical
order: 4
level: technical
---

Alpenglow es el mayor cambio en el consenso de Solana desde su lanzamiento: sustituyó **TowerBFT** por un
mecanismo de finalización nuevo llamado **Votor**. Está activo en mainnet, activado al principio de la
**época 1064, el 21 de octubre de 2026**, después de que SIMD-0326 se aprobara con el 98,27 % del stake
participante en septiembre de 2025.

Su segunda fase, **Rotor**, que sustituye al protocolo de propagación de bloques Turbine, no está
desplegada ni programada. No las confundas — la página de la actualización es explícita sobre el orden.

## Qué hacía TowerBFT y por qué tenía que irse

TowerBFT alcanzaba la finalidad acumulando **lockouts**: cada voto a un bloque posterior aumentaba el
lockout de un validador, y la finalidad exigía unos **12,8 segundos** de confirmaciones acumuladas. Los
votos eran a su vez transacciones incluidas en bloques — el tráfico de consenso competía con los pagos
de los usuarios por el mismo espacio de bloque.

Eso es sobrevivible para liquidar. Es desesperante para una cola de café.

## Cómo finaliza Votor

Los validadores intercambian votos ahora **directamente**, fuera del flujo de bloques, y los agregan en
**certificados de quórum** — pruebas ponderadas por stake de que un umbral ha votado de una manera
concreta. SIMD-0326 describe cinco tipos de certificado:

| Certificado | Umbral | Función |
|---|---|---|
| Notarization | 60 % | Los validadores se ponen de acuerdo en un bloque |
| Skip | 60 % | Los validadores acuerdan saltarse un slot |
| Finalization | 60 % | Finalización en segunda ronda |
| Fast-Finalization | 80 % | Finalidad en una sola ronda |
| Notar-fallback | 60 % | Vía cuando la ronda rápida no se completa |

La vía feliz es por tanto: si el **80 % del stake** notariza un bloque en una sola ronda, existe un
certificado de Fast-Finalization y el bloque es final — sin segunda ronda y sin acumular lockouts.

Si eso no ocurre, el protocolo recurre a una vía más lenta de dos rondas con los umbrales del 60 %: un
certificado de notarización más el certificado correspondiente de segunda ronda. La finalidad llega
igual, solo con un viaje de ida y vuelta más.

## El modelo de fallos

Alpenglow está diseñado para **un 20 % de stake adversario más un 20 % de stake desconectado**,
combinados. No es la afirmación de que se tolere un 40 % de stake bizantino — la división 20+20 es el
supuesto, y es lo que codifica el umbral del 80 % de la vía rápida.

Eso importa al leer cifras de marketing. «El 80 % del stake» no es una supermayoría en el sentido clásico
de dos tercios; es el umbral que permite que una ronda baste *dado* el modelo de fallos.

## De dónde salen los 150 ms

El artículo de diseño de Anza cita una **mediana de unos 150 ms**, con casos alrededor de **100 ms**, a
partir de simulaciones con la distribución de stake de entonces y excluyendo explícitamente el coste de
cómputo. La página de la actualización enmarca unos 150 ms como expectativa, y el panel en vivo muestra
normalmente unos 200 ms.

No hay un máximo publicado. La afirmación ingenieril honesta es: un viaje de ida y vuelta entre
validadores, más transmisión, más varianza en la distribución de stake y en el ritmo de slots.

## Qué cambió más allá del número

- **Las transacciones de voto han desaparecido de los bloques**, así que el consenso ya no consume el
  espacio de bloque que necesitan los pagos.
- **Proof of History dejó de marcar el ritmo de la producción de bloques.** Los timeouts y certificados
  de Votor asumieron esa tarea, y las tick entries salieron del flujo de shreds.
- **`confirmed` y `finalized` convergieron.** Anza documenta ambos como confirmados de forma permanente;
  `processed` sigue siendo el estado débil y temprano. El código nuevo debería preferir `finalized`, y
  Solana dice que `confirmed` se eliminará en una versión posterior.
- **La ejecución no ha cambiado.** La SVM, los programas, el modelo de cuentas y el formato de
  transacción siguen igual. Una app de pagos no necesita reescribirse; necesita fiarse de otra etiqueta
  de commitment.

## Cómo comprobar un clúster

`getAgGenesisCert` informa de si un clúster ha migrado, y la génesis de Alpenglow para mainnet es el slot
**459.653.000**. En devnet, la activación fue la época **1167, el 25 de septiembre de 2026**. Para
cualquier cosa de cara al usuario, consulta el commitment en lugar de suponer una fecha.

## Qué significa para una app como esta

Un flujo de punto de venta puede tratar «finalized» como «entrega la mercancía», porque ya no queda ningún
periodo de lockout que esperar. El riesgo que no elimina es el humano: la finalidad convierte una
dirección equivocada en equivocada para siempre, más rápido que nunca. Por eso la pantalla de aprobación
sigue importando más que la insignia de confirmación.

## Fuentes

- [Solana: Alpenglow upgrade and status](https://solana.com/upgrades/alpenglow) · [dashboard](https://solana.com/upgrades/alpenglow/dashboard)
- [SIMD-0326: Alpenglow](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md)
- [Anza: Alpenglow, a new consensus for Solana](https://www.anza.xyz/blog/alpenglow-a-new-consensus-for-solana)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
