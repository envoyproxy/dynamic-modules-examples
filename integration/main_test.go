package main

import (
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

	"github.com/stretchr/testify/require"
)

func TestIntegration(t *testing.T) {
	envoyImage := cmp.Or(os.Getenv("ENVOY_IMAGE"), "envoy-with-dynamic-modules:latest")

	cwd, err := os.Getwd()
	require.NoError(t, err)

	// Create a directory for the access logs to be written to.
	accessLogsDir := cwd + "/access_logs"
	require.NoError(t, os.RemoveAll(accessLogsDir))
	require.NoError(t, os.Mkdir(accessLogsDir, 0700))
	require.NoError(t, os.Chmod(accessLogsDir, 0777))

	cmd := exec.Command(
		"docker",
		"run",
		"--network", "host",
		"-v", cwd+":/integration",
		"-w", "/integration",
		envoyImage,
		"--concurrency", "1",
		"--config-path", "/integration/envoy.yaml",
		"--base-id", strconv.Itoa(time.Now().Nanosecond()),
	)
	cmd.Stderr = os.Stderr
	cmd.Stdout = os.Stdout
	require.NoError(t, cmd.Start())
	t.Cleanup(func() { require.NoError(t, cmd.Process.Signal(os.Interrupt)) })

	// Let's wait at least 5 seconds for Envoy to start since it might take a while
	// to pull the image.
	time.Sleep(5 * time.Second)

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
			}, 30*time.Second, 1*time.Second)
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

			// HttpBin returns a JSON object containing the request headers.
			type httpBinHeadersBody struct {
				Headers map[string]string `json:"headers"`
			}
			var headersBody httpBinHeadersBody
			require.NoError(t, json.Unmarshal(body, &headersBody))

			require.Equal(t, "envoy-header", headersBody.Headers["X-Envoy-Header"])
			require.Equal(t, "envoy-header2", headersBody.Headers["X-Envoy-Header2"])
			require.NotContains(t, headersBody.Headers, "apple")

			// We also need to check that the response headers were mutated.
			require.Equal(t, "bar", resp.Header.Get("Foo"))
			require.Equal(t, "bar2", resp.Header.Get("Foo2"))
			require.Equal(t, "", resp.Header.Get("Access-Control-Allow-Credentials"))
			return true
		}, 30*time.Second, 200*time.Millisecond)
	})

	t.Run("http_random_auth", func(t *testing.T) {
		got200 := false
		got403 := false
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
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
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
}
