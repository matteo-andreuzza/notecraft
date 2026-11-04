//! Dependency layering rules (`docs/architecture.md` › Layers).
//!
//! NoteCraft follows PhotoCraft's layer scheme (the brief's §5): L0 foundation, L1 `doc`, L2
//! text/ink/search/math, L3 render/gpu/format, L4 interop (`onestore`, `import-export`, …), L5
//! `engine`, L6 `ui-egui` and `automation`. A crate may depend only on strictly lower layers
//! (plus the explicit intra-layer chains below); nothing below L6 may use a UI toolkit.
//!
//! The rule engine works on a small, metadata-independent model so it can be unit-tested;
//! [`from_metadata`] builds that model from `cargo metadata`.

use serde_json::Value;

/// Where a workspace crate sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Regular layered crate.
    Layer(u8),
    /// Layer 0, and additionally may depend on no workspace crate at all.
    Standalone,
    /// Test tooling: may depend on anything up to L5 (it sits at L6 for rule purposes); other
    /// crates may use it only as a dev-dependency.
    Testkit,
    /// Binaries and build tooling: exempt from the rules.
    Exempt,
}

impl Class {
    fn layer(self) -> Option<u8> {
        match self {
            Class::Layer(l) => Some(l),
            Class::Standalone => Some(0),
            Class::Testkit => Some(6),
            Class::Exempt => None,
        }
    }
}

/// The layering table. Names are package names without the `notecraft-` prefix. Every new
/// crate must be registered here (an unregistered crate is a violation).
pub const TABLE: &[(&str, Class)] = &[
    // L0 foundation
    ("util", Class::Layer(0)),
    ("geom", Class::Layer(0)),
    ("color", Class::Layer(0)),
    ("ink-geom", Class::Layer(0)),
    ("fonts", Class::Layer(0)),
    ("codecs", Class::Standalone),
    // L1 document model
    ("doc", Class::Layer(1)),
    // L2 algorithms on the model
    ("text", Class::Layer(2)),
    ("ink", Class::Layer(2)),
    ("search", Class::Layer(2)),
    ("math", Class::Layer(2)),
    ("spell", Class::Layer(2)),
    // L3 rendering and the native format
    ("render", Class::Layer(3)),
    ("gpu", Class::Layer(3)),
    ("format", Class::Layer(3)),
    // L4 interop and extension
    ("onestore", Class::Layer(4)),
    ("import-export", Class::Layer(4)),
    ("ocr", Class::Layer(4)),
    ("plugins", Class::Layer(4)),
    ("sync", Class::Layer(4)),
    // L5 session + command registry
    ("engine", Class::Layer(5)),
    // L6 front ends
    ("ui-egui", Class::Layer(6)),
    ("automation", Class::Layer(6)),
    ("testkit", Class::Testkit),
    // apps and tooling
    ("notecraft", Class::Exempt),
    ("cli", Class::Exempt),
    ("web", Class::Exempt),
    ("xtask", Class::Exempt),
];

/// Explicit orderings *within* a layer (earlier may be used by later). The L0 foundation is a
/// small chain (`geom` uses `util`; `color` and `ink-geom` use `geom`); the GPU renderer reuses
/// the CPU reference renderer (`render`, the oracle) for parity tests.
pub const INTRA_LAYER_ORDER: &[&[&str]] = &[&["util", "geom", "color"], &["util", "geom", "ink-geom"], &["render", "gpu"]];

fn intra_layer_allowed(from: &str, to: &str) -> bool {
    let (from, to) = (short_name(from), short_name(to));
    INTRA_LAYER_ORDER.iter().any(|chain| match (chain.iter().position(|n| *n == from), chain.iter().position(|n| *n == to)) {
        (Some(f), Some(t)) => t < f,
        _ => false,
    })
}

/// External crates that constitute a UI toolkit / windowing dependency. Entries ending in `*`
/// are prefixes.
pub const UI_CRATES: &[&str] = &["egui", "eframe", "winit", "egui_kittest", "egui_extras", "rfd", "arboard", "bevy*"];

/// First layer allowed to use UI crates.
pub const UI_MIN_LAYER: u8 = 6;

/// The package name without the workspace prefix.
pub fn short_name(pkg: &str) -> &str {
    pkg.strip_prefix("notecraft-").unwrap_or(pkg)
}

/// The class of a workspace package, if registered.
pub fn classify(pkg: &str) -> Option<Class> {
    let s = short_name(pkg);
    TABLE.iter().find(|(n, _)| *n == s).map(|(_, c)| *c)
}

fn is_ui_crate(name: &str) -> bool {
    UI_CRATES.iter().any(|p| match p.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *p,
    })
}

/// Dependency kind as cargo reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

