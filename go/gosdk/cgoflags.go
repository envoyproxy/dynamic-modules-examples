//go:build darwin

package gosdk

// #cgo LDFLAGS: -Wl,-undefined,dynamic_lookup
import "C"
