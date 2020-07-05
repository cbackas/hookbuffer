package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"regexp"
	"sort"
	"strings"
	"time"
)

var inQueue []*Hook  // hooks we recieve
var outQueue []*Hook // hooks we're sending

var timerActive bool = false         // is a hook timer currenty running
var timerDefault int = 10            // timer default
var timeRemaining int = timerDefault // timer that counts down

// Body structure of a Discord webhook
type Body struct {
	Content string  `json:"content"`
	Embeds  []Embed `json:"embeds"`
}

// Embed structure for use inside Body
type Embed struct {
	Description string `json:"description"`
	Title       string `json:"title"`
	Text        string `json:"text"`
	Color       int    `json:"color"`
}

type webhook struct {
	url  string
	body Body
}

type group struct {
	action   string
	title    string
	season   string
	episodes []string
	hook     Hook // one of the original hooks (to get URL)
}

// HandleSonarr recieves the hook from webhookhandler, processes content, and manages its own timer/queue
func HandleSonarr(h *Hook) {
	var b Body
	err := json.Unmarshal(h.BodyByte, &b)
	if err != nil {
		return
	}

	// gets rid of the byte content in memory and saves a struct version
	h.BodyByte = nil
	h.Body = b

	// add the hook to the queue
	inQueue = append(inQueue, h)

	// trigger the timer
	fmt.Println("[Recieved] " + h.Body.Content)
	startTimer()
}

func startTimer() {
	// if timer already active reset it
	if timerActive {
		timeRemaining = timerDefault
		return
	}

	timerActive = true
	for range time.Tick(1 * time.Second) {
		if timeRemaining <= 0 {
			timerActive = false
			go processQueue(inQueue)
			break
		}
		timeRemaining--
	}
	timeRemaining = timerDefault
}

func processQueue(q []*Hook) {
	// reset overall queue
	// we just need the q we recieved by value
	resetQueue()

	mergedQueue := mergeQueue(q)

	var keys []string
	for k := range mergedQueue {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	for _, k := range keys {
		webhook := prepareWebhook(mergedQueue[k])
		err := sendWebhook(&webhook)
		if err != nil {
			fmt.Println(err)
		}
		time.Sleep(1 * time.Second)
	}
}

func prepareWebhook(g group) webhook {
	// parse URL path out of original request
	re := regexp.MustCompile(`^/api/webhooks/(.+)/(.+)`)
	segs := re.FindAllStringSubmatch(g.hook.URL, -1)
	seg1, seg2 := segs[0][1], segs[0][2]

	// build discord webhook with path
	webhookURL := "https://canary.discordapp.com/api/webhooks/" + seg1 + "/" + seg2

	webhook := webhook{url: webhookURL}
	if len(g.episodes) == 1 {
		h := prepareSingleHook(&g)
		webhook.body = h.body
	} else {
		h := prepareGroupHook(&g)
		webhook.body = h.body
	}
	return webhook
}

func prepareGroupHook(g *group) webhook {
	originalEmbed := g.hook.Body.Embeds[0]

	// make a string of the list of episodes
	var d string
	for _, e := range g.episodes {
		r := g.title + " - "
		e = strings.Replace(e, r, "", 1)
		d += e + "\n"
	}

	c := g.action + ": " + g.title + " Season " + g.season

	// build body
	body := Body{
		Content: c,
		Embeds: []Embed{
			{
				Description: d,
				Title:       originalEmbed.Title,
				Text:        d,
				Color:       originalEmbed.Color,
			},
		},
	}

	return webhook{
		body: body,
	}
}

func prepareSingleHook(g *group) webhook {
	originalEmbed := g.hook.Body.Embeds[0]

	c := g.action + ": " + originalEmbed.Text

	// build body
	body := Body{
		Content: c,
		Embeds: []Embed{
			{
				Description: originalEmbed.Description,
				Title:       originalEmbed.Title,
				Text:        originalEmbed.Text,
				Color:       originalEmbed.Color,
			},
		},
	}

	return webhook{
		body: body,
	}
}

// SendWebhook POSTs to webhookURL
// body needs to be Marshalled into body []byte
func sendWebhook(webhook *webhook) error {
	b, _ := json.Marshal(webhook.body)
	req, reqErr := http.NewRequest(http.MethodPost, webhook.url, bytes.NewBuffer(b))
	if reqErr != nil {
		return reqErr
	}

	req.Header.Add("Content-Type", "application/json")

	client := &http.Client{Timeout: 10 * time.Second}
	_, doErr := client.Do(req)
	if doErr != nil {
		return doErr
	}

	fmt.Println("[POST] Posted " + webhook.body.Content)
	return nil
}

// merges queue items into season and show groupings
func mergeQueue(q []*Hook) map[string]group {

	groups := make(map[string]group)

	re := regexp.MustCompile(`^(Grabbed|Imported): (.+) - ([0-9]+)x([0-9]+) - (.+) (\[.+\])`)
	for _, h := range q {
		content := (*h).Body.Content
		text := (*h).Body.Embeds[0].Text
		segs := re.FindAllStringSubmatch(content, -1)
		action, show, season := segs[0][1], segs[0][2], segs[0][3]

		groupName := action + "-" + show + "-" + season
		if groups[groupName].title == "" { // action-show NOT present in groups
			groups[groupName] = group{
				action:   action,
				title:    show,
				season:   season,
				episodes: []string{text},
				hook:     *h,
			}
		} else { // action-show already present in groups
			g := groups[groupName]
			g.episodes = append(g.episodes, text)

			groups[groupName] = g
		}
	}
	return groups
}

func contains(arr []string, str string) bool {
	for _, a := range arr {
		if a == str {
			return true
		}
	}
	return false
}

func resetQueue() {
	inQueue = []*Hook{}
}
