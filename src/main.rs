// Copyright (c) 2017 Vittorio Romeo
// MIT License |  https://opensource.org/licenses/MIT
// http://vittorioromeo.info | vittorio.romeo@outlook.com

// TODO:
#![allow(dead_code)]

#[macro_use]
extern crate structopt_derive;

#[macro_use]
extern crate lazy_static;

extern crate regex;
extern crate structopt;
extern crate walkdir;

use walkdir::WalkDir;
use structopt::StructOpt;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;
use std::collections::HashSet;
use regex::Regex;

#[derive(StructOpt, Debug)]
#[structopt(name = "unosolo",
            about = "transforms a C++ header-only library in a self-contained single header.")]
struct Opt {
    #[structopt(short = "p",
                long = "paths",
                help = "Include paths",
                default_value = ".")]
    paths: Vec<String>,

    #[structopt(short = "v",
                long = "verbose",
                help = "Enable verbose mode")]
    verbose: bool,

    #[structopt(short = "t",
                long = "topinclude",
                help = "Top-level include file path (entrypoint)")]
    top_include: String,
}

/// Prints to `stderr` only if verbose mode is enabled.
macro_rules! verbose_eprintln {
    ($opt:expr, $($tts:tt)*) => {
        if $opt.verbose {
            eprintln!($($tts)*);
        }
    }
}

/// Attempts to unwrap `x`, otherwise panics with formatted string.
macro_rules! expect_fmt {
    ($x:expr, $($tts:tt)*) => {
        $x.unwrap_or_else(|_| panic!($($tts)*))
    }
}

/// Generates a function called `fn_name` that takes a `s: &str` and
// returns `true` if `s` matches `regex_string`.
macro_rules! regex_matcher {
    ($fn_name:ident, $regex_string:expr) => {
        fn $fn_name(s: &str) -> bool {
            lazy_static! {
                static ref RE: Regex =
                    Regex::new($regex_string).unwrap();
            }

            RE.is_match(s)
        }
    }
}

// Type aliases for the path graph.
type PathSet = HashSet<PathBuf>;
type PathGraph = HashMap<PathBuf, PathSet>;

/// Returns `true` if `x` is a path to an header currently supported by `unosolo`.
fn is_header(x: &walkdir::DirEntry) -> bool {
    x.file_name()
        .to_str()
        .map_or(false, |s| {
            s.ends_with(".h") || s.ends_with(".hpp") || s.ends_with(".inl") || s.ends_with(".cpp")
        })
}

