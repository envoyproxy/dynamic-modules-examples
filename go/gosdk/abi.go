//go:build cgo

package gosdk

// Following is a distillation of the Envoy ABI for dynamic modules:
// https://github.com/envoyproxy/envoy/blob/73fe00fc139fd5053f4c4a5d66569cc254449896/source/extensions/dynamic_modules/abi.h
//
// Why not using the header file directly? That is because Go runtime complains
// about passing pointers to C code on the boundary. In the following code, we replace
// all the pointers with uintptr_t instread of *char. At the end of the day, what we
// need from the header is declarations of callbacks, not event hooks, so it won't be that hard to maintain.

/*
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#cgo noescape envoy_dynamic_module_callback_http_get_request_header
#cgo nocallback envoy_dynamic_module_callback_http_get_request_header
size_t envoy_dynamic_module_callback_http_get_request_header(
    uintptr_t filter_envoy_ptr,
    uintptr_t key, size_t key_length,
    uintptr_t* result_buffer_ptr, size_t* result_buffer_length_ptr,
    size_t index);

#cgo noescape envoy_dynamic_module_callback_http_set_request_header
#cgo nocallback envoy_dynamic_module_callback_http_set_request_header
bool envoy_dynamic_module_callback_http_set_request_header(
    uintptr_t filter_envoy_ptr,
    uintptr_t key, size_t key_length,
    uintptr_t value, size_t value_length);

#cgo noescape envoy_dynamic_module_callback_http_get_response_header
#cgo nocallback envoy_dynamic_module_callback_http_get_response_header
size_t envoy_dynamic_module_callback_http_get_response_header(
    uintptr_t filter_envoy_ptr,
    uintptr_t key, size_t key_length,
    uintptr_t* result_buffer_ptr, size_t* result_buffer_length_ptr,
    size_t index);

#cgo noescape envoy_dynamic_module_callback_http_set_response_header
#cgo nocallback envoy_dynamic_module_callback_http_set_response_header
bool envoy_dynamic_module_callback_http_set_response_header(
    uintptr_t filter_envoy_ptr,
    uintptr_t key, size_t key_length,
    uintptr_t value, size_t value_length);

#cgo noescape envoy_dynamic_module_callback_http_append_request_body
#cgo nocallback envoy_dynamic_module_callback_http_append_request_body
bool envoy_dynamic_module_callback_http_append_request_body(
    uintptr_t filter_envoy_ptr,
    uintptr_t data, size_t length);

#cgo noescape envoy_dynamic_module_callback_http_drain_request_body
#cgo nocallback envoy_dynamic_module_callback_http_drain_request_body
bool envoy_dynamic_module_callback_http_drain_request_body(
	uintptr_t filter_envoy_ptr,
	size_t length);

#cgo noescape envoy_dynamic_module_callback_http_get_request_body_vector
#cgo nocallback envoy_dynamic_module_callback_http_get_request_body_vector
bool envoy_dynamic_module_callback_http_get_request_body_vector(
    uintptr_t filter_envoy_ptr,
    uintptr_t* result_buffer_vector);

#cgo noescape envoy_dynamic_module_callback_http_get_request_body_vector_size
#cgo nocallback envoy_dynamic_module_callback_http_get_request_body_vector_size
bool envoy_dynamic_module_callback_http_get_request_body_vector_size(
    uintptr_t filter_envoy_ptr, size_t* size);

#cgo noescape envoy_dynamic_module_callback_http_append_response_body
#cgo nocallback envoy_dynamic_module_callback_http_append_response_body
bool envoy_dynamic_module_callback_http_append_response_body(
    uintptr_t filter_envoy_ptr,
    uintptr_t data, size_t length);

#cgo noescape envoy_dynamic_module_callback_http_drain_response_body
#cgo nocallback envoy_dynamic_module_callback_http_drain_response_body
bool envoy_dynamic_module_callback_http_drain_response_body(
	uintptr_t filter_envoy_ptr,
	size_t length);

#cgo noescape envoy_dynamic_module_callback_http_get_response_body_vector
#cgo nocallback envoy_dynamic_module_callback_http_get_response_body_vector
bool envoy_dynamic_module_callback_http_get_response_body_vector(
    uintptr_t filter_envoy_ptr,
    uintptr_t* result_buffer_vector);

#cgo noescape envoy_dynamic_module_callback_http_get_response_body_vector_size
#cgo nocallback envoy_dynamic_module_callback_http_get_response_body_vector_size
bool envoy_dynamic_module_callback_http_get_response_body_vector_size(
    uintptr_t filter_envoy_ptr, size_t* size);

#cgo noescape envoy_dynamic_module_callback_http_send_response
// Uncomment once https://github.com/envoyproxy/envoy/pull/39206 is merged.
// #cgo nocallback envoy_dynamic_module_callback_http_send_response
void envoy_dynamic_module_callback_http_send_response(
    uintptr_t filter_envoy_ptr, uint32_t status_code,
    uintptr_t headers_vector, size_t headers_vector_size,
    uintptr_t body, size_t body_length);

#cgo noescape envoy_dynamic_module_callback_http_get_request_headers_count
#cgo nocallback envoy_dynamic_module_callback_http_get_request_headers_count
size_t envoy_dynamic_module_callback_http_get_request_headers_count(
	uintptr_t filter_envoy_ptr);

#cgo noescape envoy_dynamic_module_callback_http_get_request_headers
#cgo nocallback envoy_dynamic_module_callback_http_get_request_headers
bool envoy_dynamic_module_callback_http_get_request_headers(
    uintptr_t filter_envoy_ptr,
    uintptr_t* result_headers);

#cgo noescape envoy_dynamic_module_callback_http_get_request_headers_count
#cgo nocallback envoy_dynamic_module_callback_http_get_request_headers_count
size_t envoy_dynamic_module_callback_http_get_response_headers_count(
	uintptr_t filter_envoy_ptr);

#cgo noescape envoy_dynamic_module_callback_http_get_request_headers
#cgo nocallback envoy_dynamic_module_callback_http_get_request_headers
bool envoy_dynamic_module_callback_http_get_response_headers(
    uintptr_t filter_envoy_ptr,
    uintptr_t* result_headers);

#cgo noescape envoy_dynamic_module_callback_http_filter_get_attribute_string
#cgo nocallback envoy_dynamic_module_callback_http_filter_get_attribute_string
bool envoy_dynamic_module_callback_http_filter_get_attribute_string(
    uintptr_t filter_envoy_ptr,
    size_t attribute_id,
    uintptr_t* result, size_t* result_length);

#cgo noescape envoy_dynamic_module_callback_http_filter_continue_decoding
#cgo nocallback envoy_dynamic_module_callback_http_filter_continue_decoding
void envoy_dynamic_module_callback_http_filter_continue_decoding(
    uintptr_t filter_envoy_ptr);

#cgo noescape envoy_dynamic_module_callback_http_filter_continue_encoding
#cgo nocallback envoy_dynamic_module_callback_http_filter_continue_encoding
void envoy_dynamic_module_callback_http_filter_continue_encoding(
    uintptr_t filter_envoy_ptr);

#cgo noescape envoy_dynamic_module_callback_http_filter_scheduler_new
#cgo nocallback envoy_dynamic_module_callback_http_filter_scheduler_new
uintptr_t envoy_dynamic_module_callback_http_filter_scheduler_new(
	uintptr_t filter_envoy_ptr);

#cgo noescape envoy_dynamic_module_callback_http_filter_scheduler_delete
#cgo nocallback envoy_dynamic_module_callback_http_filter_scheduler_delete
void envoy_dynamic_module_callback_http_filter_scheduler_delete(
	uintptr_t scheduler_ptr);

#cgo noescape envoy_dynamic_module_callback_http_filter_scheduler_commit
#cgo nocallback envoy_dynamic_module_callback_http_filter_scheduler_commit
void envoy_dynamic_module_callback_http_filter_scheduler_commit(
	uintptr_t scheduler_ptr, uint64_t event_id);
*/
import "C"

