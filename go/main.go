package main

import (
	"fmt"
	"io"

	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
)

func main() {}

// Set the envoy.NewHttpFilter function to create a new http filter.
func init() { gosdk.NewHttpFilterConfig = newHttpFilterConfig }

// newHttpFilter creates a new http filter based on the config.
//
// `config` is the configuration string that is specified in the Envoy configuration.
func newHttpFilterConfig(name string, config []byte) gosdk.HttpFilterConfig {
	switch name {
	case "passthrough":
		return passthroughFilterConfig{}
	default:
		panic("unknown filter: " + name)
	}
}

type (
	// passthroughFilterConfig implements [gosdk.HttpFilterConfig].
	passthroughFilterConfig struct{}
	// passthroughFilter implements [gosdk.HttpFilter].
	passsthroughFilter struct{}
)

// Destroy implements [gosdk.HttpFilterConfig].
func (p passthroughFilterConfig) Destroy() {}

// NewFilter implements [gosdk.HttpFilterConfig].
func (p passthroughFilterConfig) NewFilter() gosdk.HttpFilter { return passsthroughFilter{} }

// Destroy implements [gosdk.HttpFilter].
func (p passsthroughFilter) Destroy() {}

// RequestHeaders implements [gosdk.HttpFilter].
func (p passsthroughFilter) RequestHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestHeadersStatus {
	fooValue, _ := e.GetRequestHeader("foo")
	fmt.Printf("gosdk: RequestHeaders, foo: %v\n", fooValue)
	fmt.Printf("gosdk: RequestHeaders, endOfStream: %v\n", endOfStream)
	return gosdk.RequestHeadersStatusContinue
}

// RequestBody implements [gosdk.HttpFilter].
func (p passsthroughFilter) RequestBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.RequestBodyStatus {
	if !endOfStream {
		// Wait for the end of stream.
		return gosdk.RequestBodyStatusStopIterationAndBuffer
	}
	fmt.Println("gosdk: RequestBody")
	r, ok := e.GetRequestBody()
	if !ok {
		panic("request body should be set")
	}
	original, err := io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	fmt.Printf("gosdk: RequestBody, body: %s\n", original)
	e.DrainRequestBody(len(original))
	e.AppendRequestBody([]byte("hello world"))
	r, ok = e.GetRequestBody()
	if !ok {
		panic("request body should be set")
	}
	modified, err := io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	if string(modified) != "hello world" {
		panic("request body should be modified")
	}

	// Write it back.
	e.DrainRequestBody(len(modified))
	e.AppendRequestBody(original)
	r, ok = e.GetRequestBody()
	if !ok {
		panic("request body should be set")
	}
	modified, err = io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	if string(modified) != string(original) {
		panic("request body should be modified")
	}
	return gosdk.RequestBodyStatusContinue
}

// ResponseHeaders implements [gosdk.HttpFilter].
func (p passsthroughFilter) ResponseHeaders(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseHeadersStatus {
	status, ok := e.GetResponseHeader(":status")
	if !ok {
		panic("x-status header should be set")
	}
	fmt.Printf("gosdk: ResponseHeaders, status: %v\n", status)
	e.SetResponseHeader("x-passthrough-response-header", []byte("true"))
	return gosdk.ResponseHeadersStatusContinue
}

// ResponseBody implements [gosdk.HttpFilter].
func (p passsthroughFilter) ResponseBody(e gosdk.EnvoyHttpFilter, endOfStream bool) gosdk.ResponseBodyStatus {
	if !endOfStream {
		// Wait for the end of stream.
		return gosdk.ResponseBodyStatusStopIterationAndBuffer
	}

	r, ok := e.GetResponseBody()
	if !ok {
		panic("response body should be set")
	}
	original, err := io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	fmt.Printf("gosdk: ResponseBody, body: %s\n", original)
	e.DrainResponseBody(len(original))
	e.AppendResponseBody([]byte("hello world"))
	r, ok = e.GetResponseBody()
	if !ok {
		panic("response body should be set")
	}
	modified, err := io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	if string(modified) != "hello world" {
		panic("response body should be modified")
	}
	// Write it back.
	e.DrainResponseBody(len(modified))
	e.AppendResponseBody(original)
	r, ok = e.GetResponseBody()
	if !ok {
		panic("response body should be set")
	}
	modified, err = io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	if string(modified) != string(original) {
		panic("response body should be modified")
	}
	return gosdk.ResponseBodyStatusContinue
}
