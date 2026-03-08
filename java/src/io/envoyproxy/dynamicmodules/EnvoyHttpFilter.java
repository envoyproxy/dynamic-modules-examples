package io.envoyproxy.dynamicmodules;

/**
 * Interface for implementing Envoy HTTP filters in Java.
 *
 * <p>Implement this interface, package your class in a JAR (together with
 * {@link HeaderMutation}), and configure the Rust dynamic module:
 *
 * <pre>{@code
 * {
 *   "jar_path": "/path/to/your-filter.jar",
 *   "class_name": "com.example.YourFilter"
 * }
 * }</pre>
 *
 * <p>Your class must have a public no-arg constructor.  A single instance is
 * created per filter-chain config block and reused across all requests, so
 * implementations must be thread-safe if Envoy dispatches requests concurrently.
 */
public interface EnvoyHttpFilter {

    /**
     * Called when request headers arrive from the downstream client.
     *
     * @param names  header names  (parallel array)
     * @param values header values (parallel array, same length as {@code names})
     * @return mutations to apply, or {@code null} to leave headers unchanged
     *         and continue the filter chain
     */
    HeaderMutation onRequestHeaders(String[] names, String[] values);

    /**
     * Called when response headers arrive from the upstream.
     *
     * @param names  header names  (parallel array)
     * @param values header values (parallel array, same length as {@code names})
     * @return mutations to apply, or {@code null} to leave headers unchanged
     *         and continue the filter chain
     */
    HeaderMutation onResponseHeaders(String[] names, String[] values);
}