import (
	"io"
	"runtime"
	"unsafe"
)

// https://github.com/envoyproxy/envoy/blob/73fe00fc139fd5053f4c4a5d66569cc254449896/source/extensions/dynamic_modules/abi_version.h
var version = append([]byte("cb17cd829c177bc6b75a920283a3347b90d5aaa4d5e723eaa33bad31c8c5b9a9"), 0)

//export envoy_dynamic_module_on_program_init
func envoy_dynamic_module_on_program_init() uintptr {
	return uintptr(unsafe.Pointer(&version[0]))
}

//export envoy_dynamic_module_on_http_filter_config_new
func envoy_dynamic_module_on_http_filter_config_new(
	_ uintptr,
	namePtr *C.char,
	nameSize C.size_t,
	configPtr *C.char,
	configSize C.size_t,
) uintptr {
	name := C.GoStringN(namePtr, C.int(nameSize))
	config := C.GoBytes(unsafe.Pointer(configPtr), C.int(configSize))
	filterConfig := NewHttpFilterConfig(name, config)
	if filterConfig == nil {
		return 0
	}
	// Pin the filter config to the memory manager.
	pinnedFilterConfig := memManager.pinHttpFilterConfig(filterConfig)
	return uintptr(unsafe.Pointer(pinnedFilterConfig))
}

