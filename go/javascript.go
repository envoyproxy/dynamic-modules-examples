package main

import (
	"fmt"
	"log"
	"math/rand"
	"strings"
	"sync"

	"github.com/dop251/goja"
	"github.com/envoyproxy/dynamic-modules-examples/go/gosdk"
)

const (
	javaScriptExportedSymbolOnConfig          = "OnConfigure"
	javaScriptExportedSymbolOnRequestHeaders  = "OnRequestHeaders"
	javaScriptExportedSymbolOnResponseHeaders = "OnResponseHeaders"

	functionDeclTemplate = `globalThis.%[1]s = %[1]s`
	numberOfVMPool       = 24
)

type (
	// javaScriptFilterConfig implements [gosdk.HttpFilterConfig].
	javaScriptFilterConfig struct {
		vms [numberOfVMPool]*javaScriptVM
	}
	// javaScriptFilter implements [gosdk.HttpFilter].
	javaScriptFilter struct {
		vm              *javaScriptVM
		requestHeaders  map[string]string
		responseHeaders map[string]string
	}
	javaScriptVM struct {
		*goja.Runtime
		mux               sync.Mutex
		onRequestHeaders  goja.Callable
		onResponseHeaders goja.Callable
	}
)

func newJavaScriptFilterConfig(script string) gosdk.HttpFilterConfig {
	c := &javaScriptFilterConfig{}

	script = strings.Join([]string{
		script,
		fmt.Sprintf(functionDeclTemplate, javaScriptExportedSymbolOnConfig),
		fmt.Sprintf(functionDeclTemplate, javaScriptExportedSymbolOnRequestHeaders),
		fmt.Sprintf(functionDeclTemplate, javaScriptExportedSymbolOnResponseHeaders),
	}, "\n")

	for i := range numberOfVMPool {
		vm, err := newJavaScriptVM(script)
		if err != nil {
			log.Printf("failed to create JavaScript VM: %v", err)
			return nil
		}
		c.vms[i] = vm
	}
	return c
}

func newJavaScriptVM(script string) (*javaScriptVM, error) {
	vm := goja.New()
	console := vm.NewObject()
	err := console.Set("log", func(call goja.FunctionCall) goja.Value {
		args := make([]interface{}, 0, len(call.Arguments))
		for _, a := range call.Arguments {
			args = append(args, a.Export())
		}
		fmt.Println(args...)
		return goja.Undefined()
	})
	if err != nil {
	}
	err = vm.Set("console", console)
	if err != nil {
		return nil, fmt.Errorf("failed to set console: %w", err)
	}

	_, err = vm.RunString(script)
	if err != nil {
		return nil, fmt.Errorf("failed to run script: %w", err)
	}

	// Call OnConfigure.
	onConfigure, ok := goja.AssertFunction(vm.GlobalObject().Get(javaScriptExportedSymbolOnConfig))
	if !ok {
		return nil, fmt.Errorf("failed to get %s function", javaScriptExportedSymbolOnConfig)
	}
	_, err = onConfigure(goja.Undefined())
	if err != nil {
		return nil, fmt.Errorf("failed to call %s function: %w", javaScriptExportedSymbolOnConfig, err)
	}

	ret := &javaScriptVM{Runtime: vm}
	// Check two exported functions.
	ret.onRequestHeaders, ok = goja.AssertFunction(vm.GlobalObject().Get(javaScriptExportedSymbolOnRequestHeaders))
	if !ok {
		return nil, fmt.Errorf("failed to get %s function", javaScriptExportedSymbolOnRequestHeaders)
	}
	ret.onResponseHeaders, ok = goja.AssertFunction(vm.GlobalObject().Get(javaScriptExportedSymbolOnResponseHeaders))
	if !ok {
		return nil, fmt.Errorf("failed to get %s function", javaScriptExportedSymbolOnResponseHeaders)
	}
	return ret, nil
}

