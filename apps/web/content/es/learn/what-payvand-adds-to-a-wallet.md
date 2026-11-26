---
title: Lo que Payvand añade a una cartera
description: Una cartera ya puede mostrar un código QR y enviar USDC. Lo que te deja a ti es comprobar: monedas falsas con nombres reales, solicitudes que piden más que un pago y saber a qué venta corresponde cada transferencia.
category: payvand
order: 0
---

Una cartera como Phantom o Solflare ya puede enviar USDC, EURC o SOL, y la mayoría puede mostrar un
código QR con un importe. Así que «hace códigos QR» no es el motivo para usar Payvand. Incluso el
dinero se mueve igual: directamente de la cartera del comprador a la del vendedor, nunca a través de
nosotros.

La diferencia está en todo lo que rodea a la transferencia. Una cartera mueve dinero y deja las
comprobaciones a las dos personas que la usan. Payvand hace esas comprobaciones por ellas. Esto es lo
que significa en la práctica.

## Solo cuenta la moneda real

Cualquiera puede crear una moneda nueva en unos minutos, llamarla «USDC» y ponerle el mismo logo. En
una cartera se ve exactamente igual que la real. No vale nada.

La única forma fiable de distinguirlas es la **dirección de mint** de la moneda: una cadena de 44
letras y números que la identifica, como un número de serie. El USDC real en Solana empieza por
`EPjFWdd5…`, y uno falso puede empezar por cualquier cosa. Para estar seguro de que te pagaron con
dinero de verdad, tendrías que abrir la transferencia y comparar esa cadena, carácter a carácter,
cada vez. Casi nadie lo hace, y por eso la estafa funciona.

Con Payvand nunca tienes que hacerlo. Las direcciones reales de USDC y EURC, tomadas de la lista que
publica la propia Circle, vienen integradas en Payvand, y la moneda forma parte de la dirección a la
que debe llegar el pago. Un pago con una moneda imitada va a otra dirección, una que este código no
vigila, así que la factura sigue **sin pagar**. Si pone Pagado, llegó la moneda real. Más sobre cómo
funcionan monedas como USDC en [las stablecoins, en palabras llanas](/learn/stablecoins-explained).

## El cliente aprueba un pago sencillo

Todo pago cripto termina con una pantalla en la cartera que pide al cliente que apruebe. Normalmente
no pasa nada. Pero una firma puede hacer más que enviar dinero. También puede dar a un programa un
**permiso permanente** para mover fondos más tarde, y así funcionan los «drainers»: una solicitud que
parece rutinaria deja que otra persona vacíe la cartera después.

Con una transferencia normal desde la cartera, además, el cliente tiene que escribir o pegar la
dirección y el importe, y un solo carácter equivocado manda el dinero a otro sitio para siempre.

Una solicitud de Payvand es siempre una sola cosa: **paga este importe, en esta moneda, a este
vendedor**. El importe, la moneda y el destinatario ya vienen rellenados desde el código, así que no
hay nada que escribir, pegar o leer mal, y no hay ningún permiso extra escondido. Lo que el cliente ve
en la cartera es lo que ocurre.

Dos límites, con honestidad:

- Payvand hace que su propia solicitud sea sencilla y clara. No puede impedir que un cliente firme
  algo dañino en otra web, así que el hábito de [lee antes de firmar](/learn/checking-before-you-sign)
  sigue importando, igual que las señales de [cómo detectar una estafa cripto](/learn/spotting-crypto-scams).
- El [presupuesto mensual](/learn/monthly-allowance-explained) opcional es un permiso, a propósito. Lo
  abre el propio cliente, con un límite que él fija; solo puede pagar facturas y se puede cerrar en
  cualquier momento.

## El precio y la moneda no pueden cambiar

El vendedor fija el precio, la moneda y una nota corta una sola vez, al crear el código. El importe
forma parte de la propia dirección de pago, así que un cliente no puede pagar menos y una página no
puede cambiar el precio después de escanear el código. Un importe distinto iría a una dirección que
este código no vigila. [Qué pasa cuando pagas con una cartera](/learn/paying-with-a-wallet) lo
explica paso a paso.

## El vendedor sabe quién pagó y por qué

Una cartera muestra el dinero que entra como una lista de importes desde direcciones de
desconocidos. Emparejarlos con ventas se hace a ojo: ¿qué 4,50 era el café y cuál el cruasán?

Un código Payvand vigila su propio pago. Pasa solo a **Pagado** en cuanto llega el dinero, en la
pantalla del vendedor y en la del cliente, y guarda la dirección del pagador y la transacción como
recibo. No hay que fiarse de una captura de «mira, ya lo envié». Una captura se puede falsificar; un
código que ha pasado a Pagado, no.

## El cliente no necesita SOL

Enviar USDC desde una cartera requiere un poco de SOL para la comisión de red. Un cliente con mucho
USDC y nada de SOL no puede pagar, algo que sorprende a casi todos la primera vez. Con USDC o EURC en
Payvand, nuestro relayer paga la comisión de red y se le devuelve con la comisión incluida en el
precio, así que el cliente solo necesita la moneda con la que paga.
[Lo que cuesta un pago](/learn/what-a-payment-costs) muestra adónde va cada parte.

## Y sigue siendo tu cartera

Nada de esto requiere que Payvand guarde nada. No hay saldo en Payvand ni frase semilla que
conservar, y la app no tiene ninguna clave que pueda mover tus fondos. Cada pago va directo a la
cartera del propio vendedor. Payvand es la capa sobre la cartera en la que ya confías: añade las
comprobaciones y la contabilidad, y el dinero se queda donde estaba.
