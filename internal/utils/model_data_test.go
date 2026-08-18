package utils

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// payload mirrors the shape of the Hugging Face tree response, including the split model that is the reason a model can
// be more than one file.
const payload = `[
  {"path":"models/dn_stockholm_fp32.onnx","size":100,"lfs":{"oid":"aaa"}},
  {"path":"models/up_osaka_fp16.onnx","size":10,"lfs":{"oid":"bbb"}},
  {"path":"models/up_osaka_fp16.onnx.data","size":5000,"lfs":{"oid":"ccc"}}
]`

// withModelDataUrl points the loader at a test server for the duration of a test.
func withModelDataUrl(t *testing.T, url string) {
	t.Helper()

	original := modelDataUrlOverride
	modelDataUrlOverride = url
	t.Cleanup(func() { modelDataUrlOverride = original })
}

func assertPayload(t *testing.T, data []internal.RemoteModelData) {
	t.Helper()

	if len(data) != 3 {
		t.Fatalf("manifest holds %d entries, want 3", len(data))
	}
	if data[0].Name != "dn_stockholm_fp32.onnx" || data[0].Size != 100 || data[0].Hash != "aaa" {
		t.Errorf("first entry = %+v, want the parsed name, size and LFS oid", data[0])
	}
	if data[2].Name != "up_osaka_fp16.onnx.data" || data[2].Size != 5000 {
		t.Errorf("weights entry = %+v, want the external data blob", data[2])
	}
}

func TestLoadModelDataCachesAndFallsBack(t *testing.T) {
	t.Run("a successful fetch is cached", func(t *testing.T) {
		root := setupConfigRoot(t)

		srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// The tree endpoint pages at 50 by default, so the limit is not decoration: without it models fall off the
			// manifest silently and are then installed with nothing checking them.
			if r.URL.Query().Get("limit") == "" {
				t.Error("the request must carry a limit, or the manifest is silently truncated")
			}
			w.Write([]byte(payload))
		}))
		defer srv.Close()
		withModelDataUrl(t, srv.URL+"/tree?limit=1000")

		data, err := LoadModelData()
		if err != nil {
			t.Fatalf("LoadModelData: %v", err)
		}
		assertPayload(t, data)

		if _, err = os.Stat(filepath.Join(root, internal.ModelsDir, modelDataFile)); err != nil {
			t.Errorf("a successful fetch must be cached: %v", err)
		}
	})

	// This is the case that used to disable verification without saying so: a slow or unreachable Hugging Face turned
	// every expected hash into an empty string, and the whole session downloaded models unchecked.
	t.Run("a failed fetch falls back to the cache", func(t *testing.T) {
		setupConfigRoot(t)

		good := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
			w.Write([]byte(payload))
		}))
		withModelDataUrl(t, good.URL+"/tree?limit=1000")
		if _, err := LoadModelData(); err != nil {
			t.Fatalf("priming LoadModelData: %v", err)
		}
		good.Close()

		bad := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
			w.WriteHeader(http.StatusInternalServerError)
		}))
		defer bad.Close()
		withModelDataUrl(t, bad.URL+"/tree?limit=1000")

		data, err := LoadModelData()
		if err != nil {
			t.Fatalf("LoadModelData must fall back rather than fail: %v", err)
		}
		assertPayload(t, data)
	})

	// With neither a fetch nor a cache there is genuinely nothing to verify against, and the caller has to be told.
	t.Run("no fetch and no cache is an error", func(t *testing.T) {
		setupConfigRoot(t)

		bad := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
			w.WriteHeader(http.StatusInternalServerError)
		}))
		defer bad.Close()
		withModelDataUrl(t, bad.URL+"/tree?limit=1000")

		if _, err := LoadModelData(); err == nil {
			t.Error("LoadModelData succeeded with no source of data, want an error")
		}
	})
}
