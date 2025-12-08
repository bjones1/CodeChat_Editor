/* c8 ignore next */
const { delayWorkerLoading } = window.graphvizWebComponent || {}
let renderer, rendererUrl

if (!delayWorkerLoading) setTimeout(getRenderer)

function ensureConfiguration () {
  if (!rendererUrl) {
    ({
      rendererUrl = 'https://unpkg.com/graphviz-webcomponent@2.0.0/dist/renderer.min.js'
    /* c8 ignore next */
    } = window.graphvizWebComponent || {})
  }
}

export default function getRenderer () {
  if (!renderer) {
    ensureConfiguration()
    renderer = new Worker(rendererUrl)
  }
  return renderer
}
