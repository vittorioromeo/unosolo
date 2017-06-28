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
    #[structopt(help = "input file")]
    input: String,

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

type PathSet = HashSet<PathBuf>;
type PathGraph = HashMap<PathBuf, PathSet>;

/// Returns `true` if `x` is a path to an header currently supported by `unosolo`.
fn is_header(x: &walkdir::DirEntry) -> bool {
    x.file_name().to_str().map_or(false, |s| {
        s.ends_with(".h") || s.ends_with(".hpp")
    })
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
    s.find("#include").map_or(false, |y| {
        s[0..y].chars().all(|c| c.is_whitespace())
    })
}

/// Step of the graph traversal, prints lines to the final single include.
fn walk_pg_impl(
    opt: &Opt,
    dependencies: &PathGraph,
    include_directive_lines: &HashSet<String>,
    key: &Path,
    depth: usize,
    visited: &mut PathSet,
) {
    dependencies.get(key).map(|vec| for dependency_path in vec {
        walk_pg_impl(
            opt,
            dependencies,
            include_directive_lines,
            dependency_path,
            depth + 1,
            visited,
        );
    });

    if !visited.contains(key) {
        verbose_eprintln!(opt, "{} {:?}", "\t".repeat(depth), key);
        visited.insert(key.to_owned());

        let f = File::open(key).unwrap();
        let f = BufReader::new(f);

        for line in f.lines().filter_map(|e| e.ok()).filter(|l| {
            !include_directive_lines.contains(l) && !is_comment(l) && !is_pragma_once(l)
        })
        {
            println!("{}", line);
        }

        println!("\n\n\n");
    }
}

/// Begins walking through the `dependencies` graph, starting from `top_include_path`.
fn walk_pg(
    opt: &Opt,
    dependencies: &PathGraph,
    include_directive_lines: &HashSet<String>,
    top_include_path: &Path,
) {
    let mut visited = PathSet::new();

    walk_pg_impl(
        opt,
        dependencies,
        include_directive_lines,
        top_include_path,
        0,
        &mut visited,
    )
}

/// Builds the dependency graph and include directives set by reading the file at `entry_path`.
fn fill_dependencies(
    opt: &Opt,
    dependencies: &mut PathGraph,
    include_directive_lines: &mut HashSet<String>,
    entry_path: &PathBuf,
    prefix: &str,
    parent: &str,
) {
    let f = File::open(entry_path).unwrap();
    let f = BufReader::new(f);

    for line in f.lines().filter_map(|e| e.ok()).filter(
        |x| is_include_directive(x),
    )
    {
        // Cut off `#include`.
        let filename = &line[9..];

        if filename.chars().nth(0).unwrap() == '"' {
            include_directive_lines.insert(line.clone());
            let fullpath = prefix.to_owned() + "/" + parent + "/" + unquote(filename);
            let path = std::path::Path::new(&fullpath);
            let cpath = path.canonicalize().unwrap();
            verbose_eprintln!(opt, "cpath: {:?}", cpath);

            dependencies
                .entry(entry_path.clone())
                .or_insert_with(PathSet::new)
                .insert(cpath.clone());

            verbose_eprintln!(
                opt,
                "putting {:?} inside {:?}",
                entry_path.clone(),
                cpath.clone()
            );
        }

        verbose_eprintln!(opt, "");
    }
}

/// Executes `f` for all header files in the user-specified search path.
fn for_all_headers<F>(opt: &Opt, mut f: F)
where
    F: FnMut(PathBuf, &Path, &str, &str) -> (),
{
    for prefix in &opt.paths {
        for entry in WalkDir::new(&prefix)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(is_header)
        {
            let entry_path = entry.path();
            let c_entry_path = entry_path.to_path_buf().canonicalize().unwrap();
            let at_library_root = entry_path.strip_prefix(&prefix).unwrap();
            let parent = at_library_root.parent().unwrap().to_str().unwrap();

            f(c_entry_path, at_library_root, prefix, parent)
        }
    }
}

/// Prints the final header file to `stdout`.
fn print_final_result(
    opt: &Opt,
    top_include_path: &Path,
    dependencies: &PathGraph,
    include_directive_lines: &HashSet<String>,
) {
    println!("// generated with `unosolo`");
    println!("https://github.com/SuperV1234/unosolo");
    println!("#pragma once");

    walk_pg(opt, dependencies, include_directive_lines, top_include_path);
}

fn main() {
    let opt = Opt::from_args();
    verbose_eprintln!(opt, "Options: {:?}", opt);

    let mut dependencies = PathGraph::new();
    let mut include_directive_lines = HashSet::new();

    for_all_headers(&opt, |c_entry_path, at_library_root, prefix, parent| {
        verbose_eprintln!(opt, "c_entry_path: {:?}", c_entry_path);
        verbose_eprintln!(opt, "at_library_root: {:?}", at_library_root);
        verbose_eprintln!(opt, "parent: {:?}", parent);
        verbose_eprintln!(opt, "\n");

        dependencies.entry(c_entry_path.clone()).or_insert_with(
            PathSet::new,
        );

        fill_dependencies(
            &opt,
            &mut dependencies,
            &mut include_directive_lines,
            &c_entry_path,
            prefix,
            parent,
        );
    });

    let top_include_path = PathBuf::from(&opt.top_include).canonicalize().unwrap();
    print_final_result(
        &opt,
        &top_include_path,
        &dependencies,
        &include_directive_lines,
    );
}
