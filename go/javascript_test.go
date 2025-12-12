package main

import (
	"bytes"
	"testing"

	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
	"github.com/stretchr/testify/require"
)

func Test_newJavaScriptFilterConfig(t *testing.T) {
	f := newJavaScriptFilterConfig(`
function OnConfigure () {}
function OnRequestHeaders(ctx) {}
function OnResponseHeaders(ctx) {}
`)
	require.NotNil(t, f)
}

func Test_newJavasScriptVM(t *testing.T) {
	for _, tc := range []struct {
		name   string
		script string
		expOut string
		expErr string
	}{
		{
			name:   "valid script with all functions",
			expOut: `OnConfigure called`,
			script: `
function OnConfigure () {
  console.log("OnConfigure called");
}
function OnRequestHeaders(ctx) {
  console.log("OnRequestHeader called");
}
function OnResponseHeaders(ctx) {
  console.log("OnResponseHeader called");
}
`,
		},
		{
			name: "invalid script with missing functions",
			script: `
function OnConfigure () {
  console.log("OnConfigure called");
}
`,
			expErr: `failed to get OnRequestHeaders function`,
		},
		{
			name:   "invalid script",
			script: `invalid`,
			expErr: `failed to run script: ReferenceError: invalid is not defined at <eval>:1:1(0)`,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			logout := &bytes.Buffer{}
			_, err := newJavaScriptVM(tc.script, logout)
			if tc.expErr == "" {
				require.Equal(t, tc.expOut, logout.String())
				require.NoError(t, err)
			} else {
				require.ErrorContains(t, err, tc.expErr)
			}
		})
	}
}

func Test_javaScriptFilter_RequestHeaders(t *testing.T) {
	logout := &bytes.Buffer{}
	vm, err := newJavaScriptVM(
		`function OnConfigure () {}
function OnRequestHeaders(ctx) {
  ctx.setRequestHeader("x-hello", "world");
  let reqId = ctx.getRequestHeader("x-request-id");
  console.log("Request ID: ", reqId);
}
function OnResponseHeaders(ctx) {}`, logout)
	require.NoError(t, err)

	f := &javaScriptFilter{vm: vm, requestHeaders: map[string]string{
		"x-request-id": "12345",
	}}
	called := false
	m := &mockEnvoyHttpFilter{
		getRequestHeaders: func() map[string][]string { return map[string][]string{"x-request-id": {"12345"}} },
		setRequestHeader: func(key string, value []byte) bool {
			require.Equal(t, "x-hello", key)
			require.Equal(t, "world", string(value))
			called = true
			return true
		},
	}

	status := f.RequestHeaders(m, false)
	require.Equal(t, gosdk.RequestHeadersStatusContinue, status)
	require.True(t, called)

	require.Contains(t, logout.String(), "Request ID: 12345")
}

func Test_javaScriptFilter_ResponseHeaders(t *testing.T) {
	logout := &bytes.Buffer{}
	vm, err := newJavaScriptVM(
		`function OnConfigure () {}
function OnRequestHeaders(ctx) {}
function OnResponseHeaders(ctx) {
  ctx.setResponseHeader("x-hello", "world");
  let status = ctx.getResponseHeader(":status");
  console.log("Response status: ", status);
}`, logout)
	require.NoError(t, err)

	f := &javaScriptFilter{vm: vm, responseHeaders: map[string]string{
		":status": "200",
	}}
	called := false
	m := &mockEnvoyHttpFilter{
		getResponseHeaders: func() map[string][]string { return map[string][]string{":status": {"200"}} },
		setResponseHeader: func(key string, value []byte) bool {
			require.Equal(t, "x-hello", key)
			require.Equal(t, "world", string(value))
			called = true
			return true
		},
	}

	status := f.ResponseHeaders(m, false)
	require.Equal(t, gosdk.ResponseHeadersStatusContinue, status)
	require.True(t, called)

	require.Contains(t, logout.String(), "Response status: 200")
}
