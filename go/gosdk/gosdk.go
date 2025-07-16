// Package gosdk provides the Go API for the Envoy filter chains.
package gosdk

import "io"

// NewHttpFilter is a function that creates a new HttpFilter that corresponds to each filter configuration in the Envoy filter chain.
// This is a global variable that should be set in the init function in the program once.
//
// The function is called once globally. The function is only called by the main thread,
// so it does not need to be thread-safe.
var NewHttpFilterConfig func(name string, config []byte) HttpFilterConfig

// HttpFilter is an interface that represents a single http filter in the Envoy filter chain.
// It is used to create HttpFilter(s) that correspond to each Http request.
//
// This is only created once per module configuration via the NewHttpFilter function.
type HttpFilterConfig interface {
	// NewFilter is called for each new Http request.
	// Note that this must be concurrency-safe as it can be called concurrently for multiple requests.
	NewFilter() HttpFilter

	// Destroy is called when this filter is destroyed. E.g. the filter chain configuration is updated and removed from the Envoy.
	Destroy()
}

// EnvoyHttpFilter is an interface that represents the underlying Envoy filter.
// This is passed to each event hook of the HttpFilter.
//
// **WARNING**: This must not outlive each event hook since there's no guarantee that the EnvoyHttpFilter will be valid after the event hook is returned.
// To perform the asynchronous operations, use [EnvoyHttpFilter.NewScheduler] to create a [Scheduler] and perform the operations in a separate Goroutine.
// Then, use the [Scheduler.Commit] method to commit the event to the Envoy filter on the correct worker thread to continue processing the request.
type EnvoyHttpFilter interface {
	// GetRequestHeader gets the first value of the request header. Returns the value and true if the header is found.
	GetRequestHeader(key string) (string, bool)
	// GetRequestHeaders gets all the request headers.
	GetRequestHeaders() map[string][]string
	// SetRequestHeader sets the request header. Returns true if the header is set successfully.
	SetRequestHeader(key string, value []byte) bool
	// GetResponseHeader gets the first value of the response header. Returns the value and true if the header is found.
	GetResponseHeader(key string) (string, bool)
	// GetResponseHeaders gets all the response headers.
	GetResponseHeaders() map[string][]string
	// SetResponseHeader sets the response header. Returns true if the header is set successfully.
	SetResponseHeader(key string, value []byte) bool
	// GetRequestBody gets the request body. Returns the io.Reader and true if the body is found.
	GetRequestBody() (io.Reader, bool)
	// DrainRequestBody drains n bytes from the request body. This will invalidate the io.Reader returned by GetRequestBody before this is called.
	DrainRequestBody(n int) bool
	// AppendRequestBody appends the data to the request body. This will invalidate the io.Reader returned by GetRequestBody before this is called.
	AppendRequestBody(data []byte) bool
	// GetResponseBody gets the response body. Returns the io.Reader and true if the body is found.
	GetResponseBody() (io.Reader, bool)
	// DrainResponseBody drains n bytes from the response body. This will invalidate the io.Reader returned by GetResponseBody before this is called.
	DrainResponseBody(n int) bool
	// AppendResponseBody appends the data to the response body. This will invalidate the io.Reader returned by GetResponseBody before this is called.
	AppendResponseBody(data []byte) bool
	// SendLocalReply sends a local reply to the client. This must not be used in after returning continue from the response headers phase.
	SendLocalReply(statusCode uint32, headers [][2]string, body []byte)
	// GetSourceAddress gets the source address of the request in the format of "IP:PORT".
	// This corresponds to `source.address` attribute https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/attributes.
	GetSourceAddress() string
	// GetRequestProtocol gets the request protocol. This corresponds to `request.protocol` attribute https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/attributes.
	GetRequestProtocol() string
	// NewScheduler creates a new Scheduler that can be used to schedule events to the correct Envoy worker thread.
	// Created schedulers must be closed when they are no longer needed.
	//
	// Returns nil if this is called from any other than normal event hooks such as RequestHeaders, RequestBody, ResponseHeaders, and ResponseBody.
	NewScheduler() Scheduler
	// ContinueRequest continues the request processing after the Stop variants are returned from the normal event hooks such as RequestHeaders, RequestBody, ResponseHeaders, and ResponseBody.
	// Mainly this is intented to be used during the HttpFilter.Scheduled method being called.
	ContinueRequest()
	// ContinueResponse is the same as ContinueRequest but for the response processing.
	ContinueResponse()
}

// Scheduler is an interface that can be used to schedule a generic event to the correct Envoy worker thread.
//
// This is created via [EnvoyHttpFilter.NewScheduler] and can be passed across Goroutines.
type Scheduler interface {
	// Commit commits the event to the Envoy filter on the correct worker thread.
	// The eventID is a unique identifier for the event, and it can be used to distinguish between different events.
	Commit(eventID uint64)
	// Close closes the scheduler and releases any resources associated with it.
	// This must be called when the scheduler is no longer needed to avoid memory leaks.
	Close()
}

// HttpFilter is an interface that represents each Http request.
//
// Thisis created for each new Http request and is destroyed when the request is completed.
type HttpFilter interface {
	// RequestHeaders is called when the request headers are received.
	RequestHeaders(e EnvoyHttpFilter, endOfStream bool) RequestHeadersStatus
	// RequestBody is called when the request body is received.
	RequestBody(e EnvoyHttpFilter, endOfStream bool) RequestBodyStatus
	// TODO: add RequestTrailers support.

	// ResponseHeaders is called when the response headers are received.
	ResponseHeaders(e EnvoyHttpFilter, endOfStream bool) ResponseHeadersStatus
	// ResponseBody is called when the response body is received.
	ResponseBody(e EnvoyHttpFilter, endOfStream bool) ResponseBodyStatus
	// TODO: add ResponseTrailers support.

	// Scheuled is called when the filter is scheduled to run on the Envoy worker thread.
	// Such event is created via [Scheduler.Commit] and the eventID is the unique identifier for the event.
	Scheduled(e EnvoyHttpFilter, eventID uint64)

	// Destroy is called when the stream is destroyed.
	Destroy()
}

// RequestHeadersStatus is the return value of the HttpFilter.RequestHeaders.
type RequestHeadersStatus int

const (
	// RequestHeadersStatusContinue is returned when the operation should continue.
	RequestHeadersStatusContinue                  RequestHeadersStatus = 0
	RequestHeadersStatusStopIteration             RequestHeadersStatus = 1
	RequestHeadersStatusStopAllIterationAndBuffer RequestHeadersStatus = 3
)

// RequestBodyStatus is the return value of the HttpFilter.RequestBody event.
type RequestBodyStatus int

const (
	RequestBodyStatusContinue               RequestBodyStatus = 0
	RequestBodyStatusStopIterationAndBuffer RequestBodyStatus = 1
)

// ResponseHeadersStatus is the return value of the HttpFilter.ResponseHeaders event.
type ResponseHeadersStatus int

const (
	ResponseHeadersStatusContinue                  ResponseHeadersStatus = 0
	ResponseHeadersStatusStopIteration             ResponseHeadersStatus = 1
	ResponseHeadersStatusStopAllIterationAndBuffer ResponseHeadersStatus = 3
)

// ResponseBodyStatus is the return value of the HttpFilter.ResponseBody event.
type ResponseBodyStatus int

const (
	ResponseBodyStatusContinue               ResponseBodyStatus = 0
	ResponseBodyStatusStopIterationAndBuffer ResponseBodyStatus = 1
)
