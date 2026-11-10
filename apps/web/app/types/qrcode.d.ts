/**
 * `qrcode` ships no types of its own. The app only ever renders an SVG from it
 * (see PaymentQr), so this declares that one call instead of pulling in a
 * whole typings package for options nothing here passes.
 */
declare module 'qrcode' {
  interface QrCodeSvgOptions {
    type: 'svg'
    /** Quiet zone, in modules. Four is what the specification asks for. */
    margin: number
    /** Rendered size in pixels; the modules are scaled to fit it. */
    width: number
    errorCorrectionLevel: 'L' | 'M' | 'Q' | 'H'
    color: { dark: string, light: string }
  }

  const QRCode: {
    toString: (text: string, options: QrCodeSvgOptions) => Promise<string>
  }

  export default QRCode
}
