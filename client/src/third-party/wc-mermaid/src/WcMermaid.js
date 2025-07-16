/**
 * WcMermaid
 * @class
 */
export class WcMermaid extends HTMLElement {
  // A promise created by the dynamic import of Mermaid, which resolves to the Mermaid API.
  #mermaidApiPromise

  constructor() {
    super();

    this.attachShadow({ mode: 'open' });
    this.__renderGraph = this.__renderGraph.bind(this);

    this.#mermaidApiPromise = new Promise(async (resolve) => {
      const mermaidApiModule = await import('mermaid/dist/mermaid.core.mjs');
      // We now have access to the dynamically-loaded Mermaid module. Store its API for use by the renderer.
      const mermaidApi = mermaidApiModule.default;
      mermaidApi.initialize({
        logLevel: 'none',
        startOnLoad: false,
      });
      resolve(mermaidApi);
    });
  }

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

  __renderGraph() {
    /** @type {Promise<void>} */
    this.updated = (async () => {
      if (this.__textContent !== '') {
        if (this.shadowRoot) {
          // Delay rendering until the Mermaid API is loaded.
          const mermaidApi = await this.#mermaidApiPromise;
          // Create the element that will contain the rendered graph.
          this.shadowRoot.innerHTML = '';
          const div = document.createElement('div');
          div.id = "graph";
          this.shadowRoot.appendChild(div);
          try {
            const renderResult = await mermaidApi.render(
              'graph',
              this.__textContent
            );
            div.innerHTML = renderResult.svg;
          } catch (e) {
            div.textContent = e.toString();
            div.innerHTML = `<pre style="color:red; white-space: pre-wrap;">${div.innerHTML}</pre>`
          }
        }
      } else {
        if (this.shadowRoot) {
          this.shadowRoot.innerHTML = '';
        }
      }
    })();
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
