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

	cmd := exec.Command(
		"docker",
		"run",
		"-p", "1062:1062",
		"-v", os.Getenv("PWD")+":/integration",
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
	}, 10*time.Second, 100*time.Millisecond)
}
