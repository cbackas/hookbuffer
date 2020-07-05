package main

import (
	"strings"
)

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

// HandleHook takes in the Hook from httpserver
// parses out info, makes some decisions, sends to forward queue
func HandleHook(h *Hook) {
	switch {
	case checkAgent(h.Header["User-Agent"], "Sonarr"):
		HandleSonarr(h)
	default:
		return
	}
}

// CheckAgent is just a tiny wrapper of HasPrefix
func checkAgent(a []string, s string) bool {
	return strings.HasPrefix(a[0], s)
}
