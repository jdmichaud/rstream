
async function hookSearchInput() {
  const songList = await new Promise(resolve => {
    const intervalId = setInterval(() => {
      const songList = document.querySelector('browser-component').shadowRoot.querySelector('.song-list');
      if (songList !== undefined) {
        console.log('done');
        clearInterval(intervalId);
        resolve(songList);
      }
    }, 10);
  });

  const searchInput = document.querySelector('browser-component').shadowRoot
    .querySelector('.search')
    .querySelector('input');
  const searchObservable = new Observable.Subject();
  searchInput.addEventListener('input', event => searchObservable.next(event));
  searchObservable.subscribe(async event => {
    const value = event.target.value;
    if (value.length >= 3) {
      const results = await fetch(`/search?term=${value}`);
      if (((results.status / 100) | 0) === 2) { // Check this is a 2XX code
        const songs = await results.json();
        const songsElements = songs.map(song => {
          const songElement = document.createElement('div');
          songElement.classList.add('song');
          if (playingSong.get() == song.id) {
            songElement.classList.add('playing');
          }
          songElement.innerText = song.title;
          songElement.addEventListener('click', () => playingSong.next(song.id));
          playingSong.subscribe(songId => {
            if (songId == song.id) {
              songElement.classList.add('playing');
            } else {
              songElement.classList.remove('playing');
            }
          });
          return songElement;
        });
        songList.replaceChildren(...songsElements);
      }
    } else {
      songList.innerHTML = '';
    }
  });
}

async function hookClearButton() {
  const searchInput = document.querySelector('browser-component').shadowRoot.querySelector('.search').querySelector('input');
  const clearButton = document.querySelector('browser-component').shadowRoot.querySelector('.clear-button');
  clearButton.addEventListener('click', () => {
    searchInput.value = '';
    searchInput.dispatchEvent(new Event('input'));
  });
}

async function main() {
  hookSearchInput();
  hookClearButton();
}

if (window.songsScript === undefined || window.songsScript === false) {
  window.songsScript = true
  main();
}

