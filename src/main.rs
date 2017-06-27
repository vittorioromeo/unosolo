extern crate walkdir;
extern crate structopt;
extern crate regex;

#[macro_use]
extern crate structopt_derive;

#[macro_use]
extern crate maplit;

#[macro_use]
extern crate lazy_static;

use walkdir::WalkDir;
use structopt::StructOpt;
use std::str::FromStr;
use std::io::BufReader;
use std::io::Read;
use std::io::BufRead;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(StructOpt, Debug)]
#[structopt(name = "unosolo",
            about = "Transforms a C++ header-only library in a self-contained single header.")]
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

macro_rules! verbose_eprintln {
    ($opt:expr, $($tts:tt)*) => {
        if $opt.verbose {
            eprintln!($($tts)*);
        }
    }
}

trait HashMapExt<K, C, S>
where
    C: SetLike,
{
    fn put(&mut self, k: K, v: C::Item);
    fn put_empty(&mut self, k: K);
}

trait SetLike {
    type Item;
    fn with_one_item(x: Self::Item) -> Self;
    fn add(&mut self, x: Self::Item);
    fn new() -> Self;
}

impl<T> SetLike for Vec<T> {
    type Item = T;
    fn with_one_item(x: Self::Item) -> Self {
        vec![x]
    }
    fn add(&mut self, x: Self::Item) {
        self.push(x)
    }
    fn new() -> Self {
        Vec::new()
    }
}

impl<T> SetLike for HashSet<T>
where
    T: Eq + std::hash::Hash,
{
    type Item = T;
    fn with_one_item(x: Self::Item) -> Self {
        hashset!{x}
    }
    fn add(&mut self, x: Self::Item) {
        self.insert(x);
    }
    fn new() -> Self {
        HashSet::new()
    }
}

impl<K, C, S> HashMapExt<K, C, S> for HashMap<K, C, S>
where
    C: SetLike,
    K: Eq + std::hash::Hash,
    S: std::hash::BuildHasher,
{
    fn put(&mut self, k: K, v: C::Item) {
        if self.contains_key(&k) {
            self.get_mut(&k).unwrap().add(v);
        } else {
            self.insert(k, C::with_one_item(v));
        }
    }

    fn put_empty(&mut self, k: K) {
        self.insert(k, C::new());
    }
}

fn is_header(x: &walkdir::DirEntry) -> bool {
    x.file_name()
        .to_str()
        .map(|s| s.ends_with(".h") || s.ends_with(".hpp"))
        .unwrap_or(false)
}

type PathSet = HashSet<PathBuf>;
type PathGraph = HashMap<PathBuf, PathSet>;

use regex::Regex;

fn is_comment(s: &str) -> bool {
    lazy_static!{
        static ref COMMENT_REGEX: Regex = Regex::new(r#"^[[:blank:]]*(//.*)|"(?:\\"|.)*?""#).unwrap();
    }

    COMMENT_REGEX.is_match(s)
}

fn is_pragma_once(s: &str) -> bool {
    lazy_static!{
        static ref COMMENT_REGEX: Regex = Regex::new(r#"[[:blank:]]*#pragma once.*"#).unwrap();
    }

    COMMENT_REGEX.is_match(s)
}

fn walk_pg_impl(
    opt: &Opt,
    dependencies: &PathGraph,
    include_directive_lines: &mut HashSet<String>,
    key: &PathBuf,
    depth: usize,
    ps: &mut PathSet,
    target: &mut String,
) {
    dependencies.get(key).map(|v| for x in v {
        walk_pg_impl(
            opt,
            dependencies,
            include_directive_lines,
            x,
            depth + 1,
            ps,
            target,
        );
    });

    if !ps.contains(key) {
        verbose_eprintln!(opt, "{} {:?}", "\t".repeat(depth), key);
        ps.insert(key.clone());

        let f = File::open(key).unwrap();
        let mut f = BufReader::new(f);

        let mut buf = String::new();
        for line in f.lines().filter_map(|e| e.ok()).filter(|l| {
            !include_directive_lines.contains(l) && !is_comment(l) && !is_pragma_once(l)
        })
        {
            *target += &line;
            *target += "\n";
        }


        //f.read_to_string(&mut buf).unwrap();
        *target += &buf;
        *target += "\n\n\n";
    }
}

fn walk_pg(
    opt: &Opt,
    dependencies: &PathGraph,
    include_directive_lines: &mut HashSet<String>,
    key: &PathBuf,
    ps: &mut PathSet,
    target: &mut String,
) {
    walk_pg_impl(
        opt,
        dependencies,
        include_directive_lines,
        key,
        0,
        ps,
        target,
    )
}

fn unquote(x: &str) -> &str {
    &x[1..x.len() - 1]
}

fn is_include_directive(x: &str) -> bool {
    x.find("#include").map_or(false, |y| {
        x[0..y].chars().all(|c| c.is_whitespace())
    })
}

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
        let filename = &line[9..];

        if filename.chars().nth(0).unwrap() == '"' {
            include_directive_lines.insert(line.clone());
            let fullpath = prefix.to_owned() + "/" + parent + "/" + unquote(filename);
            let path = std::path::Path::new(&fullpath);
            let cpath = path.canonicalize().unwrap();
            verbose_eprintln!(opt, "cpath: {:?}", cpath);

            dependencies.put(entry_path.clone(), cpath.clone());

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

        if !dependencies.contains_key(&c_entry_path) {
            dependencies.put_empty(c_entry_path.clone());
        }

        fill_dependencies(
            &opt,
            &mut dependencies,
            &mut include_directive_lines,
            &c_entry_path,
            &prefix,
            &parent,
        );
    });


    let mut ps = PathSet::new();
    let begin = PathBuf::from(&opt.top_include).canonicalize().unwrap();

    let mut res = String::new();
    res += "// generated with unosolo\n#pragma once\n";

    walk_pg(
        &opt,
        &mut dependencies,
        &mut include_directive_lines,
        &begin,
        &mut ps,
        &mut res,
    );
    println!("{}", res);
}
