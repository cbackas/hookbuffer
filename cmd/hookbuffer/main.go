package main

func main() {
	// infinite loop that starts a new http server any time the existing one fails
	for true {
		err := StartHTTPServer()
		if err != nil {
			continue
		}
	}
}
