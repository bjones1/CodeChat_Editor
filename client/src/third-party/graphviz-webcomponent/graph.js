import getRenderer from './separate-engine'

const scaleKey = Symbol('scale')
// Use this to assign a unique ID to each render. Messages with the rendered
// SVG will be received by multiple graphs, if multiple graphs dispatched
// a request for rendering.
let renderId = 1

function requestRendering (element, script, receiveResult) {
  const renderer = getRenderer()
  renderer.addEventListener('message', receiveResult)
  renderer.postMessage({ script: script || element.graph, renderId })
  return renderId++
}

function closeRendering (receiveResult) {
  const renderer = getRenderer()
  renderer.removeEventListener('message', receiveResult)
}

function triggerEvent (element, type, detail) {
  element.dispatchEvent(new CustomEvent(type, { detail }))
}

function applyScale (element) {
  const svg = element.shadowRoot.children[0]
  const scale = element.scale
  if (svg) {
    if (scale) {
      svg.style.transform = `scale(${scale})`
      svg.style.transformOrigin = 'top left'
    } else {
      svg.style.transform = ''
      svg.style.transformOrigin = ''
    }
  }
}

function showImage (element, svg) {
  element.shadowRoot.innerHTML = svg
  applyScale(element)
  triggerEvent(element, 'render', svg)
}

function showError (element, error) {
  console.error('Graphviz failed:', error)
  element.shadowRoot.innerHTML = error.message
  return triggerEvent(element, 'error', error)
}

function updateGraph (element) {
  return new Promise(resolve => {
    element.shadowRoot.innerHTML = ''
    const script = element.__textContent;
    if (!script) return resolve()
    const assignedRenderId = requestRendering(element, script, receiveResult)

    function receiveResult ({ data }) {
      const { svg, error, renderId } = data
      // This render was for a different request. Ignore it.
      if (assignedRenderId !== renderId) return
      closeRendering(receiveResult)
      if (error) {
        error.message = error.message.trim()
        showError(element, error)
        return resolve(error)
      }
      showImage(element, svg)
      resolve(svg)
    }
  })
}

function tryUpdateGraph (element, script) {
  return new Promise((resolve, reject) => {
    if (!script) {
      element.innerHTML = ''
      element.shadowRoot.innerHTML = ''
      return resolve()
    }
    const assignedRenderId = requestRendering(element, script, receiveResult)

    function receiveResult ({ data }) {
      const { svg, error, renderId } = data
      // This render was for a different request. Ignore it.
      if (assignedRenderId !== renderId) return
      closeRendering(receiveResult)
      if (error) return reject(error)
      element.innerHTML = script
      showImage(element, svg)
      resolve(svg)
    }
  })
}

class GraphvizGraphElement extends HTMLElement {
  constructor () {
    super()
    this.attachShadow({ mode: 'open' })
    this.graphCompleted = Promise.resolve()
    // From Mermaid web component -- see below.
    this.__renderGraph = this.__renderGraph.bind(this);
  }

  get scale () { return this[scaleKey] }
  set scale (value) { this.setAttribute('scale', value) }

  attributeChangedCallback (name, oldValue, newValue) {
    switch (name) {
      case 'scale':
        this[scaleKey] = newValue
        applyScale(this)
    }
  }

  tryGraph (graph) {
    const promise = tryUpdateGraph(this, graph)
    this.graphCompleted = promise.catch(error => error)
    return promise
  }

  static get observedAttributes () { return ['scale'] }

  __renderGraph() {
    this.graphCompleted = updateGraph(this).catch(error => error)
  }

  // Copied from https://github.com/manolakis/wc-mermaid/blob/master/src/WcMermaid.js:
  /**
   * @returns {ChildNode[]}
   * @private
   */
  get __textNodes() {
    return Array.from(this.childNodes).filter(
      node => node.nodeType === this.TEXT_NODE
    );
  }

  /**
   * @returns {string}
   * @private
   */
  get __textContent() {
    return this.__textNodes.map(node => node.textContent?.trim()).join('');
  }

    __observeTextNodes() {
    if (this.__textNodeObservers) {
      this.__cleanTextNodeObservers();
    }

    this.__textNodeObservers = this.__textNodes.map(textNode => {
      const observer = new MutationObserver(this.__renderGraph);

      observer.observe(textNode, { characterData: true });

      return observer;
    });
  }

  __cleanTextNodeObservers() {
    if (this.__textNodeObservers) {
      this.__textNodeObservers.forEach(observer => observer.disconnect());
    }
  }

  connectedCallback() {
    this.__observer = new MutationObserver(() => {
      this.__observeTextNodes();
      this.__renderGraph();
    });
    this.__observer.observe(this, { childList: true });
    this.__observeTextNodes();
    this.__renderGraph();
  }

  disconnectedCallback() {
    this.__cleanTextNodeObservers();

    if (this.__observer) {
      this.__observer.disconnect();
      this.__observer = null;
    }
  }
}

customElements.define('graphviz-graph', GraphvizGraphElement)

export default GraphvizGraphElement
