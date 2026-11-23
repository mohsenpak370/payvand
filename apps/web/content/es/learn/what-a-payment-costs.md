---
title: Lo que cuesta un pago, y por qué
description: A dónde va el dinero cuando alguien paga con Payvand: la comisión de red, la del protocolo, el recibo permanente en la cadena y los gastos que mantienen el servicio en marcha.
category: payvand
order: 3
---

Nada de esto es gratis de mantener, y casi nada de lo que cuesta un pago es beneficio. Esta página
explica cada coste y quién lo paga, para que la cifra de tu pantalla nunca sea una sorpresa.

## Lo que paga el cliente

Exactamente el precio del código, más la comisión de red de Solana por la transacción: fracciones de
céntimo, pagadas a la red y no a nosotros. No hay recargo, ni cargo por servicio, ni comisión de
gestión. Si un código dice 4 EURC, la billetera muestra 4 EURC.

## Lo que paga el vendedor

Dos cosas, ambas descontadas del importe en lugar de añadidas, y ambas visibles antes de compartir un
código:

1. **La comisión del protocolo**, un porcentaje del precio. Es marginal en lugar de plana: un tipo se
   aplica a la primera parte del precio y uno menor a todo lo que supera un umbral, así una factura
   grande nunca cuesta más por unidad que una pequeña. El umbral cambia entre SOL y stablecoins,
   porque no es el mismo tipo de dinero. Además hay un tope: un pago nunca paga más de 0,1 SOL,
   10 USDC o 10 EURC de comisión, así una factura muy grande paga una comisión con forma de tipo, no
   una parte de sí misma que no deja de crecer.
2. **La comisión de registro**, en pagos hechos con un token. Cada pago liquidado deja un recibo
   pequeño y permanente en la cadena. El espacio en la cadena se paga por adelantado en SOL y no se
   recupera nunca. La comisión de registro cubre ese coste, y se calcula con el tipo de cambio de SOL
   en vivo, así que sigue lo que cuesta el espacio hoy y no lo que valía SOL cuando se escribió la
   regla. Compra una cantidad fija de espacio, así que pesa más en una factura pequeña. Una factura en
   SOL no la lleva aparte, porque el vendedor ya entrega esa renta del precio.

Cuando un pago es tan pequeño que el porcentaje sale menor que ese registro, el porcentaje no se
cobra: el vendedor paga el registro y nada más. El protocolo no cobra comisión en ese pago en lugar de
una comisión que no cubriría su propio recibo.

La pantalla del vendedor muestra el importe exacto que recibirá y el desglose de la comisión en esas
dos partes, antes de que un cliente vea el código.

## Lo que nos cuesta mantener esto

Un pago va de billetera a billetera. Nunca retenemos el dinero, así que no tenemos saldo del que
ganar intereses. Lo que sí tenemos es una factura:

- **Servidores**: la web y la API que construye códigos, lee el estado de los pagos y sirve las tiendas.
- **Una base de datos**: cuentas, tiendas, productos, códigos guardados y suscripciones.
- **Acceso a la red**: el servicio RPC del que la app lee saldos y estados, una infraestructura de pago
  a cualquier volumen real.
- **Información de precios**: tipos de cambio, para que un precio en euros se pueda pagar en SOL o en
  dólares a un tipo justo.
- **Correo**: los pocos mensajes que una cuenta necesita: avisos de acceso y restablecer contraseña.
- **Monitorización, copias de seguridad y trabajo de seguridad**: la mitad menos vistosa de cualquier
  servicio que toca dinero.
- **Personas**: construir, arreglar y responder preguntas.

La suscripción de tiendas y la comisión del protocolo cubren parte de eso. La comisión no está puesta
para sacar el máximo: está puesta para mantener el servicio vivo mientras un pago sigue siendo más
barato que un terminal de tarjeta.

## Por qué la comisión tiene dos partes

Un porcentaje solo no cubre un coste fijo. El recibo en la cadena cuesta lo mismo si el pago es de una
unidad o de mil, así que una comisión solo porcentual sería demasiado pequeña en pagos pequeños y un
recorte innecesariamente grande en los grandes. Dividirla — un porcentaje por el trabajo, una comisión
fija por el espacio, y el registro como suelo de ambas — mantiene posibles los pagos pequeños y
baratos los grandes.

## Qué puede cambiar

Los tipos y la comisión de registro los fija la plataforma y siguen el coste de operar el servicio; la
comisión de registro se recalcula con el tipo de cambio de SOL en vivo, así que una subida de SOL se
sigue en vez de absorberse.
El precio de un código nunca cambia, pero la comisión es la vigente al pagar, así que un cambio de
tipos afecta también a códigos ya compartidos y aún sin pagar. La app muestra el desglose con los
tipos vigentes.
