package main

import (
	_ "embed"
	"io"
	"net/http"
	"os"
	"os/exec"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
)

//go:embed envoy.yaml
var envoyYaml string

func TestIntegration(t *testing.T) {
	envoyImage := "envoy-with-dynamic-modules:latest"
	if os.Getenv("ENVOY_IMAGE") != "" {
		envoyImage = os.Getenv("ENVOY_IMAGE")
	}

	cwd, err := os.Getwd()
	require.NoError(t, err)

	cmd := exec.Command(
		"docker",
		"run",
		"--network", "host",
		"-v", cwd+":/integration",
		"-w", "/integration",
		envoyImage,
		"--concurrency", "1",
		"--config-path", "envoy.yaml",
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
}
