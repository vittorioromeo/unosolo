// Copyright (c) 2017 Vittorio Romeo
// MIT License |  https://opensource.org/licenses/MIT
// http://vittorioromeo.info | vittorio.romeo@outlook.com

#[macro_use]
extern crate structopt_derive;

#[macro_use]
extern crate lazy_static;

extern crate walkdir;
extern crate structopt;
extern crate regex;

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
    #[structopt(short = "p", long = "paths", help = "paths", default_value = ".")]
    paths: Vec<String>,

    #[structopt(short = "v", long = "verbose", help = "verbose")]
    verbose: bool,

    #[structopt(short = "t", long = "topinclude", help = "top-level include")]
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

// Type aliases for the path graph.
type PathSet = HashSet<PathBuf>;
type PathGraph = HashMap<PathBuf, PathSet>;

/// Returns `true` if `x` is a path to an header currently supported by `unosolo`.
fn is_header(x: &walkdir::DirEntry) -> bool {
    x.file_name()
        .to_str()
        .map_or(false, |s| s.ends_with(".h") || s.ends_with(".hpp"))
}

/// Returns `true` if `s` is a line contaning only an inline C++ comment.
fn is_comment(s: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r#"^[[:blank:]]*(//.*)|"(?:\\"|.)*?""#).unwrap();
    }

    RE.is_match(s)
}

/// Returns `true` if `s` is a line containing only `#pragma once`.
fn is_pragma_once(s: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r#"[[:blank:]]*#pragma once.*"#).unwrap();
    }

    RE.is_match(s)
}

/// Returns `s` without the first and last character.
fn unquote(s: &str) -> &str {
    &s[1..s.len() - 1]
}

/// Returns `true` if `s` is a line containing only an `#include` directive.
fn is_include_directive(s: &str) -> bool {
    s.find("#include")
        .map_or(false, |y| s[0..y].chars().all(|c| c.is_whitespace()))
}

/// Step of the graph traversal, prints lines to the final single include.
fn walk_pg_impl(result: &mut String,
                opt: &Opt,
                dependencies: &PathGraph,
                include_directive_lines: &HashSet<String>,
                key: &Path,
                depth: usize,
                visited: &mut PathSet) {
    dependencies
        .get(key)
        .map(|vec| for dependency_path in vec {
                 walk_pg_impl(result,
                              opt,
                              dependencies,
                              include_directive_lines,
                              dependency_path,
                              depth + 1,
                              visited);
             });

    if !visited.contains(key) {
        verbose_eprintln!(opt, "{} {:?}", "\t".repeat(depth), key);
        visited.insert(key.to_owned());

        let f = expect_fmt!(File::open(key), "File {:?} doesn't exist", key);
        let f = BufReader::new(f);

        for line in f.lines()
                .filter_map(|e| e.ok())
                .filter(|l| {
                            !include_directive_lines.contains(l) && !is_comment(l) &&
                            !is_pragma_once(l)
                        }) {
            *result += &line;
            *result += "\n";
        }

        *result += "\n\n\n";
    }
}

/// Begins walking through the `dependencies` graph, starting from `top_include_path`.
fn walk_pg(result: &mut String,
           opt: &Opt,
           dependencies: &PathGraph,
           include_directive_lines: &HashSet<String>,
           top_include_path: &Path) {
    let mut visited = PathSet::new();

    walk_pg_impl(result,
                 opt,
                 dependencies,
                 include_directive_lines,
                 top_include_path,
                 0,
                 &mut visited)
}

