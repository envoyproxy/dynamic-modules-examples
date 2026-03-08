package io.envoyproxy.dynamicmodules;

/**
 * Example Envoy HTTP filter implemented in Java.
 *
 * <p>This filter:
 * <ul>
 *   <li>Adds {@code x-java-filter: active} to every request.</li>
 *   <li>Mirrors the request {@code :path} pseudo-header back as
 *       {@code x-java-filter-path} on the response.</li>
 *   <li>Removes the {@code x-powered-by} response header (if present).</li>
 * </ul>
 *
 * <p>Build and package this example with:
 * <pre>{@code
 * make -C java
 * }</pre>
 *
 * <p>Then reference it in your Envoy config:
 * <pre>{@code
 * {
 *   "jar_path": "/path/to/envoy-java-filter.jar",
 *   "class_name": "io.envoyproxy.dynamicmodules.ExampleFilter"
 * }
 * }</pre>
 */
public class ExampleFilter implements EnvoyHttpFilter {

    // Captured from request headers; written into the response.
    // In a concurrent environment you would use ThreadLocal instead.
    private volatile String lastPath = "";

    @Override
    public HeaderMutation onRequestHeaders(String[] names, String[] values) {
        // Capture :path for use in onResponseHeaders.
        for (int i = 0; i < names.length; i++) {
            if (":path".equals(names[i])) {
                lastPath = values[i];
                break;
            }
        }

        HeaderMutation m = new HeaderMutation();
        m.addHeaders = new String[]{"x-java-filter", "active"};
        return m;
    }

    @Override
    public HeaderMutation onResponseHeaders(String[] names, String[] values) {
        HeaderMutation m = new HeaderMutation();
        m.addHeaders    = new String[]{"x-java-filter-path", lastPath};
        m.removeHeaders = new String[]{"x-powered-by"};
        return m;
    }
}
