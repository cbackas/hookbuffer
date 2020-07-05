package main

import (
	"strings"
)

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
