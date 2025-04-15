package gosdk

import (
	"sync"
	"unsafe"
)

var memManager memoryManager

const (
	shardingSize = 1 << 8
	shardingMask = shardingSize - 1
)

type (
	// memoryManager manages the heap allocated objects.
	// It is used to pin the objects to the heap to avoid them being garbage collected by the Go runtime.
	memoryManager struct {
		// httpFilterConfigs holds a linked lists of HttpFilter.
		httpFilterConfigs    *pinedHttpFilterConfig
		httpFilterLists      [shardingSize]*pinedHttpFilter
		httpFilterListsMuxes [shardingSize]sync.Mutex
	}

	// pinedHttpFilterConfig holds a pinned HttpFilter managed by the memory manager.
	pinedHttpFilterConfig = linkedList[HttpFilterConfig]

	// pinedHttpFilter holds a pinned HttpFilter managed by the memory manager.
	pinedHttpFilter = linkedList[HttpFilter]

	linkedList[T any] struct {
		obj        T
		next, prev *linkedList[T]
	}
)

// pinHttpFilterConfig pins the HttpFilterConfig to the memory manager.
func (m *memoryManager) pinHttpFilterConfig(filterConfig HttpFilterConfig) *pinedHttpFilterConfig {
	item := &pinedHttpFilterConfig{obj: filterConfig, next: m.httpFilterConfigs, prev: nil}
	if m.httpFilterConfigs != nil {
		m.httpFilterConfigs.prev = item
	}
	m.httpFilterConfigs = item
	return item
}

// unpinHttpFilterConfig unpins the HttpFilterConfig from the memory manager.
func (m *memoryManager) unpinHttpFilterConfig(filterConfig *pinedHttpFilterConfig) {
	if filterConfig.prev != nil {
		filterConfig.prev.next = filterConfig.next
	} else {
		m.httpFilterConfigs = filterConfig.next
	}
	if filterConfig.next != nil {
		filterConfig.next.prev = filterConfig.prev
	}
}

// unwrapPinnedHttpFilterConfig unwraps the pinned http filter config.
func unwrapPinnedHttpFilterConfig(raw uintptr) *pinedHttpFilterConfig {
	return (*pinedHttpFilterConfig)(unsafe.Pointer(raw))
}

// pinHttpFilter pins the http filter to the memory manager.
func (m *memoryManager) pinHttpFilter(filter HttpFilter) *pinedHttpFilter {
	item := &pinedHttpFilter{obj: filter, next: nil, prev: nil}
	index := shardingKey(uintptr(unsafe.Pointer(item)))
	mux := &m.httpFilterListsMuxes[index]
	mux.Lock()
	defer mux.Unlock()
	item.next = m.httpFilterLists[index]
	if m.httpFilterLists[index] != nil {
		m.httpFilterLists[index].prev = item
	}
	m.httpFilterLists[index] = item
	return item
}

// unpinHttpFilter unpins the http filter from the memory manager.
func (m *memoryManager) unpinHttpFilter(filter *pinedHttpFilter) {
	index := shardingKey(uintptr(unsafe.Pointer(filter)))
	mux := &m.httpFilterListsMuxes[index]
	mux.Lock()
	defer mux.Unlock()

	if filter.prev != nil {
		filter.prev.next = filter.next
	} else {
		m.httpFilterLists[index] = filter.next
	}
	if filter.next != nil {
		filter.next.prev = filter.prev
	}
}

// unwrapPinnedHttpFilter unwraps the raw pointer to the pinned http filter.
func unwrapPinnedHttpFilter(raw uintptr) *pinedHttpFilter {
	return (*pinedHttpFilter)(unsafe.Pointer(raw))
}

func shardingKey(key uintptr) uintptr {
	return splitmix64(key) & shardingMask
}

func splitmix64(x uintptr) uintptr {
	x += 0x9e3779b97f4a7c15
	x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9
	x = (x ^ (x >> 27)) * 0x94d049bb133111eb
	x = x ^ (x >> 31)
	return x
}
