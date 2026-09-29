//! The files plugins serve to browsers, and the bundles they are grouped in.
//!
//! A plugin embeds its static directory when it is compiled, TypeScript already turned into
//! JavaScript (`erp_assets_build`). Its files are then reachable as `<plugin>/static/<path>`, and
//! it says which of them — or of other plugins' — go in which bundle: `web.assets_backend` for the
//! back office, another for the site, another for the point of sale. Only installed plugins count,
//! in the order they were installed, so a bundle holds exactly what the installed plugins bring.

use std::collections::{HashMap, HashSet};

/// The files a plugin serves, relative to its static directory: what `erp_assets_build` embeds.
pub type StaticFiles = &'static [(&'static str, &'static [u8])];

/// What a plugin adds to one bundle: globs over public paths, such as `web/static/src/**/*.js`.
///
/// A plugin may name another plugin's files, to put them in a bundle that plugin did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleContribution {
    pub bundle: &'static str,
    pub globs: Vec<&'static str>,
}

impl BundleContribution {
    pub fn new(bundle: &'static str, globs: &[&'static str]) -> Self {
        BundleContribution {
            bundle,
            globs: globs.to_vec(),
        }
    }
}

struct Contribution {
    plugin: String,
    bundle: &'static str,
    globs: Vec<&'static str>,
}

/// Every file and bundle the installed plugins declared.
///
/// Held by the model manager, cleared before plugin libraries are unloaded: the contents are
/// slices of those libraries.
#[derive(Default)]
pub struct AssetRegistry {
    files: HashMap<String, &'static [u8]>,
    contributions: Vec<Contribution>,
}

impl AssetRegistry {
    /// Record what a plugin being installed serves, and what it adds to bundles.
    pub fn register(
        &mut self,
        plugin: &str,
        files: StaticFiles,
        contributions: Vec<BundleContribution>,
    ) {
        for (path, content) in files {
            self.files
                .insert(format!("{plugin}/static/{path}"), content);
        }
        for contribution in contributions {
            self.contributions.push(Contribution {
                plugin: plugin.to_string(),
                bundle: contribution.bundle,
                globs: contribution.globs,
            });
        }
    }

    /// A file by its public path, `<plugin>/static/<path>`.
    pub fn file(&self, path: &str) -> Option<&'static [u8]> {
        self.files.get(path.trim_start_matches('/')).copied()
    }

    /// The files of a bundle, in the order a browser must load them.
    ///
    /// Contributions in the order their plugins were installed, so a plugin's files come after
    /// those of the plugins it depends on; within one contribution, by path, so the order never
    /// depends on how the files were listed. A file matched twice keeps its first place.
    ///
    /// A glob matching nothing is logged: it is a mistyped path far more often than an intent.
    pub fn bundle(&self, name: &str) -> Vec<String> {
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        let mut seen = HashSet::new();
        let mut bundle = Vec::new();
        for contribution in self.contributions.iter().filter(|c| c.bundle == name) {
            for glob in &contribution.globs {
                let mut matched = false;
                for path in &paths {
                    if glob_match(glob, path) {
                        matched = true;
                        if seen.insert(path.as_str()) {
                            bundle.push((*path).clone());
                        }
                    }
                }
                if !matched {
                    tracing::warn!(
                        plugin = %contribution.plugin,
                        bundle = %name,
                        glob = %glob,
                        "A glob of a bundle matches no file"
                    );
                }
            }
        }
        bundle
    }

    /// Every bundle some installed plugin contributes to.
    pub fn bundles(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.contributions.iter().map(|c| c.bundle).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// The media type a browser needs to use a file, from its extension.
pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Whether a path matches a glob: `?` is one character, `*` any run within one segment, and `**`
/// as a whole segment any number of segments, none included.
pub fn glob_match(glob: &str, path: &str) -> bool {
    let glob: Vec<&str> = glob.trim_start_matches('/').split('/').collect();
    let path: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match_segments(&glob, &path)
}

fn match_segments(glob: &[&str], path: &[&str]) -> bool {
    match glob.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| match_segments(rest, &path[skip..])),
        Some((segment, rest)) => match path.split_first() {
            Some((part, others)) => {
                let segment: Vec<char> = segment.chars().collect();
                let part: Vec<char> = part.chars().collect();
                match_segment(&segment, &part) && match_segments(rest, others)
            }
            None => false,
        },
    }
}

fn match_segment(glob: &[char], text: &[char]) -> bool {
    match glob.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| match_segment(rest, &text[skip..])),
        Some(('?', rest)) => !text.is_empty() && match_segment(rest, &text[1..]),
        Some((wanted, rest)) => text.first() == Some(wanted) && match_segment(rest, &text[1..]),
    }
}
