use std::path::Path;

use ely_gpui_component::{
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, TextSize},
};
use gpui::{App, FontWeight, IntoElement, ParentElement, Styled, div, relative};

use crate::term::MONO;

enum Mark {
    Label(&'static str),
    File(IconName),
}

/// Compact lettermarks and line icons share one size, weight and neutral palette.
pub(super) fn file(path: &Path, folder: bool, open: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let mark = if folder {
        Mark::File(if open {
            IconName::FolderOpen
        } else {
            IconName::Folder
        })
    } else {
        kind(path)
    };
    let art = match mark {
        Mark::Label(label) => div()
            .font_family(MONO)
            .font_weight(FontWeight::MEDIUM)
            .text_size(theme.text_size(TextSize::Xs))
            .line_height(relative(1.0))
            .text_color(theme.colors.fg_muted)
            .child(label)
            .into_any_element(),
        Mark::File(icon) => Icon::new(icon)
            .size(IconSize::Sm)
            .color(theme.colors.fg_muted)
            .into_any_element(),
    };
    div()
        .flex_none()
        .size(theme.icon_size(IconSize::Lg))
        .flex()
        .items_center()
        .justify_center()
        .child(art)
}

fn kind(path: &Path) -> Mark {
    use Mark::{File, Label};
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "go.mod" | "go.sum" | "go.work" | "go.work.sum" => return Label("GO"),
        "cargo.toml" | "cargo.lock" => return Label("RS"),
        "buf.yaml" | "buf.gen.yaml" | "buf.lock" => return Label("PB"),
        "openapi.yaml" | "openapi.yml" | "openapi.json" | "swagger.yaml" | "swagger.yml"
        | "swagger.json" => return Label("API"),
        "package.json"
        | "package-lock.json"
        | "pnpm-lock.yaml"
        | "pnpm-workspace.yaml"
        | "yarn.lock"
        | "bun.lock"
        | "bun.lockb"
        | ".npmrc"
        | ".yarnrc"
        | ".yarnrc.yml" => return Label("PKG"),
        "dockerfile"
        | "containerfile"
        | ".dockerignore"
        | "compose.yaml"
        | "compose.yml"
        | "docker-compose.yml"
        | "docker-compose.yaml" => return Label("DK"),
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitkeep" => {
            return Label("GIT");
        }
        "makefile" | "gnumakefile" => return Label("MK"),
        "cmakelists.txt" | "cmakecache.txt" => return Label("CM"),
        "build" | "build.bazel" | "workspace" | "workspace.bazel" | "module.bazel" => {
            return Label("BZ");
        }
        "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
            return Label("GR");
        }
        "pom.xml" => return Label("MVN"),
        "gemfile" | "gemfile.lock" | "rakefile" => return Label("RB"),
        "composer.json" | "composer.lock" => return Label("PHP"),
        "mix.exs" | "mix.lock" => return Label("EX"),
        "pubspec.yaml" | "pubspec.lock" => return Label("DT"),
        "jenkinsfile" => return Label("CI"),
        "justfile" | "taskfile.yml" | "taskfile.yaml" | ".editorconfig" | ".prettierrc"
        | ".eslintrc" | ".babelrc" => {
            return File(IconName::FileCog);
        }
        "pyproject.toml" | "requirements.txt" | "pipfile" | "pipfile.lock" | "uv.lock" => {
            return Label("PY");
        }
        _ => {}
    }
    if name.starts_with("dockerfile.")
        || name.starts_with("containerfile.")
        || matches!(extension.as_str(), "dockerfile" | "containerfile")
    {
        return Label("DK");
    }
    if name.ends_with(".tf.json")
        || name.ends_with(".tfvars.json")
        || name.ends_with(".tfstate.backup")
    {
        return Label("TF");
    }
    if name == ".env" || name.starts_with(".env.") || extension == "env" {
        return File(IconName::FileCog);
    }
    if name == "readme"
        || name.starts_with("readme.")
        || name == "license"
        || name.starts_with("license.")
    {
        return File(IconName::FileText);
    }
    match extension.as_str() {
        "thrift" => Label("TH"),
        "proto" | "textproto" | "pbtxt" | "prototxt" => Label("PB"),
        "graphql" | "gql" | "graphqls" => Label("GQL"),
        "avsc" | "avdl" => Label("AV"),
        "fbs" => Label("FB"),
        "capnp" => Label("CAP"),
        "idl" | "wsdl" => Label("IDL"),
        "prisma" => Label("PR"),
        "go" => Label("GO"),
        "ts" | "mts" | "cts" => Label("TS"),
        "tsx" => Label("TX"),
        "jsx" => Label("JX"),
        "js" | "mjs" | "cjs" => Label("JS"),
        "rs" => Label("RS"),
        "py" | "pyi" | "pyw" | "ipynb" => Label("PY"),
        "java" | "jar" | "class" => Label("JV"),
        "kt" | "kts" => Label("KT"),
        "swift" => Label("SW"),
        "m" | "mm" => Label("OC"),
        "c" | "h" => Label("C"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Label("C++"),
        "cu" | "cuh" => Label("CU"),
        "cs" | "csx" => Label("C#"),
        "rb" | "gemspec" => Label("RB"),
        "php" => Label("PHP"),
        "lua" => Label("LUA"),
        "dart" => Label("DT"),
        "scala" | "sc" | "sbt" => Label("SCL"),
        "groovy" | "gradle" => Label("GR"),
        "clj" | "cljs" | "cljc" | "edn" => Label("CLJ"),
        "ex" | "exs" => Label("EX"),
        "erl" | "hrl" => Label("ERL"),
        "hs" | "lhs" => Label("HS"),
        "ml" | "mli" => Label("ML"),
        "fs" | "fsx" | "fsi" => Label("F#"),
        "r" | "rmd" => Label("R"),
        "jl" => Label("JL"),
        "zig" | "zon" => Label("ZIG"),
        "pl" | "pm" => Label("PL"),
        "sol" => Label("SOL"),
        "asm" | "s" => Label("ASM"),
        "wasm" | "wat" => Label("WAS"),
        "tf" | "tfvars" | "tfstate" => Label("TF"),
        "hcl" => Label("HCL"),
        "nix" => Label("NIX"),
        "cmake" => Label("CM"),
        "mk" => Label("MK"),
        "bazel" | "bzl" => Label("BZ"),
        "vue" => Label("V"),
        "svelte" => Label("S"),
        "html" | "htm" => Label("<>"),
        "css" => Label("#"),
        "scss" | "sass" => Label("SC"),
        "less" => Label("LE"),
        "styl" => Label("ST"),
        "astro" => Label("AS"),
        "hbs" | "handlebars" | "mustache" | "ejs" | "pug" | "j2" | "jinja" | "jinja2" | "tmpl" => {
            Label("TPL")
        }
        "json" | "jsonc" | "json5" | "jsonl" | "ndjson" => Label("{}"),
        "yaml" | "yml" => Label("YML"),
        "toml" => Label("TML"),
        "md" | "mdx" | "markdown" => Label("MD"),
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "psm1" | "psd1" | "bat" | "cmd" => Label(">_"),
        "sql" => Label("SQL"),
        "db" | "sqlite" | "sqlite3" | "duckdb" => File(IconName::Database),
        "parquet" | "arrow" | "avro" => File(IconName::Database),
        "tex" | "bib" => Label("TEX"),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "heic" | "avif" | "bmp"
        | "tiff" | "tif" => File(IconName::FileImage),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => File(IconName::FileAudio),
        "mp4" | "mov" | "mkv" | "webm" | "avi" => File(IconName::FileVideoCamera),
        "zip" | "gz" | "bz2" | "xz" | "tar" | "tgz" | "7z" | "rar" | "zst" | "lz4" => {
            File(IconName::FileArchive)
        }
        "csv" | "tsv" | "xls" | "xlsx" | "ods" => File(IconName::FileSpreadsheet),
        "pdf" => File(IconName::FileText),
        "txt" | "rtf" | "doc" | "docx" | "odt" | "log" | "rst" | "adoc" | "org" => {
            File(IconName::FileText)
        }
        "xml" | "xsd" | "xsl" | "xslt" => File(IconName::FileCode),
        "ttf" | "otf" | "woff" | "woff2" => File(IconName::FileType),
        "ini" | "conf" | "cfg" | "properties" | "lock" | "plist" => File(IconName::FileCog),
        _ => File(IconName::File),
    }
}