/// One dependency edge.
#[derive(Debug, Clone)]
pub struct Dep {
    pub name: String,
    pub kind: DepKind,
    /// `true` if the dependency is a workspace member.
    pub workspace: bool,
}

/// One workspace package and its dependencies.
#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub deps: Vec<Dep>,
}

/// A broken layering rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    Unregistered { krate: String },
    Upward { krate: String, dep: String, from: u8, to: u8, kind: DepKind },
    StandaloneHasWorkspaceDep { krate: String, dep: String },
    TestkitAsNormalDep { krate: String },
    UiBelowL6 { krate: String, dep: String, layer: u8 },
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Violation::Unregistered { krate } => {
                write!(f, "{krate}: unknown workspace crate; register it in xtask/src/layers.rs TABLE (see docs/architecture.md)")
            }
            Violation::Upward { krate, dep, from, to, kind } => {
                write!(f, "{krate} (L{from}) -> {dep} (L{to}) [{kind:?}]: may only depend on strictly lower layers")
            }
            Violation::StandaloneHasWorkspaceDep { krate, dep } => {
                write!(f, "{krate}: standalone crate must not depend on workspace crate {dep}")
            }
            Violation::TestkitAsNormalDep { krate } => {
                write!(f, "{krate}: notecraft-testkit may only be a dev-dependency")
            }
            Violation::UiBelowL6 { krate, dep, layer } => {
                write!(f, "{krate} (L{layer}) depends on UI crate `{dep}`; UI toolkits are only allowed in L6+")
            }
        }
    }
}

/// Check all rules. Returns violations sorted for stable output.
pub fn check(crates: &[Crate]) -> Vec<Violation> {
    let mut out = Vec::new();
    for c in crates {
        let Some(class) = classify(&c.name) else {
            out.push(Violation::Unregistered { krate: c.name.clone() });
            continue;
        };
        if class == Class::Exempt {
            continue;
        }
        let layer = class.layer().unwrap_or(0);
        for d in &c.deps {
            // Self dev-dependencies (e.g. to enable features in tests) are fine.
            if d.name == c.name {
                continue;
            }
            if d.workspace {
                if class == Class::Standalone {
                    out.push(Violation::StandaloneHasWorkspaceDep { krate: c.name.clone(), dep: d.name.clone() });
                    continue;
                }
                match classify(&d.name) {
                    Some(Class::Testkit) if d.kind != DepKind::Dev => {
                        out.push(Violation::TestkitAsNormalDep { krate: c.name.clone() });
                    }
                    // Unregistered deps are reported on their own entry; testkit dev deps are allowed.
                    None | Some(Class::Testkit) => {}
                    Some(dc) => {
                        let to = dc.layer().unwrap_or(u8::MAX);
                        if to >= layer && !(to == layer && intra_layer_allowed(&c.name, &d.name)) {
                            out.push(Violation::Upward {
                                krate: c.name.clone(),
                                dep: d.name.clone(),
                                from: layer,
                                to: if to == u8::MAX { 7 } else { to },
                                kind: d.kind,
                            });
                        }
                    }
                }
            } else if layer < UI_MIN_LAYER && is_ui_crate(&d.name) {
                out.push(Violation::UiBelowL6 { krate: c.name.clone(), dep: d.name.clone(), layer });
            }
        }
    }
    out.sort_by_key(|v| v.to_string());
    out.dedup();
    out
}

