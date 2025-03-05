# Hookbuffer
Essentially a buffering webhook proxy server.

Takes in webhooks that originate from Sonarr and are intended for Discord. It catches grab, import, and upgrade event notifications from Sonarr, uses some timers to add a delay in which it can catch and group together many notifications by show and season, then pass those groupings along to the intended Discord webhook URL. Sonarr should probably have this built in.... but until then I made this.

Hookbuffer keeps separate queues of messages for each Discord URL it recieves notifications for. This allows 1 Hookbuffer instance to be used for multiple Sonarr instances which go to different destinations without messages getting crossed, which is why I have Hookbuffer deployed to the cloud.

Now with Cloudflare Workers support! This is kinda pointless but this serverless version of Hookbuffer makes use of Durable Objects and Queues to keep track of message timing, grouping, and reliable delivery. The standalone version of the project is still available.

Main benefits:
- Use Sonarr notifications without as much spam
- IMPORT SPEED
    - When Sonarr is configured with a Discord connection for notifications, imports will slow down to the speed of Discord's rate limit (1 webhook per 1-2 seconds). This proxy server catches the webhooks from Sonarr significantly faster, and then groups them up and ends up sending significantly less requests to Discord, 1 second apart.

#### Example:

<img src="/assets/example_results.png" width="300">

Instead of 60+ separate Discord messages (one per episode), Hookbuffer groups them by season and sends them to Discord.

### Deploying App
#### How to host:
- You can build and run the binary for `hookbuffer-standalone` wherever you want
- You can run the Docker container for `hookbuffer-standalone`
- You can use the cloudflare workers version, `hookbuffer-cf-worker`

#### Docker:
Docker image can be found on Github Container Registry: https://github.com/cbackas/hookbuffer/pkgs/container/hookbuffer

`docker run --name hookbuffer -p 8000:8000 ghcr.io/cbackas/hookbuffer:latest`

#### Cloudflare Workers:
Deploy worker: `npx wrangler deploy`

When running this you might see a warning regarding durable objects and not having migrations. Add this block to your wrangler.toml file to provide an initial migration for the durable object:
```toml
[[migrations]]
tag = "v1"
new_classes = [ "ChannelQueue" ]
```

### Authentication:

#### Docker:
If you deploy this container on the same local network as your Sonarr instance, you don't really need authentication. But if you deploy it to tyhe cloud, you should enable hookbuffer's auth feature. You can add authentication checks by setting the `HOOKBUFFER_USER` and `HOOKBUFFER_PASS` environment variables, ex:

`docker run --name hookbuffer -p 8000:8000 -e HOOKBUFFER_USER=user -e HOOKBUFFER_PASS=pass ghcr.io/cbackas/hookbuffer:latest`

#### Cloudflare Workers:
Similar to the Docker auth, you can require basic auth for all Hookbuffer requests when the Workers version. If you populate the `SECET_KEY` secret on your Worker environment, then all requests will require basic auth with a username matching `admin` and password matching the value of `SECRET_KEY`. To set the secret on the environment, run `npx wrangler secret put SECRET_KEY`

### Other Env vars:
- `HOOKBUFFER_PORT` - Port to listen on inside container (default 8000)
- `HOOKBUFFER_DESTINATION_URL` - The URL used to send the grouped webhooks to. Defaults to `https://discordapp.com/`

### Configuring Sonarr
First you need to create a Discord webhook in the Discord channel you want to send notifications to. You can do this by going to the channel settings, then "Integrations" and "Webhooks". Create a webhook and copy the URL. More here: https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks

Next we need to configure Sonarr to send notifications through Hookbuffer.
1. Go to Sonarr -> Settings -> Connect
2. Create new connection, choose "Webhook" as the connection type
3. Give the connection a name and *enable the "On Grab", "On Import", and "On Upgrade" triggers*
4. Configure Webhook URL
	a. Paste your Discord webhook URL into the "Webhook URL" field
    b. Replace the "https://discordapp.com/" part of the URL with 'http://\<hookbuffer_host_ip\>:\<hookbuffer_port\>' (example: https://discordapp.com/api/webhooks/12345678910/abcdefghijklmnopqrstuvwxyz -> http://192.168.0.30:8000/api/webhooks/12345678910/abcdefghijklmnopqrstuvwxyz)
5. Optional: If you configured the `HOOKBUFFER_USER` and `HOOKBUFFER_PASS` env variables, then you can put those values into the "Username" and "Password" fields
6. Test and Save the connection

![Sonarr Config Example](/assets/example_sonarr_config.png)

### Build it yourself:
#### Standalone:
Build Executable: `cargo build -p hookbuffer-standalone --release`

Build and run: `cargo run -p hookbuffer-standalone`

#### Worker:
Build WASM Worker: `npx wrangler deploy --dry-run`

Build and run: `npx wrangler dev`