/// Builds the dependency graph and include directives set by reading the file at `entry_path`.
fn fill_dependencies(opt: &Opt,
                     dependencies: &mut PathGraph,
                     include_directive_lines: &mut HashSet<String>,
                     absolute_includes: &HashMap<String, PathBuf>,
                     entry_path: &Path,
                     prefix: &str,
                     parent: &str) {
    let f = expect_fmt!(File::open(entry_path), "Could not open '{:?}'", entry_path);
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
                include_directive_lines.insert(line.clone());
                let fullpath = prefix.to_owned() + "/" + parent + "/" + unquoted;
                verbose_eprintln!(opt, "fullpath: {:?}", fullpath);
                let cpath = expect_fmt!(std::path::Path::new(&fullpath).canonicalize(),
                                        "Path {:?} does not exist",
                                        fullpath);
                verbose_eprintln!(opt, "cpath: {:?}", cpath);

                dependencies
                    .entry(entry_path.to_path_buf())
                    .or_insert_with(PathSet::new)
                    .insert(cpath.clone());

                verbose_eprintln!(opt, "putting {:?} inside {:?}", entry_path, cpath.clone());
            }
            IncludeType::Absolute => {
                verbose_eprintln!(opt, "found absolute include: {:?}", unquoted);

                if absolute_includes.contains_key(unquoted) {
                    include_directive_lines.insert(line.clone());
                    let fullpath = prefix.to_owned() + "/" + unquoted;
                    verbose_eprintln!(opt, "fullpath: {:?}", fullpath);
                    let cpath = expect_fmt!(std::path::Path::new(&fullpath).canonicalize(),
                                            "Path {:?} does not exist",
                                            fullpath);
                    verbose_eprintln!(opt, "cpath: {:?}", cpath);

                    verbose_eprintln!(opt, "found absolute include to SUBSTITUE {:?}", unquoted);
                    verbose_eprintln!(opt, "-> cpath: {:?}", cpath);

                    dependencies
                        .entry(entry_path.to_path_buf())
                        .or_insert_with(PathSet::new)
                        .insert(cpath.clone());
                }
            }
        }

        verbose_eprintln!(opt, "");
    }
}

/// Executes `f` for all header files in the user-specified search path.
fn for_all_headers<F>(opt: &Opt, mut f: F)
    where F: FnMut(&Path, &Path, &str, &str) -> ()
{
    for prefix in &opt.paths {
        let c_prefix = PathBuf::from(prefix).canonicalize().unwrap();

        for entry in WalkDir::new(&prefix)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_header) {
            let c_entry_path = entry.path().to_path_buf().canonicalize().unwrap();
            let at_library_root = c_entry_path.strip_prefix(&c_prefix).unwrap();
            let parent = at_library_root.parent().unwrap().to_str().unwrap();

            f(&c_entry_path,
              at_library_root,
              c_prefix.to_str().unwrap(),
              parent)
        }
    }
}

/// Prints the final header file to `stdout`.
fn produce_final_result(opt: &Opt,
                        top_include_path: &Path,
                        dependencies: &PathGraph,
                        include_directive_lines: &HashSet<String>)
                        -> String {
    let mut result = String::new();

    result += "// generated with `unosolo`\n";
    result += "https://github.com/SuperV1234/unosolo\n";
    result += "#pragma once\n\n";

    walk_pg(&mut result,
            opt,
            dependencies,
            include_directive_lines,
            top_include_path);

    result
}

fn produce_final_result_from_opt(opt: &Opt) -> String {
    let mut dependencies = PathGraph::new();
    let mut absolute_includes = HashMap::<String, PathBuf>::new();
    let mut include_directive_lines = HashSet::new();

    // Fill `absolute_includes` with "`<...>` -> absolute path".
    // TODO: extend to multiple libraries
    for_all_headers(opt, |c_entry_path, at_library_root, _, _| {
        absolute_includes
            .entry(at_library_root.to_str().unwrap().to_string())
            .or_insert_with(|| c_entry_path.to_path_buf());
    });

    // Create dependency graph between files.
    for_all_headers(opt, |c_entry_path, at_library_root, prefix, parent| {
        verbose_eprintln!(opt, "c_entry_path: {:?}", c_entry_path);
        verbose_eprintln!(opt, "at_library_root: {:?}", at_library_root);
        verbose_eprintln!(opt, "prefix: {:?}", prefix);
        verbose_eprintln!(opt, "parent: {:?}\n", parent);

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

    verbose_eprintln!(opt, "ABSOLUTE_INCLUDES: {:?}", absolute_includes);

    /*
    for (ak, av) in dependencies
            .iter()
            .filter(|&(k, v)| {
                        dependencies
                            .iter()
                            .filter(|&(k2, v2)| !v2.contains(k))
                            .count() == dependencies.len()
                    }) {
        verbose_eprintln!(opt, "NODEPS: {:?}", ak);
    }
    */

    // Traverse graph starting from "top include path" and return "final
    // single header" string.
    let top_include_path = PathBuf::from(&opt.top_include)
        .canonicalize()
        .expect("Top include path doesn't exist or is not a directory.");

    produce_final_result(opt,
                         &top_include_path,
                         &dependencies,
                         &include_directive_lines)
}

fn main() {
    let opt = Opt::from_args();
    verbose_eprintln!(opt, "Options: {:?}", opt);

    // Produce final single header and print to `stdout`.
    println!("{}", produce_final_result_from_opt(&opt));
}

#[test]
fn test0() {}

// TODO: allow multiple libraries to be specified (imagine vrm_core and vrm_cpp)
// TODO: automatically detect top-header includes
// TODO: rewrite?