// NewFilter implements [gosdk.HttpFilterConfig].
func (p *javaScriptFilterConfig) NewFilter() gosdk.HttpFilter {
	vm := p.vms[rand.Intn(numberOfVMPool)]
	return &javaScriptFilter{vm: vm, requestHeaders: make(map[string]string), responseHeaders: make(map[string]string)}
}

// RequestHeaders implements [gosdk.HttpFilter].
func (p *javaScriptFilter) RequestHeaders(e gosdk.EnvoyHttpFilter, _ bool) gosdk.RequestHeadersStatus {
	headers := e.GetRequestHeaders()
	for k, vs := range headers {
		p.requestHeaders[k] = vs[0]
	}
	p.vm.mux.Lock()
	defer p.vm.mux.Unlock()
	vm := p.vm
	obj := vm.NewObject()
	_ = obj.Set("getRequestHeader", func(call goja.FunctionCall) goja.Value {
		if len(call.Arguments) < 1 {
			return vm.ToValue("")
		}
		key := call.Argument(0).String()
		return vm.ToValue(p.requestHeaders[key])
	})
	_ = obj.Set("setRequestHeader", func(call goja.FunctionCall) goja.Value {
		if len(call.Arguments) < 2 {
			return goja.Undefined()
		}
		key := call.Argument(0).String()
		value := call.Argument(1).String()
		p.requestHeaders[key] = value
		e.SetRequestHeader(key, []byte(value))
		return goja.Undefined()
	})
	if _, err := vm.onRequestHeaders(goja.Undefined(), obj); err != nil {
		log.Printf("failed to call %s: %v", javaScriptExportedSymbolOnRequestHeaders, err)
		return gosdk.RequestHeadersStatusStopIteration
	}
	return gosdk.RequestHeadersStatusContinue
}

// ResponseHeaders implements [gosdk.HttpFilter].
func (p *javaScriptFilter) ResponseHeaders(e gosdk.EnvoyHttpFilter, _ bool) gosdk.ResponseHeadersStatus {
	headers := e.GetResponseHeaders()
	for k, vs := range headers {
		p.responseHeaders[k] = vs[0]
	}
	p.vm.mux.Lock()
	defer p.vm.mux.Unlock()
	vm := p.vm
	obj := vm.NewObject()
	_ = obj.Set("getRequestHeader", func(call goja.FunctionCall) goja.Value {
		if len(call.Arguments) < 1 {
			return vm.ToValue("")
		}
		key := call.Argument(0).String()
		return vm.ToValue(p.requestHeaders[key])
	})

	// Setting request header in response phase is not allowed.

	_ = obj.Set("getResponseHeader", func(call goja.FunctionCall) goja.Value {
		if len(call.Arguments) < 1 {
			return vm.ToValue("")
		}
		key := call.Argument(0).String()
		return vm.ToValue(p.responseHeaders[key])
	})
	_ = obj.Set("setResponseHeader", func(call goja.FunctionCall) goja.Value {
		if len(call.Arguments) < 2 {
			return goja.Undefined()
		}
		key := call.Argument(0).String()
		value := call.Argument(1).String()
		p.responseHeaders[key] = value
		e.SetResponseHeader(key, []byte(value))
		return goja.Undefined()
	})
	return gosdk.ResponseHeadersStatusContinue
}

// Destroy implements [gosdk.HttpFilterConfig].
func (p *javaScriptFilterConfig) Destroy() {}

// Scheduled implements gosdk.HttpFilter.
func (p *javaScriptFilter) Scheduled(gosdk.EnvoyHttpFilter, uint64) {}

// Destroy implements [gosdk.HttpFilter].
func (p *javaScriptFilter) Destroy() {}

// RequestBody implements [gosdk.HttpFilter].
func (p *javaScriptFilter) RequestBody(gosdk.EnvoyHttpFilter, bool) gosdk.RequestBodyStatus {
	return gosdk.RequestBodyStatusContinue
}

// ResponseBody implements [gosdk.HttpFilter].
func (p *javaScriptFilter) ResponseBody(gosdk.EnvoyHttpFilter, bool) gosdk.ResponseBodyStatus {
	return gosdk.ResponseBodyStatusContinue
}
