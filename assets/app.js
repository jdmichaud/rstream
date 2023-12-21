const appTemplate = document.createElement('template');

appTemplate.innerHTML = '<div id="app"></div>'

class App extends HTMLElement {
  constructor() {
    super();
  }

  connectedCallback() {
    this.route(window.location.href);
   
    const shadowRoot = this.attachShadow({ mode: 'open' });
    this.shadowRoot.appendChild(appTemplate.content);

    window.addEventListener('hashchange', event => {
      this.route(event.newURL);
    });
  }

  // https://bholmes.dev/blog/spas-clientside-routing/
  async route(pathname) {
    const route = `${window.location.origin}/assets/${pathname.split('#')[1]}.html`;
    // instead, we'll go fetch the resource ourselves
    const response = await fetch(route);
    // ...convert that response to something we can work with
    const htmlString = await response.text();
    this.displayContent(htmlString);
  }

  // Take a HTML partial and display it in the app element
  displayContent(htmlString) {
    const content = new DOMParser()
      .parseFromString(htmlString, 'text/html')
    // ...and do something with that content
    const app = this.shadowRoot.querySelector('#app');
    while (app.lastElementChild) {
      app.removeChild(app.lastElementChild);
    }
    // parseFromString returns a Document but we can only append element
    Array.from(content // document
      .children[0] // html
      .children
    ).forEach(c => app.appendChild(c));
  }
}

customElements.define('app-component', App);