/// Returns `true` if `s` is a line contaning only an inline C++ comment.
regex_matcher!(is_comment, r#"^[[:blank:]]*//.*"#);

/// Returns `true` if `s` is a line containing only `#pragma once`.
regex_matcher!(is_pragma_once, r#"[[:blank:]]*#pragma once.*"#);

/// Returns `s` without the first and last character.
fn unquote(s: &str) -> &str {
    &s[1..s.len() - 1]
}

/// Returns `true` if `s` is a line containing only an `#include` directive.
fn is_include_directive(s: &str) -> bool {
    s.find("#include")
        .map_or(false, |y| s[0..y].chars().all(|c| c.is_whitespace()))
}

fn unwrap_canonicalize(x: &str) -> std::path::PathBuf {
    expect_fmt!(std::path::Path::new(&x).canonicalize(),
                "Path {:#?} does not exist",
                x)
}

/// Builds the dependency graph and include directives set by reading the file at `entry_path`.
fn fill_dependencies(opt: &Opt,
                     dependencies: &mut PathGraph,
                     include_directive_lines: &mut HashMap<String, PathBuf>,
                     absolute_includes: &HashMap<String, PathBuf>,
                     entry_path: &Path,
                     prefix: &str,
                     parent: &str) {
    let f = expect_fmt!(File::open(entry_path), "Could not open '{:#?}'", entry_path);
    let f = BufReader::new(f);

    for line in f.lines()
            .filter_map(|e| e.ok())
            .filter(|x| is_include_directive(x)) {
        // Cut off `#include`.
        let filename = &line[9..];

        enum IncludeType {
            Relative,
            Absolute,
        };

        let include_type = match filename
                  .chars()
                  .nth(0)
                  .expect("Invalid include directive found") {
            '"' => IncludeType::Relative,
            '<' => IncludeType::Absolute,
            _ => panic!("Invalid include directive found"),
        };

        let unquoted = unquote(filename);
        match include_type {
                IncludeType::Relative => {
                    Some(unwrap_canonicalize(&format!("{}/{}/{}", prefix, parent, unquoted)))
                }
                IncludeType::Absolute => {
                    absolute_includes
                        .get(unquoted)
                        .map_or(None, |x| Some(x.to_path_buf()))
                }
            }
            .map(|cpath| {
                     dependencies
                         .entry(entry_path.to_path_buf())
                         .or_insert_with(PathSet::new)
                         .insert(cpath.clone());

                     include_directive_lines.insert(line.clone(), cpath);
                 });
    }
}

/// Executes `f` for all header files in the user-specified search path.
fn for_all_headers<F>(opt: &Opt, mut f: F)
    where F: FnMut(&Path, &Path, &str, &str) -> ()
{
    for prefix in &opt.paths {
        let c_prefix = unwrap_canonicalize(prefix);
        let c_prefix_str = c_prefix.to_str().unwrap();

        for entry in WalkDir::new(&prefix)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_header) {
            let c_entry_path = entry.path().canonicalize().unwrap();
            let at_library_root = c_entry_path.strip_prefix(&c_prefix).unwrap();
            let parent = at_library_root
                .parent()
                .and_then(|x| x.to_str())
                .unwrap();

            f(&c_entry_path, at_library_root, c_prefix_str, parent)
        }
    }
}

// TODO: docs
fn expand(opt: &Opt,
          result: &mut String,
          dependencies: &PathGraph,
          include_directive_lines: &HashMap<String, std::path::PathBuf>,
          visited: &mut PathSet,
          path: &Path) {
    verbose_eprintln!(opt, "expanding {:#?}", path);

    let f = expect_fmt!(File::open(path), "File {:#?} doesn't exist", path);
    let f = BufReader::new(f);

    for line in f.lines()
            .filter_map(|e| e.ok())
            .filter(|l| !is_comment(l) && !is_pragma_once(l)) {
        if include_directive_lines.contains_key(&line) {
            let cpath = include_directive_lines.get(&line).unwrap();
            if !visited.contains(cpath) {
                visited.insert(cpath.clone());
                &expand(opt,
                        result,
                        dependencies,
                        include_directive_lines,
                        visited,
                        cpath);
            }
        } else {
            *result += &line;
        }

        *result += "\n";
    }
}

/// Prints the final header file to `stdout`.
fn produce_final_result(opt: &Opt,
                        top_include_path: &Path,
                        dependencies: &PathGraph,
                        include_directive_lines: &HashMap<String, std::path::PathBuf>)
                        -> String {
    let mut result = String::new();
    result.reserve(1024 * 24); // TODO: calculate from source files

    result += "// generated with `unosolo`\n";
    result += "// https://github.com/SuperV1234/unosolo\n";
    result += "#pragma once\n\n";

    let mut visited = PathSet::new();
    &expand(opt,
            &mut result,
            dependencies,
            include_directive_lines,
            &mut visited,
            top_include_path);

    result
}

fn produce_final_result_from_opt(opt: &Opt) -> String {
    let mut dependencies = PathGraph::new();
    let mut absolute_includes = HashMap::<String, PathBuf>::new();
    let mut include_directive_lines = HashMap::<String, PathBuf>::new();

    // Fill `absolute_includes` with "`<...>` -> absolute path".
    for_all_headers(opt, |c_entry_path, at_library_root, _, _| {
        absolute_includes
            .entry(at_library_root.to_str().unwrap().to_string())
            .or_insert_with(|| c_entry_path.to_path_buf());
    });

    // Create dependency graph between files.
    for_all_headers(opt, |c_entry_path, at_library_root, prefix, parent| {
        verbose_eprintln!(opt, "c_entry_path: {:#?}", c_entry_path);
        verbose_eprintln!(opt, "at_library_root: {:#?}", at_library_root);
        verbose_eprintln!(opt, "prefix: {:#?}", prefix);
        verbose_eprintln!(opt, "parent: {:#?}\n", parent);

        dependencies
            .entry(c_entry_path.to_path_buf())
            .or_insert_with(PathSet::new);

        fill_dependencies(opt,
                          &mut dependencies,
                          &mut include_directive_lines,
                          &absolute_includes,
                          c_entry_path,
                          prefix,
                          parent);
    });

    verbose_eprintln!(opt, "ABSOLUTE_INCLUDES: {:#?}", absolute_includes);
    verbose_eprintln!(opt, "DEPENDENCIES: {:#?}", dependencies);
    verbose_eprintln!(opt, "ICD: {:#?}", include_directive_lines);

    // Traverse graph starting from "top include path" and return "final
    // single header" string.
    let top_include_path = unwrap_canonicalize(&opt.top_include);

    produce_final_result(opt,
                         &top_include_path,
                         &dependencies,
                         &include_directive_lines)
}

fn main() {
    let opt = Opt::from_args();
    verbose_eprintln!(opt, "Options: {:#?}", opt);

    // Produce final single header and print to `stdout`.
    println!("{}", produce_final_result_from_opt(&opt));
}
