package main

import (
	sdk "github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go"
	_ "github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go/abi"
	"github.com/envoyproxy/envoy/source/extensions/dynamic_modules/sdk/go/shared"
)

func main() {}

// init registers HTTP filter config factories.
func init() {
	sdk.RegisterHttpFilterConfigFactories(map[string]shared.HttpFilterConfigFactory{
		"passthrough": &passthroughFilterConfigFactory{},
		"header_auth": &headerAuthFilterConfigFactory{},
		"delay":       &delayFilterConfigFactory{},
		"javascript":  &javaScriptFilterConfigFactory{},
	})
}
