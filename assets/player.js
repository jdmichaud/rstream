const playerTemplate = document.createElement('template');

playerTemplate.innerHTML = `
<style>
a {
  color: inherit;
  text-decoration: inherit;
}
.player {
  display: flex;
}
</style>
<div class="player content-item">
  <div class="track-slider"></div>
  <div class="fast-reverse">FR</div>
  <div class="fast-play">P</div>
  <div class="fast-forward">FF</div>
</div>
`;

class Player extends HTMLElement {
  constructor() {
    super();
  }

  connectedCallback() {
    const pageName = this.getAttribute("data-page");
   
    const shadowRoot = this.attachShadow({ mode: 'closed' });
    shadowRoot.appendChild(playerTemplate.content);
  }
}

customElements.define('player-component', Player);
