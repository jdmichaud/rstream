const navbarTemplate = document.createElement('template');

navbarTemplate.innerHTML = `
<style>
a {
  color: inherit;
  text-decoration: inherit;
}
.navbar {
  flex-grow: 2;
  max-width: 3cm;
}
.navbar-item {
  margin-top: 0.5mm;
  margin-bottom: 0.5mm;
}
.navbar-item.selected {
  font-weight: bold;
}
</style>
<div class="navbar">
  <a href="app.html#songs"><div id="songs" class="navbar-item">Songs</div></a>
  <a href="app.html#artists"><div id="artists" class="navbar-item">Artists</div></a>
  <a href="app.html#albums"><div id="albums" class="navbar-item">Albums</div></a>
</div>
`;

class Navbar extends HTMLElement {
  constructor() {
    super();
  }

  connectedCallback() {
    const shadowRoot = this.attachShadow({ mode: 'open' });
    shadowRoot.appendChild(navbarTemplate.content);
    // Set the selected item
    this.setSelected(window.location.hash.split('#')[1]);
    window.addEventListener('hashchange', event => {
      this.setSelected(window.location.hash.split('#')[1]);
    });
  }

  setSelected(category) {
    if (category !== '' && category !== null && category !== undefined) {
      for (let entry of this.shadowRoot.querySelectorAll('.navbar-item')) {
        entry.classList.remove('selected');
      }
      const div = this.shadowRoot.getElementById(category);
      div.classList.add('selected');
    }
  }
}

customElements.define('navbar-component', Navbar);