/// Build the model from `cargo metadata --format-version 1 --no-deps`.
pub fn from_metadata(meta: &Value) -> Result<Vec<Crate>, String> {
    let pkgs = meta["packages"].as_array().ok_or("metadata: no packages array")?;
    let members: Vec<&str> = pkgs.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut out = Vec::new();
    for p in pkgs {
        let name = p["name"].as_str().ok_or("package without name")?.to_owned();
        let mut deps = Vec::new();
        for d in p["dependencies"].as_array().into_iter().flatten() {
            let dname = d["name"].as_str().unwrap_or_default().to_owned();
            let kind = match d["kind"].as_str() {
                Some("dev") => DepKind::Dev,
                Some("build") => DepKind::Build,
                _ => DepKind::Normal,
            };
            let workspace = members.contains(&dname.as_str()) || d["path"].is_string();
            deps.push(Dep { name: dname, kind, workspace });
        }
        out.push(Crate { name, deps });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Human-readable layer label.
pub fn describe(class: Option<Class>) -> String {
    match class {
        Some(Class::Layer(l)) => format!("L{l}"),
        Some(Class::Standalone) => "L0 standalone".into(),
        Some(Class::Testkit) => "testkit".into(),
        Some(Class::Exempt) => "exempt".into(),
        None => "UNREGISTERED".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, deps: &[(&str, DepKind, bool)]) -> Crate {
        Crate { name: name.into(), deps: deps.iter().map(|(n, k, w)| Dep { name: (*n).into(), kind: *k, workspace: *w }).collect() }
    }
    use DepKind::*;

    #[test]
    fn clean_downward_graph_passes() {
        let g = [
            c("notecraft-geom", &[("serde", Normal, false)]),
            c("notecraft-doc", &[("notecraft-geom", Normal, true)]),
            c("notecraft-engine", &[("notecraft-doc", Normal, true), ("notecraft-testkit", Dev, true)]),
            c("notecraft-ui-egui", &[("notecraft-engine", Normal, true), ("egui", Normal, false)]),
            c("notecraft-cli", &[("notecraft-ui-egui", Normal, true)]),
        ];
        assert!(check(&g).is_empty(), "{:?}", check(&g));
    }

    #[test]
    fn upward_dependency_flagged() {
        let v = check(&[c("notecraft-doc", &[("notecraft-engine", Normal, true)])]);
        assert!(matches!(v[..], [Violation::Upward { from: 1, to: 5, .. }]));
    }

    #[test]
    fn sideways_dependency_flagged() {
        let v = check(&[c("notecraft-text", &[("notecraft-search", Normal, true)])]);
        assert!(matches!(v[..], [Violation::Upward { from: 2, to: 2, .. }]));
    }

    #[test]
    fn intra_layer_chains_allowed_one_way() {
        assert!(check(&[c("notecraft-color", &[("notecraft-geom", Normal, true)])]).is_empty());
        assert!(check(&[c("notecraft-gpu", &[("notecraft-render", Normal, true)])]).is_empty());
        let v = check(&[c("notecraft-geom", &[("notecraft-color", Normal, true)])]);
        assert!(matches!(v[..], [Violation::Upward { from: 0, to: 0, .. }]));
    }

    #[test]
    fn upward_dev_dependency_flagged() {
        let v = check(&[c("notecraft-geom", &[("notecraft-doc", Dev, true)])]);
        assert!(matches!(v[..], [Violation::Upward { kind: Dev, .. }]));
    }

    #[test]
    fn ui_crates_below_l6_flagged() {
        for dep in ["egui", "eframe", "winit", "egui_kittest", "rfd", "arboard", "bevy_ecs"] {
            let v = check(&[c("notecraft-engine", &[(dep, Normal, false)])]);
            assert!(matches!(v[..], [Violation::UiBelowL6 { layer: 5, .. }]), "{dep}");
        }
        assert!(check(&[c("notecraft-ui-egui", &[("egui", Normal, false)])]).is_empty());
    }

    #[test]
    fn unregistered_crate_is_error() {
        let v = check(&[c("notecraft-mystery", &[])]);
        assert!(matches!(&v[..], [Violation::Unregistered { krate }] if krate == "notecraft-mystery"));
        assert!(v[0].to_string().contains("register"));
    }

    #[test]
    fn standalone_has_no_workspace_deps() {
        let v = check(&[c("notecraft-codecs", &[("notecraft-geom", Normal, true)])]);
        assert!(matches!(v[..], [Violation::StandaloneHasWorkspaceDep { .. }]));
    }

    #[test]
    fn testkit_only_as_dev_dependency() {
        let v = check(&[c("notecraft-format", &[("notecraft-testkit", Normal, true)])]);
        assert!(matches!(v[..], [Violation::TestkitAsNormalDep { .. }]));
        assert!(check(&[c("notecraft-format", &[("notecraft-testkit", Dev, true)])]).is_empty());
    }

    #[test]
    fn apps_and_xtask_exempt() {
        for app in ["notecraft", "notecraft-cli", "notecraft-web", "xtask"] {
            assert!(check(&[c(app, &[("egui", Normal, false), ("notecraft-ui-egui", Normal, true)])]).is_empty());
        }
    }

    #[test]
    fn metadata_parsing() {
        let meta: Value = serde_json::from_str(
            r#"{"packages":[
                {"name":"notecraft-doc","dependencies":[
                    {"name":"notecraft-geom","kind":null,"path":"/x/crates/geom"},
                    {"name":"serde","kind":null},
                    {"name":"proptest","kind":"dev"}]},
                {"name":"notecraft-geom","dependencies":[]}
            ]}"#,
        )
        .unwrap();
        let g = from_metadata(&meta).unwrap();
        assert_eq!(g.len(), 2);
        let doc = g.iter().find(|c| c.name == "notecraft-doc").unwrap();
        assert!(doc.deps[0].workspace && !doc.deps[1].workspace);
        assert_eq!(doc.deps[2].kind, Dev);
        assert!(check(&g).is_empty());
    }
}
