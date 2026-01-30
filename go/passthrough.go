package main

import (
	"fmt"

	"github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go/shared"
)

type (
	// passthroughFilterConfigFactory implements [shared.HttpFilterConfigFactory].
	passthroughFilterConfigFactory struct {
		shared.EmptyHttpFilterConfigFactory
	}
	// passthroughFilterFactory implements [shared.HttpFilterFactory].
	passthroughFilterFactory struct{}
	// passthroughFilter implements [shared.HttpFilter].
	passthroughFilter struct {
		handle shared.HttpFilterHandle
		shared.EmptyHttpFilter
	}
)

// Create implements [shared.HttpFilterConfigFactory].
func (p *passthroughFilterConfigFactory) Create(handle shared.HttpFilterConfigHandle, unparsedConfig []byte) (shared.HttpFilterFactory, error) {
	return &passthroughFilterFactory{}, nil
}

// Create implements [shared.HttpFilterFactory].
func (p *passthroughFilterFactory) Create(handle shared.HttpFilterHandle) shared.HttpFilter {
	return &passthroughFilter{handle: handle}
}

// OnRequestHeaders implements [shared.HttpFilter].
func (p *passthroughFilter) OnRequestHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	fooValue := headers.GetOne("foo")
	fmt.Printf("gosdk: RequestHeaders, foo: %v\n", fooValue)
	fmt.Printf("gosdk: RequestHeaders, endOfStream: %v\n", endOfStream)
	for _, header := range headers.GetAll() {
		fmt.Printf("gosdk: RequestHeaders, header: %s: %s\n", header[0], header[1])
	}
	sourceAddr, _ := p.handle.GetAttributeString(shared.AttributeIDSourceAddress)
	destAddr, _ := p.handle.GetAttributeString(shared.AttributeIDDestinationAddress)
	protocol, _ := p.handle.GetAttributeString(shared.AttributeIDRequestProtocol)
	fmt.Printf("gosdk: RequestHeaders, source address: %s\n", sourceAddr)
	fmt.Printf("gosdk: RequestHeaders, destination address: %s\n", destAddr)
	fmt.Printf("gosdk: RequestHeaders, request protocol: %s\n", protocol)
	return shared.HeadersStatusContinue
}

// OnRequestBody implements [shared.HttpFilter].
func (p *passthroughFilter) OnRequestBody(body shared.BodyBuffer, endOfStream bool) shared.BodyStatus {
	if !endOfStream {
		// Wait for the end of stream.
		return shared.BodyStatusStopAndBuffer
	}
	fmt.Println("gosdk: RequestBody")
	chunks := body.GetChunks()
	var original []byte
	for _, chunk := range chunks {
		original = append(original, chunk...)
	}
	fmt.Printf("gosdk: RequestBody, body: %s\n", original)
	body.Drain(uint64(len(original)))
	body.Append([]byte("hello world"))
	chunks = body.GetChunks()
	var modified []byte
	for _, chunk := range chunks {
		modified = append(modified, chunk...)
	}
	if string(modified) != "hello world" {
		panic("request body should be modified")
	}

	// Write it back.
	body.Drain(uint64(len(modified)))
	body.Append(original)
	chunks = body.GetChunks()
	modified = nil
	for _, chunk := range chunks {
		modified = append(modified, chunk...)
	}
	if string(modified) != string(original) {
		panic("request body should be modified")
	}
	return shared.BodyStatusContinue
}

// OnResponseHeaders implements [shared.HttpFilter].
func (p *passthroughFilter) OnResponseHeaders(headers shared.HeaderMap, endOfStream bool) shared.HeadersStatus {
	status := headers.GetOne(":status")
	if status == "" {
		panic("x-status header should be set")
	}
	fmt.Printf("gosdk: ResponseHeaders, status: %v\n", status)
	headers.Set("x-passthrough-response-header", "true")
	for _, header := range headers.GetAll() {
		fmt.Printf("gosdk: ResponseHeaders, header: %s: %s\n", header[0], header[1])
	}
	return shared.HeadersStatusContinue
}

// OnResponseBody implements [shared.HttpFilter].
func (p *passthroughFilter) OnResponseBody(body shared.BodyBuffer, endOfStream bool) shared.BodyStatus {
	if !endOfStream {
		// Wait for the end of stream.
		return shared.BodyStatusStopAndBuffer
	}

	chunks := body.GetChunks()
	var original []byte
	for _, chunk := range chunks {
		original = append(original, chunk...)
	}
	fmt.Printf("gosdk: ResponseBody, body: %s\n", original)
	body.Drain(uint64(len(original)))
	body.Append([]byte("hello world"))
	chunks = body.GetChunks()
	var modified []byte
	for _, chunk := range chunks {
		modified = append(modified, chunk...)
	}
	if string(modified) != "hello world" {
		panic("response body should be modified")
	}
	// Write it back.
	body.Drain(uint64(len(modified)))
	body.Append(original)
	chunks = body.GetChunks()
	modified = nil
	for _, chunk := range chunks {
		modified = append(modified, chunk...)
	}
	if string(modified) != string(original) {
		panic("response body should be modified")
	}
	return shared.BodyStatusContinue
}
