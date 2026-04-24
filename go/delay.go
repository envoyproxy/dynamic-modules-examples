package main

import (
	"time"

	"github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go/shared"
)

type (
	// delayFilterConfigFactory implements [shared.HttpFilterConfigFactory].
	delayFilterConfigFactory struct {
		shared.EmptyHttpFilterConfigFactory
	}
	// delayFilterFactory implements [shared.HttpFilterFactory].
	delayFilterFactory struct{}
	// delayFilter implements [shared.HttpFilter].
	//
	// This filter demonstrates how to use the scheduler to delay the request processing,
	// and how to use goroutines to perform the asynchronous operations.
	delayFilter struct {
		handle           shared.HttpFilterHandle
		onRequestHeaders time.Time
		delayLapsed      time.Duration
		shared.EmptyHttpFilter
	}
)

// Create implements [shared.HttpFilterConfigFactory].
func (p *delayFilterConfigFactory) Create(handle shared.HttpFilterConfigHandle, unparsedConfig []byte) (shared.HttpFilterFactory, error) {
	return &delayFilterFactory{}, nil
}

// Create implements [shared.HttpFilterFactory].
func (p *delayFilterFactory) Create(handle shared.HttpFilterHandle) shared.HttpFilter {
	return &delayFilter{handle: handle}
}

// OnDestroy implements [shared.HttpFilterFactory].
func (p *delayFilterFactory) OnDestroy() {}

// OnRequestHeaders implements [shared.HttpFilter].
func (p *delayFilter) OnRequestHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	// Check if the headers contain the "do-delay" header to trigger the delay.
	if len(headers.Get("do-delay")) == 0 {
		// If the header is not present, continue the request processing.
		return shared.HeadersStatusContinue
	}

	scheduler := p.handle.GetScheduler()
	now := time.Now()
	p.onRequestHeaders = now
	go func() {
		// Simulate some delay.
		time.Sleep(2 * time.Second)
		// Commit the event to continue the request processing.
		scheduler.Schedule(func() {
			p.delayLapsed = time.Since(p.onRequestHeaders)
			// We can insert some headers at this phase.
			headers := p.handle.RequestHeaders()
			headers.Set("delay-filter-on-scheduled", "yes")
			// Then continue the request processing.
			p.handle.ContinueRequest()
		})
	}()
	return shared.HeadersStatusStop
}

// OnResponseHeaders implements [shared.HttpFilter].
func (p *delayFilter) OnResponseHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	// Add a response header to indicate the delay.
	if p.delayLapsed > 0 {
		headers.Set("x-delay-filter-lapsed", p.delayLapsed.String())
	}
	return shared.HeadersStatusContinue
}
