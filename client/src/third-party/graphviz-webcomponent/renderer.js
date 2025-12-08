import { Graphviz } from '@hpcc-js/wasm/graphviz'

let graphviz
let fatalError

async function receiveRequest ({ data }) {
  const { script, renderId } = data
  if (script === undefined) return // prevent endless message loop in tests
  /* c8 ignore next */
  if (fatalError) return postMessage({ error: fatalError, renderId })
  try {
    if (!graphviz) graphviz = await Graphviz.load()
    const svg = graphviz.dot(script)
    postMessage({ svg, renderId })
  } catch ({ message }) {
    postMessage({ error: { message }, renderId })
  }
}

/* c8 ignore next 7 */
function handleRejection (event) {
  event.preventDefault()
  const { message } = event.reason
  const error = { message: `Graphviz failed. ${message}` }
  if (message.includes('fetching of the wasm failed')) fatalError = error
  postMessage({ error })
}

addEventListener('message', receiveRequest)
addEventListener('unhandledrejection', handleRejection)
