package main

import (
	"bytes"
	"cmp"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/mccutchen/go-httpbin/v2/httpbin"
	io_prometheus_client "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"
	"github.com/stretchr/testify/require"
)

func TestIntegration(t *testing.T) {
	envoyImage := cmp.Or(os.Getenv("ENVOY_IMAGE"), "envoy-with-dynamic-modules:latest")

	cwd, err := os.Getwd()
	require.NoError(t, err)

	// Setup the httpbin upstream local server.
	httpbinHandler := httpbin.New()
	server := &http.Server{Addr: ":1234", Handler: httpbinHandler}
	go func() {
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			t.Logf("HTTP server error: %v", err)
		}
	}()
	t.Cleanup(func() { _ = server.Close() })

	// Create a directory for the access logs to be written to.
	accessLogsDir := cwd + "/access_logs"
	require.NoError(t, os.RemoveAll(accessLogsDir))
	require.NoError(t, os.Mkdir(accessLogsDir, 0o700))
	require.NoError(t, os.Chmod(accessLogsDir, 0o777))

	cmd := exec.Command(
		"docker",
		"run",
		"--network", "host",
		"-v", cwd+":/integration",
		"-w", "/integration",
		"--rm",
		envoyImage,
		"--concurrency", "1",
		"--config-path", "/integration/envoy.yaml",
		"--base-id", strconv.Itoa(time.Now().Nanosecond()),
	)
	cmd.Stderr = os.Stderr
	cmd.Stdout = os.Stdout
	require.NoError(t, cmd.Start())
	t.Cleanup(func() { require.NoError(t, cmd.Process.Signal(os.Interrupt)) })

	t.Run("http_access_logger", func(t *testing.T) {
		t.Run("health checking", func(t *testing.T) {
			require.Eventually(t, func() bool {
				req, err := http.NewRequest("GET", "http://localhost:1062/uuid", nil)
				require.NoError(t, err)

				resp, err := http.DefaultClient.Do(req)
				if err != nil {
					t.Logf("Envoy not ready yet: %v", err)
					return false
				}
				defer func() {
					require.NoError(t, resp.Body.Close())
				}()
				body, err := io.ReadAll(resp.Body)
				if err != nil {
					t.Logf("Envoy not ready yet: %v", err)
					return false
				}
				t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
				return resp.StatusCode == 200
			}, 120*time.Second, 1*time.Second)
		})

		require.Eventually(t, func() bool {
			// List files in the access log directory
			files, err := os.ReadDir(accessLogsDir)
			require.NoError(t, err)

			var accessLogFiles []string
			for _, file := range files {
				if strings.HasPrefix(file.Name(), "access_log") {
					accessLogFiles = append(accessLogFiles, file.Name())
				}
			}

			if len(accessLogFiles) == 0 {
				t.Logf("No access log files found yet")
				return false
			}

			// Read the first access log file.
			file, err := os.Open(accessLogsDir + "/" + accessLogFiles[0])
			require.NoError(t, err)
			defer func() {
				require.NoError(t, file.Close())
			}()
			content, err := io.ReadAll(file)
			require.NoError(t, err)

			type logLine struct {
				RequestHeaders  []string `json:"request_headers"`
				ResponseHeaders []string `json:"response_headers"`
			}

			var found bool
			for line := range strings.Lines(string(content)) {
				t.Log(line)
				if line == "" {
					continue
				}
				var log logLine
				require.NoError(t, json.Unmarshal([]byte(line), &log))
				if len(log.RequestHeaders) > 0 && len(log.ResponseHeaders) > 0 {
					found = true
				}
			}
			return found
		}, 30*time.Second, 1*time.Second)
	})

	t.Run("delay", func(t *testing.T) {
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1062/headers", nil)
			require.NoError(t, err)
			req.Header.Set("do-delay", "true")

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}

			t.Logf("response: headers=%v, body=%s", resp.Header, string(body))
			require.Equal(t, 200, resp.StatusCode)

			// Check the request header "delay-filter-on-scheduled: yes" added in the Scheduled phase.
			type httpBinHeadersBody struct {
				Headers map[string][]string `json:"headers"`
			}
			var headersBody httpBinHeadersBody
			require.NoError(t, json.Unmarshal(body, &headersBody))
			require.Contains(t, headersBody.Headers["Delay-Filter-On-Scheduled"], "yes")

			// We also need to check that the response headers were added.
			require.NotEmpty(t, resp.Header.Get("x-delay-filter-lapsed"), "x-delay-filter-lapsed header should be set")
			require.Regexp(t, `^2\.\d+s$`, resp.Header.Get("x-delay-filter-lapsed"), "x-delay-filter-lapsed header should be around 2s")
			return true
		}, 30*time.Second, 200*time.Millisecond)
	})

	t.Run("http_header_mutation", func(t *testing.T) {
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1062/headers", nil)
			require.NoError(t, err)

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}

			t.Logf("response: headers=%v, body=%s", resp.Header, string(body))
			require.Equal(t, 200, resp.StatusCode)

			// HttpBin returns a JSON object containing the request headers in this format.
			type httpBinHeadersBody struct {
				Headers map[string][]string `json:"headers"`
			}
			var headersBody httpBinHeadersBody
			require.NoError(t, json.Unmarshal(body, &headersBody))

			require.Contains(t, headersBody.Headers["X-Envoy-Header"], "envoy-header")
			require.Contains(t, headersBody.Headers["X-Envoy-Header2"], "envoy-header2")
			require.NotContains(t, headersBody.Headers, "apple")

			// We also need to check that the response headers were mutated.
			require.Equal(t, "bar", resp.Header.Get("Foo"))
			require.Equal(t, "bar2", resp.Header.Get("Foo2"))
			require.NotEmpty(t, resp.Header.Get("X-Upstream-Address"), resp.Header.Get("X-Upstream-Address"))
			require.Equal(t, "200", resp.Header.Get("X-Response-Code"))
			require.Equal(t, "", resp.Header.Get("Access-Control-Allow-Credentials"))
			return true
		}, 30*time.Second, 200*time.Millisecond)
	})

	t.Run("http_random_auth", func(t *testing.T) {
		// Without this, the Go module will reject the request.
		const gomoduleAuthHeader = "go-module-auth-header"
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1063/uuid", nil)
			require.NoError(t, err)
			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()

			if resp.StatusCode != http.StatusUnauthorized {
				t.Logf("unexpected status code: %d", resp.StatusCode)
				return false
			}
			body, err := io.ReadAll(resp.Body)
			require.NoError(t, err)
			t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
			require.Contains(t, string(body), "Unauthorized by Go Module")
			return true
		}, 30*time.Second, 200*time.Millisecond)

		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1063/uuid", nil)
			require.NoError(t, err)
			req.Header.Add(gomoduleAuthHeader, "on_response_headers")
			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			require.NoError(t, err)
			t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
			return resp.StatusCode == 401
		}, 30*time.Second, 200*time.Millisecond)

		got200 := false
		got403 := false
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1063/uuid", nil)
			require.NoError(t, err)
			req.Header.Add(gomoduleAuthHeader, "anything")
			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			require.NoError(t, err)
			t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
			if resp.StatusCode == 200 {
				got200 = true
			}
			if resp.StatusCode == 403 {
				got403 = true
			}
			return got200 && got403
		}, 30*time.Second, 200*time.Millisecond)
	})

	t.Run("http_zero_copy_regex_waf", func(t *testing.T) {
		t.Run("ok", func(t *testing.T) {
			require.Eventually(t, func() bool {
				data := strings.Repeat("a", 1000)
				req, err := http.NewRequest("GET", "http://localhost:1064/status/200", strings.NewReader(data))
				require.NoError(t, err)

				resp, err := http.DefaultClient.Do(req)
				if err != nil {
					t.Logf("Envoy not ready yet: %v", err)
					return false
				}
				defer func() {
					require.NoError(t, resp.Body.Close())
				}()
				body, err := io.ReadAll(resp.Body)
				if err != nil {
					t.Logf("Envoy not ready yet: %v", err)
					return false
				}
				t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
				return resp.StatusCode == 200
			}, 30*time.Second, 200*time.Millisecond)
		})

		for _, body := range []string{"bash -c 'curl https://some-url.com'", "bash -c 'wget https://some-url.com'"} {
			t.Run("bad "+body, func(t *testing.T) {
				require.Eventually(t, func() bool {
					req, err := http.NewRequest("GET", "http://localhost:1064/status/200", strings.NewReader(body))
					require.NoError(t, err)

					resp, err := http.DefaultClient.Do(req)
					if err != nil {
						t.Logf("Envoy not ready yet: %v", err)
						return false
					}
					defer func() {
						require.NoError(t, resp.Body.Close())
					}()
					body, err := io.ReadAll(resp.Body)
					if err != nil {
						t.Logf("Envoy not ready yet: %v", err)
						return false
					}
					t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
					return resp.StatusCode == 403
				}, 30*time.Second, 200*time.Millisecond)
			})
		}
	})

	t.Run("javascript", func(t *testing.T) {
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1062/headers", nil)
			require.NoError(t, err)
			req.Header.Set("dog", "cat")
			req.Header.Set("foo", "bar")

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}

			t.Logf("response: headers=%v, body=%s", resp.Header, string(body))
			require.Equal(t, 200, resp.StatusCode)

			// HttpBin returns a JSON object containing the request headers in this format.
			type httpBinHeadersBody struct {
				Headers map[string][]string `json:"headers"`
			}
			var headersBody httpBinHeadersBody
			require.NoError(t, json.Unmarshal(body, &headersBody))

			require.Contains(t, headersBody.Headers["X-Foo"], "bar")
			require.Contains(t, headersBody.Headers["Foo"], "bar")
			require.Contains(t, headersBody.Headers["Dog"], "cat")

			// We also need to check that the response headers were mutated.
			require.Equal(t, "cat", resp.Header.Get("x-dog"))
			require.Equal(t, "200", resp.Header.Get("x-status"))
			return true
		}, 30*time.Second, 200*time.Millisecond)
	})

	t.Run("http_metrics", func(t *testing.T) {
		// Send test request
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1062/uuid", nil)
			require.NoError(t, err)

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
			return resp.StatusCode == 200
		}, 30*time.Second, 200*time.Millisecond)

		// Check the metrics endpoint
		lastStatsOutput := ""
		t.Cleanup(func() {
			t.Logf("last stats output:\n%s", lastStatsOutput)
		})
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:9901/stats/prometheus", nil)
			require.NoError(t, err)

			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			defer func() {
				require.NoError(t, resp.Body.Close())
			}()

			// Check that the route_latency_ms metric is present
			body, err := io.ReadAll(resp.Body)
			require.NoError(t, err)
			lastStatsOutput = string(body)

			decoder := expfmt.NewDecoder(bytes.NewReader(body), expfmt.NewFormat(expfmt.TypeTextPlain))
			for {
				var metricFamily io_prometheus_client.MetricFamily
				err := decoder.Decode(&metricFamily)
				if err == io.EOF {
					break
				}
				require.NoError(t, err)

				if metricFamily.GetName() != "route_latency_ms" {
					continue
				}
				for _, metric := range metricFamily.GetMetric() {
					hist := metric.GetHistogram()
					require.NotNil(t, hist)
					labels := make(map[string]string)
					for _, label := range metric.GetLabel() {
						labels[label.GetName()] = label.GetValue()
					}
					require.Equal(t, map[string]string{"version": "v1.0.0", "route_name": "catch_all"}, labels)
					if hist.GetSampleCount() > 0 {
						return true
					}
				}
			}
			t.Logf("route_latency_ms metric not found or no samples yet")
			return false
		}, 5*time.Second, 200*time.Millisecond)
	})
}
