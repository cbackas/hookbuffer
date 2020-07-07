# Hookbuffer

Essentially a webhook proxy server. 

Takes in webhooks that originate from Sonarr and are intended for Discord. It catches "Grabbed" and "imported" notifications from Sonarr, uses some timers to add a delay in which it can catch and group together many notifications by show and season, then pass those groupings along to the intended Discord webhook URL. Sonarr should probably have this built in.... but until then I made this. I don't really expect anyone else to use this, this is my first (and last for a while) Go project and helps solve an annoyance. 

Main benefits:
- Use Sonarr notifications without as much spam
- IMPORT SPEED
    - When Sonarr is configured with a Discord connection for notifications, imports will slow down to the speed of Discord's rate limit (1 webhook per 1-2 seconds). This proxy server catches the webhooks from Sonarr significantly faster, and then groups them up and ends up sending significantly less requests to Discord, 1 second apart.

#### Example:

![Example](https://i.imgur.com/GlZTAZc.png)

Instead of 9 separate Discord messages (one per episode), Hookbuffer groups them by season and sends them to Discord. 

### Usage: 

It's on Dockerhub: https://hub.docker.com/r/cbackas/hookbuffer

Or you can build it and run it using commands

```go build cmd/hookbuffer```

```./hookbuffer```

Then in Sonarr, go to "Connections" and set up a Discord connection, get a webhook URL from your Discord client, paste that URL and replace https://discordapp.com/ with http://\<host ip\>:5369/ (retaining /api/webhooks/ and everything after it) (Example: https://i.imgur.com/RvZUMOk.png)

Currently has no support for passing through the username or avatar fields, as I have those configured directly on my webhooks in Discord.
