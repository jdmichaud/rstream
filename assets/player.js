const playerTemplate = document.createElement('template');

playerTemplate.innerHTML = `
<style>
a {
  color: inherit;
  text-decoration: inherit;
}
.player {
  display: flex;
  justify-content: space-around;
}
.track-slider {
  flex-grow: 5;
}
.player-button {
  font-size: 50px;
  border: 2px lightgray;
  border-radius: 50%;
  flex-grow: 1;
  background: #777;
  aspect-ratio : 1 / 1;
  margin: 20px;
  text-align: center;
  cursor: grab;
}
.audio {
  display: none;
}
</style>
<div class="player content-item">
  <div class="track-slider"></div>
  <div class="player-button fast-reverse">⏮</div>
  <div class="player-button play">▶</div>
  <div class="player-button fast-forward">⏭</div>
  <audio class="audio" autoplay>
</div>
`;

class Player extends HTMLElement {
  constructor() {
    super();
  }

  connectedCallback() {
    const pageName = this.getAttribute("data-page");
    const shadowRoot = this.attachShadow({ mode: 'open' });
    shadowRoot.appendChild(playerTemplate.content);
    this.audio = this.shadowRoot.querySelector('.audio');
    // Global for controlling which song is playing
    window.playingSong = new Observable.BehaviorSubject('');
    playingSong.subscribe(songId => this.play(songId));
    // Some local observable of the player state
    this.playing = new Observable.Subject(false);
    this.timeupdate = new Observable.Subject();
    this.volume = new Observable.Subject();
    this.audio.addEventListener('playing', event => this.playing.next(!event.target.paused));
    this.audio.addEventListener('pause', event => this.playing.next(!event.target.paused));
    this.audio.addEventListener('timeupdate', event => this.timeupdate.next(event.target.currentTime));
    this.audio.addEventListener('volumechange', event => this.volumechange.next(event.target.volume));
    // Setup buttons
    this.playButton = this.shadowRoot.querySelector('.play');
    this.playButton.addEventListener('click', () => {
      this.audio.paused ? this.audio.play() : this.audio.pause();
    });
    this.playing.subscribe(playing => {
      this.playButton.innerHTML = playing ? '⏸' : '▶';
    });
  }

  async play(id) {
    if (id === '') return;
    this.shadowRoot.querySelector('.audio').src = `/song/${id}`;
    const songData = await (await fetch(`/songs/${id}`)).json();
    console.log(songData);
  }
}

customElements.define('player-component', Player);
