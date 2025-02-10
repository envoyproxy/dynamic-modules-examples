package main

import (
	_ "embed"
	"encoding/json"
	"fmt"
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

//go:embed envoy.yaml
var originalEnvoyYaml string

func requireEnvoyYaml(t *testing.T, tmpdir string) (yamlPath string) {
	yamlPath = tmpdir + "/envoy.yaml"
	replacedYaml := strings.ReplaceAll(originalEnvoyYaml, "/tmp/", tmpdir)
	require.NoError(t, os.WriteFile(yamlPath, []byte(replacedYaml), 0644))
	fmt.Println("Envoy config:", replacedYaml)
	return
}

func TestIntegration(t *testing.T) {
	envoyImage := "envoy-with-dynamic-modules:latest"
	if os.Getenv("ENVOY_IMAGE") != "" {
		envoyImage = os.Getenv("ENVOY_IMAGE")
	}

	cwd, err := os.Getwd()
	require.NoError(t, err)

	tmpdir := t.TempDir()
	// Grant write permission to the tmpdir for the envoy process.
	require.NoError(t, exec.Command("chmod", "777", tmpdir).Run())
	yamlPath := requireEnvoyYaml(t, tmpdir)

	cmd := exec.Command(
		"docker",
		"run",
		"--network", "host",
		"-v", cwd+":/integration",
		"-v", tmpdir+":"+tmpdir,
		"-w", tmpdir,
		envoyImage,
		"--concurrency", "1",
		"--config-path", yamlPath,
		"--base-id", strconv.Itoa(time.Now().Nanosecond()),
	)
	cmd.Stderr = os.Stderr
	cmd.Stdout = os.Stdout
	cmd.Env = append(os.Environ(), "ENVOY_UID=0")
	require.NoError(t, cmd.Start())
	t.Cleanup(func() {
		require.NoError(t, cmd.Process.Kill())
	})

	// Let's wait at least 5 seconds for Envoy to start since it might take a while
	// to pull the image.
	time.Sleep(5 * time.Second)

	t.Run("health checking", func(t *testing.T) {
		require.Eventually(t, func() bool {
			req, err := http.NewRequest("GET", "http://localhost:1062/uuid", nil)
			require.NoError(t, err)

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			defer resp.Body.Close()
			body, err := io.ReadAll(resp.Body)
			if err != nil {
				t.Logf("Envoy not ready yet: %v", err)
				return false
			}
			t.Logf("response: status=%d body=%s", resp.StatusCode, string(body))
			return resp.StatusCode == 200
		}, 30*time.Second, 1*time.Second)
	})

	t.Run("check access log", func(t *testing.T) {
		require.Eventually(t, func() bool {
			// List files in the access log directory
			files, err := os.ReadDir(tmpdir)
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

			// Read the first access log file
			file, err := os.Open(tmpdir + "/" + accessLogFiles[0])
			require.NoError(t, err)
			defer file.Close()
			content, err := io.ReadAll(file)
			require.NoError(t, err)

			type logLine struct {
				RequestHeaders  []string `json:"request_headers"`
				ResponseHeaders []string `json:"response_headers"`
			}

			var found bool
			for _, line := range strings.Split(string(content), "\n") {
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
}
