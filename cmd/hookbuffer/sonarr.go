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
	Content string  `json:"content,omitempty"`
	Embeds  []Embed `json:"embeds,omitempty"`
}

// Embed structure for use inside Body
type Embed struct {
	Description string        `json:"description"`
	Title       string        `json:"title"`
	Color       int           `json:"color"`
	URL         string        `json:"url"`
	Author      author        `json:"author"`
	Timestamp   string        `json:"timestamp"`
	Fields      []embedfields `json:"fields,omitempty"`
}

type author struct {
	Name    string `json:"name"`
	IconURL string `json:"icon_url"`
}

type embedfields struct {
	Name   string `json:"name"`
	Value  string `json:"value"`
	Inline bool   `json:"inline"`
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
		println(err)
		return
	}

	// gets rid of the byte content in memory and saves a struct version
	h.BodyByte = nil
	h.Body = b

	matched, _ := regexp.MatchString(`^Test message from Sonarr.+`, b.Content)
	if matched {
		forwardTestHook(h)
		return
	}

	fmt.Println("[Recieved] " + b.Embeds[0].Title)

	// add the hook to the queue
	inQueue = append(inQueue, h)

	// trigger the timer
	startTimer()
}

func forwardTestHook(h *Hook) {
	w := webhook{
		url: "https://canary.discordapp.com" + h.URL,
		body: Body{
			Content: h.Body.Content,
		},
	}

	println("Forwarded test hook.")

	err := sendWebhook(&w)
	if err != nil {
		fmt.Println(err)
	}
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
	// build discord webhook with path
	webhookURL := "https://canary.discordapp.com" + g.hook.URL

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

	c := g.action + ": " + originalEmbed.Title

	replace := g.title + " - "
	shortTitle := g.episodes[0]
	shortTitle = strings.Replace(shortTitle, replace, "", 1)

	// build body
	body := Body{
		Content: c,
		Embeds: []Embed{
			{
				Description: shortTitle,
				Title:       g.title,
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

	re := regexp.MustCompile(`^(.+) - ([0-9]+)(x[0-9]+)+ - (.+)`)
	for _, h := range q {
		for _, e := range (*h).Body.Embeds {
			text := e.Title
			for _, f := range e.Fields {
				if f.Name == "Quality" {
					text += " [" + f.Value + "]"
					break
				}
			}

			action := "Unsupported Action"
			if strings.Contains(e.Description, "Grabbed") {
				action = "Grabbed"
			} else if strings.Contains(e.Description, "Imported") {
				action = "Imported"
			} else if strings.Contains(e.Description, "Upgraded") {
				action = "Upgraded"
			}

			segs := re.FindAllStringSubmatch(e.Title, -1)
			if segs == nil {
				println("[ERROR] TV Show/Season regex match failed.")
				continue
			}
			show, season := segs[0][1], segs[0][2]

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
