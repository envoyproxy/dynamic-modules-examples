package io.envoyproxy.dynamicmodules;

/**
 * Describes mutations the Java filter wants to apply to HTTP headers.
 *
 * <p>Return an instance of this class (or {@code null} for no changes) from
 * {@link EnvoyHttpFilter#onRequestHeaders} and
 * {@link EnvoyHttpFilter#onResponseHeaders}.
 *
 * <p>All fields are optional; leave them {@code null} or {@code false} to
 * skip the corresponding action.
 */
public class HeaderMutation {

    /**
     * If {@code true} the filter chain is stopped (Envoy StopIteration).
     * Use this to short-circuit a request, e.g. for authentication failures.
     * The upstream will not receive the request.
     */
    public boolean stopIteration = false;

    /**
     * Headers to add or overwrite, expressed as alternating name/value pairs:
     * {@code ["name0", "value0", "name1", "value1", …]}.
     *
     * <p>Must be an even-length array (or {@code null}).
     */
    public String[] addHeaders = null;

    /**
     * Names of headers to remove.  {@code null} means no removals.
     */
    public String[] removeHeaders = null;
}
