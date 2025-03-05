const baseEvent = {
    eventType: "Download",
    series: {
        id: 1,
        path: "/path/to/series",
        tvdbId: 12345,
        imdbId: "tt123456",
        type: "standard"
    },
    episodes: [
        {
            id: 1234,
            episodeNumber: 1,
            seasonNumber: 2,
            title: "Episode Title"
        }
    ],
    episodeFile: {
        id: 4321,
        dateAdded: new Date().toISOString(),
        relativePath: "Season 02/Series Title - S02E01 - Episode Title.mkv",
        path: "/path/to/series/Season 02/Series Title - S02E01 - Episode Title.mkv",
        size: 1234567890,
        quality: "HDTV-720p",
        qualityVersion: 1
    },
    downloadId: "Sabnzbd_nzo_abcdef",
    isUpgrade: false
};

function modifyEvent(
    event,
    seriesId,
    seasonNumber,
    episodeNumber,
    eventType = "Download",
    isUpgrade = false
) {
    event.series.title = `Series ${seriesId}`;
    event.series.id = seriesId;
    event.series.path = `/path/to/series${seriesId}`;

    event.eventType = eventType;
    event.isUpgrade = isUpgrade;

    event.episodeFile.dateAdded = new Date().toISOString();
    event.episodes[0].seriesId = seriesId;
    event.episodes[0].seasonNumber = seasonNumber;
    event.episodes[0].episodeNumber = episodeNumber;

    event.episodeFile.relativePath = `Season ${String(seasonNumber).padStart(
        2,
        "0"
    )}/Series ${seriesId} - S${String(seasonNumber).padStart(2, "0")}E${String(
        episodeNumber
    ).padStart(2, "0")} - Episode Title.mkv`;
    event.episodeFile.path = `/path/to/series${seriesId}/${event.episodeFile.relativePath}`;

    return event;
}

function eventLoop(
    series_count = 5,
    season_count = 3,
    episode_count = 10,
    callback
) {
    const events = [];
    for (let seriesId = 1; seriesId <= series_count; seriesId++) {
        for (let seasonNumber = 1; seasonNumber <= season_count; seasonNumber++) {
            for (let episodeNumber = 1; episodeNumber <= episode_count; episodeNumber++) {
                let event = JSON.parse(JSON.stringify(baseEvent));
                events.push(callback(event, seriesId, seasonNumber, episodeNumber));
            }
        }
    }
    return events;
}

// Create each event set by specifying the event type and upgrade flag
const grabEvents = eventLoop(5, 3, 10, (e, s, se, ep) =>
    modifyEvent(e, s, se, ep, "Grab")
);

const downloadEvents = eventLoop(5, 3, 10, (e, s, se, ep) =>
    modifyEvent(e, s, se, ep, "Download")
);

const upgradeEvents = eventLoop(5, 3, 10, (e, s, se, ep) =>
    modifyEvent(e, s, se, ep, "Download", true)
);

function discordToHookbuffer(url) {
    url = url.replace('https://discord.com', 'http://localhost:8000')
    return url
}

async function post(dest, event) {
    const res = await fetch(dest, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': 'Sonarr/3.0.2.4552',
            'Authorization': 'Basic ' + Buffer.from('admin:' + 'password', "utf-8").toString("base64")
        },
        body: JSON.stringify(event)
    });

    if (res.ok) {
        console.log(`Posted to ${dest}: ${res.statusText} ${await res.text()}`);
    } else {
        console.error(`Failed to post to ${dest}: ${res.statusText} ${await res.text()}`);
    }
}

// build some urls to our test channel
const url1 = discordToHookbuffer('https://discord.com/api/webhooks/1122863335')

function shuffleArray(array) {
    for (let i = array.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [array[i], array[j]] = [array[j], array[i]];
    }
    return array;
}

// put a bunch of events together
const events = grabEvents.concat(downloadEvents).concat(upgradeEvents)
const randomizedEvents = shuffleArray(events);

let limit = events.length;
// let limit = 1;
for (let i = 0; i < limit; i++) {
    const e = randomizedEvents[i];
    try {
        setTimeout(() => post(url1, e), i); // Adjusted timing
    } catch (e) {
        console.log(`Error at index ${i} for url1: ${e} `);
    }
}