//export envoy_dynamic_module_on_http_filter_config_destroy
func envoy_dynamic_module_on_http_filter_config_destroy(ptr uintptr) {
	pinnedFilterConfig := unwrapPinnedHttpFilterConfig(uintptr(ptr))
	pinnedFilterConfig.obj.Destroy()
	memManager.unpinHttpFilterConfig(pinnedFilterConfig)
}

//export envoy_dynamic_module_on_http_filter_new
func envoy_dynamic_module_on_http_filter_new(
	filterConfigPtr uintptr,
	_ uintptr,
) uintptr {
	pinnedFilterConfig := unwrapPinnedHttpFilterConfig(uintptr(filterConfigPtr))
	filterConfig := pinnedFilterConfig.obj
	filter := filterConfig.NewFilter()
	if filter == nil {
		return 0
	}
	// Pin the filter to the memory manager.
	pinned := memManager.pinHttpFilter(filter)
	// Return the pinned filter.
	return uintptr(unsafe.Pointer(pinned))
}

//export envoy_dynamic_module_on_http_filter_destroy
func envoy_dynamic_module_on_http_filter_destroy(
	filterPtr uintptr,
) {
	pinned := unwrapPinnedHttpFilter(uintptr(filterPtr))
	pinned.obj.Destroy()
	// Unpin the filter from the memory manager.
	memManager.unpinHttpFilter(pinned)
}

//export envoy_dynamic_module_on_http_filter_request_headers
func envoy_dynamic_module_on_http_filter_request_headers(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	endOfStream bool,
) uintptr {
	pinned := unwrapPinnedHttpFilter(uintptr(filterModulePtr))
	status := pinned.obj.RequestHeaders(envoyFilter{raw: filterEnvoyPtr}, bool(endOfStream))
	return uintptr(status)
}

//export envoy_dynamic_module_on_http_filter_request_body
func envoy_dynamic_module_on_http_filter_request_body(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	endOfStream bool,
) uintptr {
	pinned := unwrapPinnedHttpFilter(uintptr(filterModulePtr))
	status := pinned.obj.RequestBody(envoyFilter{raw: uintptr(filterEnvoyPtr)}, bool(endOfStream))
	return uintptr(status)
}

//export envoy_dynamic_module_on_http_filter_request_trailers
func envoy_dynamic_module_on_http_filter_request_trailers(uintptr, uintptr) uintptr {
	return 0
}

//export envoy_dynamic_module_on_http_filter_response_headers
func envoy_dynamic_module_on_http_filter_response_headers(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	endOfStream bool,
) uintptr {
	pinned := unwrapPinnedHttpFilter(uintptr(filterModulePtr))
	status := pinned.obj.ResponseHeaders(envoyFilter{raw: uintptr(filterEnvoyPtr)}, bool(endOfStream))
	return uintptr(status)
}

//export envoy_dynamic_module_on_http_filter_response_body
func envoy_dynamic_module_on_http_filter_response_body(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	endOfStream bool,
) uintptr {
	pinned := unwrapPinnedHttpFilter(uintptr(filterModulePtr))
	status := pinned.obj.ResponseBody(envoyFilter{raw: uintptr(filterEnvoyPtr)}, bool(endOfStream))
	return uintptr(status)
}

//export envoy_dynamic_module_on_http_filter_response_trailers
func envoy_dynamic_module_on_http_filter_response_trailers(uintptr, uintptr) uintptr {
	return 0
}

//export envoy_dynamic_module_on_http_filter_stream_complete
func envoy_dynamic_module_on_http_filter_stream_complete(uintptr, uintptr) {
}

//export envoy_dynamic_module_on_http_filter_http_callout_done
func envoy_dynamic_module_on_http_filter_http_callout_done(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	calloutID C.uint32_t,
	result C.uint32_t,
	headersPtr uintptr,
	headersSize C.size_t,
	bodyVectorPtr uintptr,
	bodyVectorSize C.size_t,
) {
	panic("TODO")
}

