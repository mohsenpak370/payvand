---
title: Qué es de verdad una cartera cripto
description: Una cartera no es un monedero. Es un par de claves que demuestra que el dinero de la red es tuyo — y por eso nadie puede restablecerla por ti.
category: basics
order: 3
---

Si solo has usado un banco, la palabra «cartera» engaña. Una cartera cripto no guarda tu dinero como un
monedero guarda billetes. El dinero vive en una red pública, como un saldo asociado a una dirección.
Lo que guarda la cartera es lo único que puede moverlo: la **clave privada**.

## Las tres partes que vas a encontrar

- **Tu dirección** — una cadena larga como `8D8GowHJyQRpPqEtAZy4H1DhjpbC…`. Es pública. Puedes
  compartirla en un mensaje, imprimirla en una factura o publicarla. La gente la usa para enviarte
  dinero.
- **Tu clave privada** — el secreto que firma los pagos de esa dirección. Quien la tenga controla el
  saldo, y si se filtra no hay a quién llamar.
- **Tu frase de recuperación** — doce o veinticuatro palabras normales que *son* tu clave privada,
  escritas en una forma que una persona puede respaldar. Más sobre esto en
  [Tu frase de recuperación es toda la cartera](/learn/protecting-your-seed-phrase).

Una app de cartera es un llavero con ventana: guarda las claves en tu dispositivo y te enseña lo que la
red dice que tiene tu dirección.

## Por qué nadie puede restablecer tu contraseña

Un banco guarda tu saldo en su propia base de datos, así que puede verificar tu identidad y
restablecer lo que olvides. Una cartera cripto no tiene esa base de datos ni una cuenta. La red solo
comprueba una cosa: **¿está este pago firmado por la clave dueña de esta dirección?** Sin firma no hay
pago — y ni un servicio de soporte, ni un gobierno, ni una empresa pueden producir esa firma por ti.

Ese intercambio es todo el sentido de la autocustodia: nadie puede congelar tu saldo, y nadie puede
deshacer un error por ti. Trata las claves como la escritura de una casa, no como una contraseña que se
puede restablecer.

## Qué significa en el día a día

- **Tu dinero no está «en la app».** Borra la app, instálala en un móvil nuevo, recupera la frase y
  aparece el mismo saldo. La app nunca fue la cartera.
- **Los pagos son definitivos.** No hay devolución ni botón de «deshacer la transferencia». Comprueba
  el importe y el destinatario antes de aprobar — mira
  [Lee antes de firmar](/learn/checking-before-you-sign).
- **La red es pública.** Cualquiera puede ver un saldo y el historial de una dirección. Nadie ve tu
  nombre a menos que lo asocies en algún sitio, así que conviene pensar en una dirección como un
  seudónimo, no como anonimato.
- **Hay muchas redes.** Una dirección de Solana no es la misma que una dirección de otra cadena.
  Enviar a la red equivocada suele significar que el dinero se ha ido, porque nadie puede devolverlo.

## Una forma segura de empezar

Casi todo el mundo aprende mejor con cantidades pequeñas. Un orden práctico:

1. Instala una sola cartera de confianza, y ninguna más, desde el enlace oficial de la tienda que
   aparece en la web del propio proyecto.
2. Escribe la frase de recuperación en papel. No la fotografíes.
3. Recibe una cantidad pequeña, envíate otra pequeña de vuelta y mira cómo es una comisión.
4. Solo entonces sube los importes, y solo cuando leas la pantalla de aprobación con soltura.

Si estás en la red de prueba de Payvand, los tokens están exactamente para eso: son gratis, no valen
nada y gastarlos te enseña los mismos gestos que usarás luego con dinero real —
[hay una guía corta sobre el dinero de práctica](/learn/test-money-on-devnet).

El siguiente paso es lo que pasa de verdad cuando pagas:
[qué ocurre cuando pagas con una cartera](/learn/paying-with-a-wallet).
