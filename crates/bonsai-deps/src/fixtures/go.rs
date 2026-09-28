//! go.mod fixtures (E2 Go row).

pub(crate) const GO_MOD: &str = r#"// A comment line.
module example.com/app

go 1.22

toolchain go1.22.3

require (
	github.com/pkg/errors v0.9.1
	golang.org/x/text v0.14.0 // indirect
	corp.invalid/zzsentinel/mod v1.2.0
	example.com/replaced v1.0.0
	example.com/local v0.0.0-00010101000000-000000000000
	"example.com/quoted" v1.1.0
	github.com/pinned/only v1.0.0 // indirect; pinned
)

require github.com/single/line v2.0.0+incompatible

replace example.com/local => ./local

replace (
	example.com/replaced v1.0.0 => github.com/fork/replaced v1.0.1
	github.com/pinned/only v9.9.9 => ../never-applies
)

exclude golang.org/x/net v0.1.0

retract [v1.0.0, v1.0.5]
"#;