//export envoy_dynamic_module_on_http_filter_scheduled
func envoy_dynamic_module_on_http_filter_scheduled(
	filterEnvoyPtr uintptr,
	filterModulePtr uintptr,
	eventID C.uint64_t) {
	pinned := unwrapPinnedHttpFilter(uintptr(filterModulePtr))
	// Call the Scheduled method of the filter.
	pinned.obj.Scheduled(envoyFilter{raw: uintptr(filterEnvoyPtr)}, uint64(eventID))
}

// GetRequestHeader implements [EnvoyHttpFilter].
func (e envoyFilter) GetRequestHeader(key string) (string, bool) {
	keyPtr := uintptr(unsafe.Pointer(unsafe.StringData(key)))
	var resultBufferPtr *byte
	var resultBufferLengthPtr C.size_t

	ret := C.envoy_dynamic_module_callback_http_get_request_header(
		C.uintptr_t(e.raw),
		C.uintptr_t(keyPtr),
		C.size_t(len(key)),
		(*C.uintptr_t)(unsafe.Pointer(&resultBufferPtr)),
		(*C.size_t)(unsafe.Pointer(&resultBufferLengthPtr)),
		0,
	)

	if ret == 0 {
		return "", false
	}

	result := unsafe.Slice(resultBufferPtr, resultBufferLengthPtr)
	runtime.KeepAlive(key)
	return string(result), true
}

// GetResponseHeader implements [EnvoyHttpFilter].
func (e envoyFilter) GetResponseHeader(key string) (string, bool) {
	keyPtr := uintptr(unsafe.Pointer(unsafe.StringData(key)))
	var resultBufferPtr *byte
	var resultBufferLengthPtr C.size_t

	ret := C.envoy_dynamic_module_callback_http_get_response_header(
		C.uintptr_t(e.raw),
		C.uintptr_t(keyPtr),
		C.size_t(len(key)),
		(*C.uintptr_t)(unsafe.Pointer(&resultBufferPtr)),
		(*C.size_t)(unsafe.Pointer(&resultBufferLengthPtr)),
		0,
	)

	if ret == 0 {
		return "", false
	}

	result := unsafe.Slice(resultBufferPtr, resultBufferLengthPtr)
	runtime.KeepAlive(key)
	return string(result), true
}

// SetRequestHeader implements [EnvoyHttpFilter].
func (e envoyFilter) SetRequestHeader(key string, value []byte) bool {
	keyPtr := uintptr(unsafe.Pointer(unsafe.StringData(key)))
	valuePtr := uintptr(unsafe.Pointer(unsafe.SliceData(value)))

	ret := C.envoy_dynamic_module_callback_http_set_request_header(
		C.uintptr_t(e.raw),
		C.uintptr_t(keyPtr),
		C.size_t(len(key)),
		C.uintptr_t(valuePtr),
		C.size_t(len(value)),
	)

	runtime.KeepAlive(key)
	runtime.KeepAlive(value)
	return bool(ret)
}

// SetResponseHeader implements [EnvoyHttpFilter].
func (e envoyFilter) SetResponseHeader(key string, value []byte) bool {
	keyPtr := uintptr(unsafe.Pointer(unsafe.StringData(key)))
	valuePtr := uintptr(unsafe.Pointer(unsafe.SliceData(value)))

	ret := C.envoy_dynamic_module_callback_http_set_response_header(
		C.uintptr_t(e.raw),
		C.uintptr_t(keyPtr),
		C.size_t(len(key)),
		C.uintptr_t(valuePtr),
		C.size_t(len(value)),
	)

	runtime.KeepAlive(key)
	runtime.KeepAlive(value)
	return bool(ret)
}

// bodyReader implements [io.Reader] for the request or response body.
type bodyReader struct {
	chunks        []envoySlice
	index, offset int
}

// Read implements [io.Reader].
func (b *bodyReader) Read(p []byte) (n int, err error) {
	if b.index >= len(b.chunks) {
		return 0, io.EOF
	}

	chunk := b.chunks[b.index]
	if b.offset >= int(chunk.length) {
		b.index++
		b.offset = 0
		if b.index >= len(b.chunks) {
			return 0, io.EOF
		}
		chunk = b.chunks[b.index]
	}

	n = copy(p, unsafe.Slice((*byte)(unsafe.Pointer(chunk.data)), chunk.length)[b.offset:])
	b.offset += n
	return n, nil
}

type envoySlice struct {
	data   uintptr
	length C.size_t
}

