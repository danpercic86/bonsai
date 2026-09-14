//! AC3 as a property of the WHOLE `tools` module: detection reads the machine,
//! it never executes a candidate.
//!
//! Its own file rather than a case inside `detect_tests.rs`, because the
//! property is module-wide — `detect.rs` alone passes the scan partly BECAUSE
//! the one process detection starts (`reg.exe`) is delegated to `gitbin`.

#[test]
fn detection_never_constructs_a_child_process_for_a_candidate() {
    // Scanned across the WHOLE module, not just `detect.rs`: that file passes
    // partly BECAUSE the one process detection ever starts (`reg.exe`, a reader
    // of the registry, never a candidate tool) is delegated to `gitbin` — so a
    // spawn added in `mod.rs`, `catalog.rs`, `catalog_table.rs`, `custom.rs` or
    // a new `HostToolEnv` helper would have gone unnoticed here. `fake.rs` and
    // the `*_tests.rs` files are excluded: they are not production code.
    //
    // A NEW production file in `tools/` must be added to this list:
    // `include_str!` takes a literal path and cannot read a directory.
    //
    // The trait-shape half of AC3 is structural rather than textual: `ToolEnv`
    // exposes no run/spawn method, and `FakeToolEnv` implements the trait — so
    // adding one fails test COMPILATION before this test can even run.
    let module = [
        ("mod.rs", include_str!("mod.rs")),
        ("catalog.rs", include_str!("catalog.rs")),
        ("catalog_table.rs", include_str!("catalog_table.rs")),
        ("custom.rs", include_str!("custom.rs")),
        ("detect.rs", include_str!("detect.rs")),
    ];
    for (name, src) in module {
        for forbidden in ["Command::new", "Stdio", "std::process"] {
            assert!(
                !src.contains(forbidden),
                "tools/{name} must not spawn: found `{forbidden}`"
            );
        }
    }
}
