package main

import (
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
	case "header_auth":
		return headerAuthFilterConfig{authHeaderName: string(config)}
	case "delay":
		return delayFilterConfig{}
	case "javascript":
		return newJavaScriptFilterConfig(string(config))
	default:
		panic("unknown filter: " + name)
	}
}
