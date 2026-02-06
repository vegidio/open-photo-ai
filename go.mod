module github.com/vegidio/open-photo-ai

go 1.25.3

require (
	github.com/cockroachdb/errors v1.12.0
	github.com/disintegration/imaging v1.6.2
	github.com/samber/lo v1.52.0
	github.com/vegidio/avif-go v0.0.0-20260201182506-481b88104109
	github.com/vegidio/go-sak v0.0.0-20260122173904-429e26e71cc8
	github.com/vegidio/heif-go v0.0.0-20251219210713-e14a78e55c84
	github.com/vegidio/webp-go v0.0.0-20251220093554-d304ec2dc4e6
	github.com/yalue/onnxruntime_go v1.21.0
	golang.org/x/image v0.35.0
	golang.org/x/text v0.33.0
)

require (
	github.com/cespare/xxhash/v2 v2.3.0 // indirect
	github.com/cockroachdb/logtags v0.0.0-20241215232642-bb51bb14a506 // indirect
	github.com/cockroachdb/redact v1.1.6 // indirect
	github.com/davecgh/go-spew v1.1.2-0.20180830191138-d8f796af33cc // indirect
	github.com/dgraph-io/badger/v4 v4.9.1 // indirect
	github.com/dgraph-io/ristretto/v2 v2.4.0 // indirect
	github.com/dustin/go-humanize v1.0.1 // indirect
	github.com/getsentry/sentry-go v0.42.0 // indirect
	github.com/go-logr/logr v1.4.3 // indirect
	github.com/go-logr/stdr v1.2.2 // indirect
	github.com/gogo/protobuf v1.3.2 // indirect
	github.com/google/flatbuffers v25.12.19+incompatible // indirect
	github.com/klauspost/compress v1.18.3 // indirect
	github.com/klauspost/cpuid/v2 v2.3.0 // indirect
	github.com/kr/pretty v0.3.1 // indirect
	github.com/kr/text v0.2.0 // indirect
	github.com/otiai10/copy v1.14.1 // indirect
	github.com/otiai10/mint v1.6.3 // indirect
	github.com/pkg/errors v0.9.1 // indirect
	github.com/pmezard/go-difflib v1.0.1-0.20181226105442-5d4384ee4fb2 // indirect
	github.com/rogpeppe/go-internal v1.14.1 // indirect
	github.com/ulikunitz/xz v0.5.15 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	go.opentelemetry.io/auto/sdk v1.2.1 // indirect
	go.opentelemetry.io/otel v1.40.0 // indirect
	go.opentelemetry.io/otel/metric v1.40.0 // indirect
	go.opentelemetry.io/otel/trace v1.40.0 // indirect
	golang.org/x/net v0.49.0 // indirect
	golang.org/x/sync v0.19.0 // indirect
	golang.org/x/sys v0.40.0 // indirect
	google.golang.org/protobuf v1.36.11 // indirect
)

// Need to keep this here because github.com/cockroachdb/errors is pulling a really old version of this lib
exclude google.golang.org/genproto v0.0.0-20230410155749-daa745c078e1