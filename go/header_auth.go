package main

import (
	"net/http"

	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
)

type (
	// headerAuthFilterConfig implements [gosdk.HttpFilterConfig].
	//
	// This filter checks if the request header `authHeaderName` is present.
	headerAuthFilterConfig struct {
		authHeaderName string
	}
	// headerAuthFilter implements [gosdk.HttpFilter].
	headerAuthFilter struct{ authHeaderName string }
)

// Destroy implements [gosdk.HttpFilterConfig].
func (p headerAuthFilterConfig) Destroy() {}

// NewFilter implements [gosdk.HttpFilterConfig].
func (p headerAuthFilterConfig) NewFilter() gosdk.HttpFilter {
	return &headerAuthFilter{authHeaderName: p.authHeaderName}
}

// Destroy implements [gosdk.HttpFilter].
func (p headerAuthFilter) Destroy() {}

// RequestHeaders implements [gosdk.HttpFilter].
func (p headerAuthFilter) RequestHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestHeadersStatus {
	_, ok := e.GetRequestHeader(p.authHeaderName)
	if !ok {
		e.SendLocalReply(http.StatusUnauthorized, [][2]string{{"Content-Type", "text/plain"}}, []byte("Unauthorized by Go Module\n"))
		return gosdk.RequestHeadersStatusStopIteration
	}
	return gosdk.RequestHeadersStatusContinue
}

// RequestBody implements [gosdk.HttpFilter].
func (p headerAuthFilter) RequestBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestBodyStatus {
	return gosdk.RequestBodyStatusContinue
}

// ResponseHeaders implements [gosdk.HttpFilter].
func (p headerAuthFilter) ResponseHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseHeadersStatus {
	return gosdk.ResponseHeadersStatusContinue
}

// ResponseBody implements [gosdk.HttpFilter].
func (p headerAuthFilter) ResponseBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseBodyStatus {
	return gosdk.ResponseBodyStatusContinue
}
