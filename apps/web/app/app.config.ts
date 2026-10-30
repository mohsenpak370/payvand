export default defineAppConfig({
  ui: {
    colors: {
      // Logo palette lives on the blue scale (see assets/css/main.css):
      // primary buttons, links and focus rings use the logo's cyan->blue.
      // 'green' is left as the Tailwind scale only `success` reads.
      primary: 'blue',
      neutral: 'slate'
    }
  }
})
