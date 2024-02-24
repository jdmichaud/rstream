const appTemplate = document.createElement('template');

appTemplate.innerHTML = `
  <style>
    .browser {
      width: 100%;
      height: 100%;
    }
  </style>
  <div class="browser"></div>
`;

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
    const app = this.shadowRoot.querySelector('.browser');
    while (app.lastElementChild) {
      app.removeChild(app.lastElementChild);
    }
    const children = Array.from(content.children[0].children)
      .reduce((acc, value) => { acc.push(...value.children); return acc }, []);
    // parseFromString returns a Document but we can only append element
    let scriptsToLoad = [];
    children.forEach(c => {
      if (c.tagName === 'SCRIPT') {
        scriptsToLoad.push(c);
      } else {
        app.appendChild(c)
      }
    });
    for (let script of scriptsToLoad) {
      var s = document.createElement("script");
      s.type = "text/javascript";
      s.src = script.src;
      document.body.appendChild(s);
    }
  }
}

customElements.define('browser-component', App);

document.addEventListener('DOMContentLoaded', () => console.log('DOMContentLoaded'));
