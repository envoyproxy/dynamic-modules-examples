package main

import (
	"net/http"

	"github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go/shared"
)

type (
	// headerAuthFilterConfigFactory implements [shared.HttpFilterConfigFactory].
	headerAuthFilterConfigFactory struct {
		shared.EmptyHttpFilterConfigFactory
	}
	// headerAuthFilterFactory implements [shared.HttpFilterFactory].
	//
	// This filter checks if the request header `authHeaderName` is present.
	headerAuthFilterFactory struct {
		authHeaderName string
	}
	// headerAuthFilter implements [shared.HttpFilter].
	headerAuthFilter struct {
		handle                    shared.HttpFilterHandle
		authHeaderName            string
		sendOnResponseHeaderPhase bool
		shared.EmptyHttpFilter
	}
)

// Create implements [shared.HttpFilterConfigFactory].
func (p *headerAuthFilterConfigFactory) Create(handle shared.HttpFilterConfigHandle, unparsedConfig []byte) (shared.HttpFilterFactory, error) {
	return &headerAuthFilterFactory{authHeaderName: string(unparsedConfig)}, nil
}

// Create implements [shared.HttpFilterFactory].
func (p *headerAuthFilterFactory) Create(handle shared.HttpFilterHandle) shared.HttpFilter {
	return &headerAuthFilter{handle: handle, authHeaderName: p.authHeaderName}
}

// OnRequestHeaders implements [shared.HttpFilter].
func (p *headerAuthFilter) OnRequestHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	v := headers.GetOne(p.authHeaderName)
	if v == "" {
		p.handle.SendLocalResponse(http.StatusUnauthorized, [][2]string{{"Content-Type", "text/plain"}}, []byte("Unauthorized by Go Module at on_request_headers\n"), "unauthorized")
		return shared.HeadersStatusStop
	}
	p.sendOnResponseHeaderPhase = v == "on_response_headers"
	return shared.HeadersStatusContinue
}

// OnResponseHeaders implements [shared.HttpFilter].
func (p *headerAuthFilter) OnResponseHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	if p.sendOnResponseHeaderPhase {
		p.handle.SendLocalResponse(http.StatusUnauthorized, [][2]string{{"Content-Type", "text/plain"}}, []byte("Unauthorized by Go Module at on_response_headers\n"), "unauthorized")
		return shared.HeadersStatusStop
	}
	return shared.HeadersStatusContinue
}
