//! MSBuild project files, `Directory.*.props` and `packages.config`, read
//! through the bounded XML walker (`xml_events.rs`, `[HIGH-2]`).
//!
//! Items read (element local names compared ASCII-case-insensitively, as
//! MSBuild does): `PackageReference`, `PackageVersion` (central package
//! management), `GlobalPackageReference`, each with `Include` and a
//! `Version` / `VersionOverride` attribute or child element. `Update=`-only
//! items modify an existing item and add nothing. `ProjectReference`,
//! `PackageDownload` and `Reference` are never dependencies (E2). Only
//! `PackageId` / `AssemblyName` text is read besides (declared names).

use crate::xml_events::{walk, Elem, XmlError, XmlSink};

/// Which item an entry came from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemKind {
    PackageReference,
    PackageVersion,
    GlobalPackageReference,
    /// `packages.config` `<package>`.
    PackagesConfig,
}

pub(crate) struct Item {
    pub(crate) kind: ItemKind,
    pub(crate) id: String,
    pub(crate) version: Option<String>,
    /// `packages.config` `developmentDependency="true"`.
    pub(crate) dev: bool,
}

#[derive(Default)]
pub(crate) struct ProjectData {
    pub(crate) items: Vec<Item>,
    /// `<PackageId>` / `<AssemblyName>` texts.
    pub(crate) names: Vec<String>,
}

fn item_kind(name: &str) -> Option<ItemKind> {
    [
        ("PackageReference", ItemKind::PackageReference),
        ("PackageVersion", ItemKind::PackageVersion),
        ("GlobalPackageReference", ItemKind::GlobalPackageReference),
    ]
    .into_iter()
    .find(|(n, _)| n.eq_ignore_ascii_case(name))
    .map(|(_, k)| k)
}

fn is(name: &str, want: &str) -> bool {
    name.eq_ignore_ascii_case(want)
}

/// The item being read (between its start and end).
struct Pending {
    kind: ItemKind,
    ids: Vec<String>,
    version: Option<String>,
    version_override: Option<String>,
    depth: usize,
}

#[derive(Default)]
struct Sink {
    out: ProjectData,
    pending: Option<Pending>,
}

impl Sink {
    fn start_item(
        &mut self,
        kind: ItemKind,
        e: &Elem<'_, '_>,
        depth: usize,
    ) -> Result<(), XmlError> {
        let ids: Vec<String> = match e.attr("Include")? {
            Some(inc) => inc
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            // `Update=` / `Remove=`-only: modifies, adds nothing.
            None => Vec::new(),
        };
        self.pending = Some(Pending {
            kind,
            ids,
            version: e.attr("Version")?,
            version_override: e.attr("VersionOverride")?,
            depth,
        });
        Ok(())
    }

    fn finish_item(&mut self) {
        let Some(p) = self.pending.take() else {
            return;
        };
        let version = p.version_override.or(p.version);
        for id in p.ids {
            self.out.items.push(Item {
                kind: p.kind,
                id,
                version: version.clone(),
                dev: false,
            });
        }
    }
}

impl XmlSink for Sink {
    fn start(&mut self, stack: &[String], e: &Elem<'_, '_>) -> Result<(), XmlError> {
        let name = e.name();
        if self.pending.is_none() {
            if let Some(kind) = item_kind(name) {
                return self.start_item(kind, e, stack.len());
            }
            // packages.config: <packages><package id version …/></packages>
            if is(name, "package") && stack.len() == 1 && is(&stack[0], "packages") {
                if let Some(id) = e.attr("id")? {
                    let dev = e
                        .attr("developmentDependency")?
                        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
                    self.out.items.push(Item {
                        kind: ItemKind::PackagesConfig,
                        id: id.trim().to_string(),
                        version: e.attr("version")?,
                        dev,
                    });
                }
            }
        }
        Ok(())
    }

    fn wants_text(&self, stack: &[String]) -> bool {
        let Some(last) = stack.last() else {
            return false;
        };
        match &self.pending {
            // A direct `<Version>` / `<VersionOverride>` child of the item.
            Some(p) => {
                stack.len() == p.depth + 2 && (is(last, "Version") || is(last, "VersionOverride"))
            }
            None => is(last, "PackageId") || is(last, "AssemblyName"),
        }
    }

    fn text(&mut self, stack: &[String], text: &str) -> Result<(), XmlError> {
        let t = text.trim().to_string();
        let last = stack.last().map(String::as_str).unwrap_or("");
        match &mut self.pending {
            Some(p) if is(last, "Version") => p.version = Some(t),
            Some(p) if is(last, "VersionOverride") => p.version_override = Some(t),
            Some(_) => {}
            None if !t.is_empty() => self.out.names.push(t),
            None => {}
        }
        Ok(())
    }

    fn end(&mut self, stack: &[String]) -> Result<(), XmlError> {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| stack.len() == p.depth + 1)
        {
            self.finish_item();
        }
        Ok(())
    }
}

/// Reads one project / props / packages.config file.
pub(crate) fn read(text: &str) -> Result<ProjectData, XmlError> {
    let mut sink = Sink::default();
    walk(text, &mut sink)?;
    Ok(sink.out)
}