// envoyFilter implements [EnvoyHttpFilter].
type envoyFilter struct{ raw uintptr }

// ContinueRequest implements EnvoyHttpFilter.
func (e envoyFilter) ContinueRequest() {
	C.envoy_dynamic_module_callback_http_filter_continue_decoding(C.uintptr_t(e.raw))
}

// ContinueResponse implements EnvoyHttpFilter.
func (e envoyFilter) ContinueResponse() {
	C.envoy_dynamic_module_callback_http_filter_continue_encoding(C.uintptr_t(e.raw))
}

// NewScheduler implements EnvoyHttpFilter.
func (e envoyFilter) NewScheduler() Scheduler {
	// Create a new scheduler for the filter.
	schedulerPtr := C.envoy_dynamic_module_callback_http_filter_scheduler_new(C.uintptr_t(e.raw))
	if schedulerPtr == 0 {
		return nil
	}
	return &envoyFilterScheduler{raw: uintptr(schedulerPtr)}
}

type envoyFilterScheduler struct {
	raw uintptr
}

// Close implements Scheduler.
func (e *envoyFilterScheduler) Close() {
	C.envoy_dynamic_module_callback_http_filter_scheduler_delete(C.uintptr_t(e.raw))
}

// Commit implements Scheduler.
func (e *envoyFilterScheduler) Commit(eventID uint64) {
	C.envoy_dynamic_module_callback_http_filter_scheduler_commit(C.uintptr_t(e.raw), C.uint64_t(eventID))
}

// GetRequestProtocol implements [EnvoyHttpFilter].
func (e envoyFilter) GetRequestProtocol() string {
	// https://github.com/envoyproxy/envoy/blob/05223ee2cd143d70b32402783c2a866a9dd18bd1/source/extensions/dynamic_modules/abi.h#L237-L372
	return e.getStringAttribute(10) // request.protocol
}

// GetSourceAddress implements [EnvoyHttpFilter].
func (e envoyFilter) GetSourceAddress() string {
	// https://github.com/envoyproxy/envoy/blob/05223ee2cd143d70b32402783c2a866a9dd18bd1/source/extensions/dynamic_modules/abi.h#L237-L372
	return e.getStringAttribute(24) // source.address
}

func (e envoyFilter) getStringAttribute(id int) string {
	var resultBufferPtr *byte
	var resultBufferLengthPtr int
	ret := C.envoy_dynamic_module_callback_http_filter_get_attribute_string(
		C.uintptr_t(e.raw),
		C.size_t(id),
		(*C.uintptr_t)(unsafe.Pointer(&resultBufferPtr)),
		(*C.size_t)(unsafe.Pointer(&resultBufferLengthPtr)),
	)
	if !ret {
		return ""
	}
	return string(unsafe.Slice(resultBufferPtr, resultBufferLengthPtr)) // Copy the result to a Go string.
}

// GetRequestHeaders implements EnvoyHttpFilter.
func (e envoyFilter) GetRequestHeaders() map[string][]string {
	count := C.envoy_dynamic_module_callback_http_get_request_headers_count(C.uintptr_t(e.raw))
	raw := make([][2]envoySlice, count)
	ret := C.envoy_dynamic_module_callback_http_get_request_headers(
		C.uintptr_t(e.raw),
		(*C.uintptr_t)(unsafe.Pointer(&raw[0])),
	)
	if !ret {
		return nil
	}
	// Copy the headers to a Go slice.
	headers := make(map[string][]string, count) // The count is the number of (key, value) pairs, so this might be larger than the number of unique names.
	for i := range count {
		// Copy the Envoy owner data to a Go string.
		key := string(unsafe.Slice((*byte)(unsafe.Pointer(raw[i][0].data)), raw[i][0].length))
		value := string(unsafe.Slice((*byte)(unsafe.Pointer(raw[i][1].data)), raw[i][1].length))
		headers[key] = append(headers[key], value)
	}
	return headers
}

