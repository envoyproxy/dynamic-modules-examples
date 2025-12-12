package main

import (
	"io"

	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
)

// mockEnvoyHttpFilter is a mock implementation of [gosdk.EnvoyHttpFilter] for testing.
type mockEnvoyHttpFilter struct {
	getRequestHeader      func(key string) (string, bool)
	getRequestHeaders     func() map[string][]string
	setRequestHeader      func(key string, value []byte) bool
	getResponseHeader     func(key string) (string, bool)
	getResponseHeaders    func() map[string][]string
	setResponseHeader     func(key string, value []byte) bool
	getRequestBody        func() (io.Reader, bool)
	drainRequestBody      func(n int) bool
	appendRequestBody     func(data []byte) bool
	getResponseBody       func() (io.Reader, bool)
	drainResponseBody     func(n int) bool
	appendResponseBody    func(data []byte) bool
	sendLocalReply        func(statusCode uint32, headers [][2]string, body []byte)
	getSourceAddress      func() string
	getDestinationAddress func() string
	getRequestProtocol    func() string
	newScheduler          func() gosdk.Scheduler
	continueRequest       func()
	continueResponse      func()
}

// GetRequestHeader implements [gosdk.EnvoyHttpFilter.GetRequestHeader].
func (m mockEnvoyHttpFilter) GetRequestHeader(key string) (string, bool) {
	return m.getRequestHeader(key)
}

// GetRequestHeaders implements [gosdk.EnvoyHttpFilter.GetRequestHeaders].
func (m mockEnvoyHttpFilter) GetRequestHeaders() map[string][]string {
	return m.getRequestHeaders()
}

// SetRequestHeader implements [gosdk.EnvoyHttpFilter.SetRequestHeader].
func (m mockEnvoyHttpFilter) SetRequestHeader(key string, value []byte) bool {
	return m.setRequestHeader(key, value)
}

// GetResponseHeader implements [gosdk.EnvoyHttpFilter.GetResponseHeader].
func (m mockEnvoyHttpFilter) GetResponseHeader(key string) (string, bool) {
	return m.getResponseHeader(key)
}

// GetResponseHeaders implements [gosdk.EnvoyHttpFilter.GetResponseHeaders].
func (m mockEnvoyHttpFilter) GetResponseHeaders() map[string][]string {
	return m.getResponseHeaders()
}

// SetResponseHeader implements [gosdk.EnvoyHttpFilter.SetResponseHeader].
func (m mockEnvoyHttpFilter) SetResponseHeader(key string, value []byte) bool {
	return m.setResponseHeader(key, value)
}

// GetRequestBody implements [gosdk.EnvoyHttpFilter.GetRequestBody].
func (m mockEnvoyHttpFilter) GetRequestBody() (io.Reader, bool) {
	return m.getRequestBody()
}

// DrainRequestBody implements [gosdk.EnvoyHttpFilter.DrainRequestBody].
func (m mockEnvoyHttpFilter) DrainRequestBody(n int) bool {
	return m.drainRequestBody(n)
}

// AppendRequestBody implements [gosdk.EnvoyHttpFilter.AppendRequestBody].
func (m mockEnvoyHttpFilter) AppendRequestBody(data []byte) bool {
	return m.appendRequestBody(data)
}

// GetResponseBody implements [gosdk.EnvoyHttpFilter.GetResponseBody].
func (m mockEnvoyHttpFilter) GetResponseBody() (io.Reader, bool) {
	return m.getResponseBody()
}

// DrainResponseBody implements [gosdk.EnvoyHttpFilter.DrainResponseBody].
func (m mockEnvoyHttpFilter) DrainResponseBody(n int) bool {
	return m.drainResponseBody(n)
}

// AppendResponseBody implements [gosdk.EnvoyHttpFilter.AppendResponseBody].
func (m mockEnvoyHttpFilter) AppendResponseBody(data []byte) bool {
	return m.appendResponseBody(data)
}

// SendLocalReply implements [gosdk.EnvoyHttpFilter.SendLocalReply].
func (m mockEnvoyHttpFilter) SendLocalReply(statusCode uint32, headers [][2]string, body []byte) {
	m.sendLocalReply(statusCode, headers, body)
}

// GetSourceAddress implements [gosdk.EnvoyHttpFilter.GetSourceAddress].
func (m mockEnvoyHttpFilter) GetSourceAddress() string {
	return m.getSourceAddress()
}

// GetDestinationAddress implements [gosdk.EnvoyHttpFilter.GetDestinationAddress].
func (m mockEnvoyHttpFilter) GetDestinationAddress() string {
	return m.getDestinationAddress()
}

// GetRequestProtocol implements [gosdk.EnvoyHttpFilter.GetRequestProtocol].
func (m mockEnvoyHttpFilter) GetRequestProtocol() string {
	return m.getRequestProtocol()
}

// NewScheduler implements [gosdk.EnvoyHttpFilter.NewScheduler].
func (m mockEnvoyHttpFilter) NewScheduler() gosdk.Scheduler {
	return m.newScheduler()
}

// ContinueRequest implements [gosdk.EnvoyHttpFilter.ContinueRequest].
func (m mockEnvoyHttpFilter) ContinueRequest() {
	m.continueRequest()
}

// ContinueResponse implements [gosdk.EnvoyHttpFilter.ContinueResponse].
func (m mockEnvoyHttpFilter) ContinueResponse() {
	m.continueResponse()
}
