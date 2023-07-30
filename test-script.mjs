const baseEvent = {
  "eventType": "Download",
  "series": {
    "id": 1,
    "path": "/path/to/series",
    "tvdbId": 12345,
    "imdbId": "tt123456",
    "type": "standard"
  },
  "episodes": [{
    "id": 1234,
    "episodeNumber": 1,
    "seasonNumber": 2,
    "title": "Episode Title"
  }],
  "episodeFile": {
    "id": 4321,
    "dateAdded": new Date().toISOString(),
    "relativePath": "Season 02/Series Title - S02E01 - Episode Title.mkv",
    "path": "/path/to/series/Season 02/Series Title - S02E01 - Episode Title.mkv",
    "size": 1234567890,
    "quality": "HDTV-720p",
    "qualityVersion": 1
  },
  "downloadId": "Sabnzbd_nzo_abcdef",
  "isUpgrade": false
};

const grabEvents = eventLoop(5, 3, 10, (event, seriesId, seasonNumber, episodeNumber) => {
  // Modify the series title, ID, and path
  event.series.title = `Series ${seriesId}`;
  event.series.id = seriesId;
  event.series.path = `/path/to/series${seriesId}`;

  event.eventType = 'Grab';

  // Modify the season and episode numbers
  event.episodeFile.dateAdded = new Date().toISOString();
  event.episodes[0].seriesId = seriesId;
  event.episodes[0].seasonNumber = seasonNumber;
  event.episodes[0].episodeNumber = episodeNumber;

  // Modify the relative and absolute paths for the episode file
  event.episodeFile.relativePath = `Season ${seasonNumber.toString().padStart(2, '0')}/Series ${seriesId} - S${seasonNumber.toString().padStart(2, '0')}E${episodeNumber.toString().padStart(2, '0')} - Episode Title.mkv`;
  event.episodeFile.path = `/path/to/series${seriesId}/${event.episodeFile.relativePath}`;
  return event;
})

const downloadEvents = eventLoop(5, 3, 10, (event, seriesId, seasonNumber, episodeNumber) => {
  // Modify the series title, ID, and path
  event.series.title = `Series ${seriesId}`;
  event.series.id = seriesId;
  event.series.path = `/path/to/series${seriesId}`;

  event.eventType = 'Download';

  // Modify the season and episode numbers
  event.episodeFile.dateAdded = new Date().toISOString();
  event.episodes[0].seriesId = seriesId;
  event.episodes[0].seasonNumber = seasonNumber;
  event.episodes[0].episodeNumber = episodeNumber;

  // Modify the relative and absolute paths for the episode file
  event.episodeFile.relativePath = `Season ${seasonNumber.toString().padStart(2, '0')}/Series ${seriesId} - S${seasonNumber.toString().padStart(2, '0')}E${episodeNumber.toString().padStart(2, '0')} - Episode Title.mkv`;
  event.episodeFile.path = `/path/to/series${seriesId}/${event.episodeFile.relativePath}`;
  return event;
})


const upgradeEvents = eventLoop(5, 3, 10, (event, seriesId, seasonNumber, episodeNumber) => {
  // Modify the series title, ID, and path
  event.series.title = `Series ${seriesId}`;
  event.series.id = seriesId;
  event.series.path = `/path/to/series${seriesId}`;

  event.eventType = 'Download';
  event.isUpgrade = true;

  // Modify the season and episode numbers
  event.episodeFile.dateAdded = new Date().toISOString();
  event.episodes[0].seriesId = seriesId;
  event.episodes[0].seasonNumber = seasonNumber;
  event.episodes[0].episodeNumber = episodeNumber;

  // Modify the relative and absolute paths for the episode file
  event.episodeFile.relativePath = `Season ${seasonNumber.toString().padStart(2, '0')}/Series ${seriesId} - S${seasonNumber.toString().padStart(2, '0')}E${episodeNumber.toString().padStart(2, '0')} - Episode Title.mkv`;
  event.episodeFile.path = `/path/to/series${seriesId}/${event.episodeFile.relativePath}`;

  return event
})

function eventLoop(series_count = 5, season_count = 3, episode_count = 10, callback) {
  const events = []
  for (let seriesId = 1; seriesId <= series_count; seriesId++) {
    for (let seasonNumber = 1; seasonNumber <= season_count; seasonNumber++) {
      for (let episodeNumber = 1; episodeNumber <= episode_count; episodeNumber++) {
        // Create a new event based on the base event (deep copy)
        let event = JSON.parse(JSON.stringify(baseEvent));
        events.push(callback(event, seriesId, seasonNumber, episodeNumber));
      }
    }
  }
  return events
}

function discordToHookbuffer(url) {
  url = url.replace('https://discord.com', 'http://10.20.30.114:8000')
  return url
}

async function post(dest, event) {
  const res = await fetch(dest, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'User-Agent': 'Sonarr/3.0.2.4552',
    },
    body: JSON.stringify(event)
  });

  if (!res.ok) {
    throw new Error(`Error posting to ${dest}: ${res.statusText}`);
  }
}

// build some urls to our test channel
const url1 = discordToHookbuffer('https://discord.com/api/webhooks/1122863335')
const url2 = discordToHookbuffer('https://discord.com/api/webhooks/1135235117')

// put a bunch of events together
const events = grabEvents.concat(downloadEvents).concat(upgradeEvents)

for (let i = 0; i < events.length; i++) {
  const e = events[i];
  try {
    setTimeout(() => post(url1, e), i); // Adjusted timing
  } catch (e) {
    console.log(`Error at index ${i} for url1: ${e}`);
  }
  try {
    setTimeout(() => post(url2, e), i); // Adjusted timing
  } catch (e) {
    console.log(`Error at index ${i} for url2: ${e}`);
  }
}