// GetResponseHeaders implements [EnvoyHttpFilter].
func (e envoyFilter) GetResponseHeaders() map[string][]string {
	count := C.envoy_dynamic_module_callback_http_get_response_headers_count(C.uintptr_t(e.raw))
	raw := make([][2]envoySlice, count)
	ret := C.envoy_dynamic_module_callback_http_get_response_headers(
		C.uintptr_t(e.raw),
		(*C.uintptr_t)(unsafe.Pointer(&raw[0])),
	)
	if !ret {
		return nil
	}
	// Copy the headers to a Go slice.
	headers := make(map[string][]string, count) // The count is the number of (key, value) pairs, so this might be larger than the number of unique names.
	for i := range count {
		// Copy the Envoy owner data to a Go string.
		key := string(unsafe.Slice((*byte)(unsafe.Pointer(raw[i][0].data)), raw[i][0].length))
		value := string(unsafe.Slice((*byte)(unsafe.Pointer(raw[i][1].data)), raw[i][1].length))
		headers[key] = append(headers[key], value)
	}
	return headers
}

// SendLocalReply implements EnvoyHttpFilter.
func (e envoyFilter) SendLocalReply(statusCode uint32, headers [][2]string, body []byte) {
	headersVecPtr := uintptr(unsafe.Pointer(unsafe.SliceData(headers)))
	headersVecSize := len(headers)
	bodyPtr := uintptr(unsafe.Pointer(unsafe.SliceData(body)))
	bodySize := len(body)
	C.envoy_dynamic_module_callback_http_send_response(
		C.uintptr_t(e.raw),
		C.uint32_t(statusCode),
		C.uintptr_t(headersVecPtr),
		C.size_t(headersVecSize),
		C.uintptr_t(bodyPtr),
		C.size_t(bodySize),
	)
	runtime.KeepAlive(headers)
	runtime.KeepAlive(body)
}

// AppendRequestBody implements [EnvoyHttpFilter].
func (e envoyFilter) AppendRequestBody(data []byte) bool {
	dataPtr := uintptr(unsafe.Pointer(unsafe.SliceData(data)))
	ret := C.envoy_dynamic_module_callback_http_append_request_body(
		C.uintptr_t(e.raw),
		C.uintptr_t(dataPtr),
		C.size_t(len(data)),
	)
	runtime.KeepAlive(data)
	return bool(ret)
}

// DrainRequestBody implements [EnvoyHttpFilter].
func (e envoyFilter) DrainRequestBody(n int) bool {
	ret := C.envoy_dynamic_module_callback_http_drain_request_body(
		C.uintptr_t(e.raw),
		C.size_t(n),
	)
	return bool(ret)
}

// GetRequestBody implements [EnvoyHttpFilter].
func (e envoyFilter) GetRequestBody() (io.Reader, bool) {
	var vectorSize int
	ret := C.envoy_dynamic_module_callback_http_get_request_body_vector_size(
		C.uintptr_t(e.raw),
		(*C.size_t)(unsafe.Pointer(&vectorSize)),
	)
	if !ret {
		return nil, false
	}

	chunks := make([]envoySlice, vectorSize)
	ret = C.envoy_dynamic_module_callback_http_get_request_body_vector(
		C.uintptr_t(e.raw),
		(*C.uintptr_t)(unsafe.Pointer(&chunks[0])),
	)
	if !ret {
		return nil, false
	}
	return &bodyReader{chunks: chunks}, true
}

// AppendResponseBody implements [EnvoyHttpFilter].
func (e envoyFilter) AppendResponseBody(data []byte) bool {
	dataPtr := uintptr(unsafe.Pointer(unsafe.SliceData(data)))
	ret := C.envoy_dynamic_module_callback_http_append_response_body(
		C.uintptr_t(e.raw),
		C.uintptr_t(dataPtr),
		C.size_t(len(data)),
	)
	runtime.KeepAlive(data)
	return bool(ret)
}

// DrainResponseBody implements [EnvoyHttpFilter].
func (e envoyFilter) DrainResponseBody(n int) bool {
	ret := C.envoy_dynamic_module_callback_http_drain_response_body(
		C.uintptr_t(e.raw),
		C.size_t(n),
	)
	return bool(ret)
}

// GetResponseBody implements [EnvoyHttpFilter].
func (e envoyFilter) GetResponseBody() (io.Reader, bool) {
	var vectorSize int
	ret := C.envoy_dynamic_module_callback_http_get_response_body_vector_size(
		C.uintptr_t(e.raw),
		(*C.size_t)(unsafe.Pointer(&vectorSize)),
	)
	if !ret {
		return nil, false
	}
	chunks := make([]envoySlice, vectorSize)
	ret = C.envoy_dynamic_module_callback_http_get_response_body_vector(
		C.uintptr_t(e.raw),
		(*C.uintptr_t)(unsafe.Pointer(&chunks[0])),
	)
	if !ret {
		return nil, false
	}
	return &bodyReader{chunks: chunks}, true
}
