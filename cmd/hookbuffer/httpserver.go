package main

import (
	"io/ioutil"
	"log"
	"net/http"
)

// Hook holds all the info that needs needs to be processed and forwarded
type Hook struct {
	URL      string
	Header   http.Header
	BodyByte []byte
	Body     Body
}

// StartHTTPServer starts the webhook listening server
func StartHTTPServer() error {
	log.Println("server started")
	http.HandleFunc("/", handleWebhook)
	// log.Fatal(http.ListenAndServe(":5369", nil))
	err := http.ListenAndServe(":5369", nil)
	if err != nil {
		return err
	}
	return nil
}

func handleWebhook(w http.ResponseWriter, r *http.Request) {
	body, err := ioutil.ReadAll(r.Body)
	if err != nil {
		return
	}
	defer r.Body.Close()

	// build Hook obj with relevant forwarding info
	h := Hook{URL: r.URL.String(), Header: r.Header, BodyByte: body}

	// send data to get handled async
	// so http listener can respond and move on
	go HandleHook(&h)
}
