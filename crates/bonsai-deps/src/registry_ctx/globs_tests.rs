use super::*;

#[test]
fn go_glob_prefix_semantics() {
    assert!(go_glob_matches(
        "corp.invalid",
        "corp.invalid/zzsentinel/mod"
    ));
    assert!(go_glob_matches("*.corp.invalid", "git.corp.invalid/a/b"));
    assert!(go_glob_matches("github.com/corp/*", "github.com/corp/x/v2"));
    assert!(go_glob_matches("Corp.Invalid", "corp.invalid/x"));
    assert!(!go_glob_matches("github.com/corp/*", "github.com/other/x"));
    assert!(!go_glob_matches("github.com/corp/x", "github.com/corp"));
    assert!(!go_glob_matches("corp.invalid", "notcorp.invalid/x"));
    assert!(!go_glob_matches("", "anything.com/x"));
}

#[test]
fn go_glob_wildcards() {
    assert!(go_glob_matches("git?.corp.invalid", "git1.corp.invalid/x"));
    assert!(go_glob_matches("a*b*c.io", "aXXbYYc.io/m"));
    assert!(!go_glob_matches("a*b*c.io", "aXXbYYd.io/m"));
    // Unsupported class syntax fails closed (treated as a match).
    assert!(go_glob_matches("[abc].io", "z.io/m"));
}

fn m(pattern: &str, public_only: bool) -> NugetMapping {
    NugetMapping {
        pattern: pattern.into(),
        public_only,
        host: (!public_only).then(|| "nuget.corp.invalid".to_string()),
    }
}

#[test]
fn nuget_most_specific_wins() {
    let maps = [
        m("*", true),
        m("contoso.*", false),
        m("contoso.public", true),
    ];
    let best = |id| nuget_best_mapping(&maps, id).map(|x| x.pattern.as_str());
    assert_eq!(best("Newtonsoft.Json"), Some("*"));
    assert_eq!(best("Contoso.Lib"), Some("contoso.*"));
    assert_eq!(best("CONTOSO.PUBLIC"), Some("contoso.public"));
    assert!(nuget_best_mapping(&[m("contoso.*", false)], "Other").is_none());
}
