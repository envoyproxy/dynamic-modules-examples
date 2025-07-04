package main

import (
	"fmt"
	"strconv"
	"time"

	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
)

type (
	// delayFilterConfig implements [gosdk.HttpFilterConfig].
	delayFilterConfig struct{}
	// delayFilter implements [gosdk.HttpFilter].
	//
	// This filter demostrates how to use the scheduler to delay the request processing,
	// and how to use goroutines to perform the asynchronous operations.
	delayFilter struct {
		onRequestHeaders time.Time
		delayLapsed      time.Duration
	}
)

// Destroy implements [gosdk.HttpFilterConfig].
func (p delayFilterConfig) Destroy() {}

// NewFilter implements [gosdk.HttpFilterConfig].
func (p delayFilterConfig) NewFilter() gosdk.HttpFilter { return &delayFilter{} }

// Destroy implements [gosdk.HttpFilter].
func (p *delayFilter) Destroy() {}

// RequestHeaders implements [gosdk.HttpFilter].
func (p *delayFilter) RequestHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestHeadersStatus {
	// Check if the headers contain the "do-delay" header to trigger the delay.
	if _, ok := e.GetRequestHeader("do-delay"); !ok {
		// If the header is not present, continue the request processing.
		fmt.Println("gosdk: RequestHeaders, do-delay header not found, continuing request processing")
		return gosdk.RequestHeadersStatusContinue
	}

	schduler := e.NewScheduler()
	now := time.Now()
	p.onRequestHeaders = now
	go func() {
		// Simulate some delay.
		time.Sleep(2 * time.Second)
		// Commit the event to continue the request processing.
		schduler.Commit(0)
	}()
	fmt.Printf("gosdk: RequestHeaders, delaying request processing for 2 seconds at %s\n", now)
	return gosdk.RequestHeadersStatusStopIteration
}

// Sheduled implements gosdk.HttpFilter.
func (p *delayFilter) Sheduled(e gosdk.EnvoyHttpFilter, eventID uint64) {
	if eventID != 0 {
		panic("unexpected eventID in Sheduled: " + strconv.Itoa(int(eventID)))
	}
	fmt.Println("gosdk: Sheduled, continuing request processing after delay")
	p.delayLapsed = time.Since(p.onRequestHeaders)
	// We can insert some headers at this phase.
	e.SetRequestHeader("delay-filter-on-scheduled", []byte("yes"))
	// Then continue the request processing.
	e.ContinueRequest()
}

// RequestBody implements [gosdk.HttpFilter].
func (p *delayFilter) RequestBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestBodyStatus {
	return gosdk.RequestBodyStatusContinue
}

// ResponseHeaders implements [gosdk.HttpFilter].
func (p *delayFilter) ResponseHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseHeadersStatus {
	// Add a response header to indicate the delay.
	if p.delayLapsed > 0 {
		e.SetResponseHeader("x-delay-filter-lapsed", []byte(p.delayLapsed.String()))
	}
	return gosdk.ResponseHeadersStatusContinue
}

// ResponseBody implements [gosdk.HttpFilter].
func (p *delayFilter) ResponseBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseBodyStatus {
	return gosdk.ResponseBodyStatusContinue
}
