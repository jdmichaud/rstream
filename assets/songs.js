
function main() {
  const songList = document.getElementsByClassName('song-list')[0];
  const searchInput = document.getElementsByClassName('search')[0];
  const searchObservable = new Observable.Subject();
  searchInput.addEventListener('input', event => searchObservable.next(event));
  searchObservable.subscribe(async event => {
    const value = event.target.value;
    if (value.length >= 3) {
      const results = await fetch(`/search?term=${value}`);
      if (((results.status / 100) | 0) === 2) { // Check this is a 2XX code
        const songs = await results.json();
        const songsElement = songs.map(song => {
          const songElement = document.createElement('div');
          songElement.classList.add('song');
          songElement.innerText = song.title;
          return songElement;
        });
        songList.replaceChildren(...songsElement);
      }
    }
  });
}

document.addEventListener('DOMContentLoaded', function() {
  main();
});

