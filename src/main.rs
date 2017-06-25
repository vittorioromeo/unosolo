extern crate walkdir;

extern crate structopt;

#[macro_use]
extern crate structopt_derive;

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
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(help = "Input file")]
    input: String,

    #[structopt(short = "f", long = "folders", help = "search folders")]
    folders: Vec<String>,

    #[structopt(short = "p", long = "path", help = "path", default_value = ".")]
    path: String,
}

trait HashMapExt<K, V, S> {
    fn put(&mut self, k: K, v: V);
}

impl<K, V, S> HashMapExt<K, V, S> for HashMap<K, Vec<V>, S>
where
    K: Eq + std::hash::Hash,
    S: std::hash::BuildHasher,
{
    fn put(&mut self, k: K, v: V) {
        if self.contains_key(&k) {
            self.get_mut(&k).unwrap().push(v);
        } else {
            self.insert(k, vec![v]);
        }
    }
}

fn is_header(x: &walkdir::DirEntry) -> bool {
    x.file_name()
        .to_str()
        .map(|s| s.ends_with(".h") || s.ends_with(".hpp"))
        .unwrap_or(false)
}

type PathGraph = HashMap<PathBuf, Vec<PathBuf>>;
type PathSet = HashSet<PathBuf>;

fn walk_pg_impl(
    pg: &PathGraph,
    key: &PathBuf,
    depth: usize,
    ps: &mut PathSet,
    target: &mut String,
) {
    pg.get(key).map(|v| for x in v {
        walk_pg_impl(pg, x, depth + 1, ps, target);
    });

    if !ps.contains(key) {
        println!("{} {:?}", "\t".repeat(depth), key);
        ps.insert(key.clone());

        let f = File::open(key).unwrap();
        let mut f = BufReader::new(f);

        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        *target += &buf;
        *target += "\n\n\n";
    }
}

fn walk_pg(pg: &PathGraph, key: &PathBuf, ps: &mut PathSet, target: &mut String) {
    walk_pg_impl(pg, key, 0, ps, target)
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);


    let mut dependencies = PathGraph::new();
    let mut dependents = PathGraph::new();

    for f in opt.folders {
        let prefix = opt.path.to_string() + "/" + &f;
        for entry in WalkDir::new(&prefix)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(is_header)
        {
            let entry_path = entry.path();
            let c_entry_path = entry_path.to_path_buf().canonicalize().unwrap();

            if !dependencies.contains_key(&c_entry_path) {
                dependencies.insert(c_entry_path.clone(), Vec::new());
            }

            if !dependents.contains_key(&c_entry_path) {
                dependents.insert(c_entry_path.clone(), Vec::new());
            }

            println!("entry_path: {:?}", entry_path);
            let parent = entry_path.strip_prefix(&prefix).unwrap();
            println!("parent: {:?}", parent);

            let currprefix = parent.parent().unwrap().to_str().unwrap();
            println!("currprefix: {:?}", currprefix);

            let f = File::open(entry_path).unwrap();
            let f = BufReader::new(f);

            for line in f.lines().filter_map(|e| e.ok()).filter(|x| {
                x.find("#include").map_or(false, |y| {
                    x[0..y].chars().all(|c| c.is_whitespace())
                })
            })
            {
                let filename = &line[9..];

                if filename.chars().nth(0).unwrap() == '"' {
                    let fullpath = prefix.clone() + "/" + currprefix + "/" +
                        &filename[1..filename.len() - 1];
                    let path = std::path::Path::new(&fullpath);
                    let cpath = path.canonicalize().unwrap();
                    println!("fullpath: {:?}", fullpath);
                    println!("path: {:?}", path);
                    println!("cpath: {:?}", cpath);

                    dependencies.put(c_entry_path.clone(), cpath.clone());

                    println!(
                        "putting {:?} inside {:?}",
                        c_entry_path.clone(),
                        cpath.clone()
                    );
                    if dependents.contains_key(&cpath) {
                        dependents.get_mut(&cpath).unwrap().push(
                            c_entry_path.clone(),
                        );
                    } else {
                        dependents.insert(cpath.clone(), vec![c_entry_path.clone()]);
                    }
                //dependents.put(cpath.clone(), c_entry_path.clone());
                } else {

                }
                println!("");
            }

            println!("");
            println!("");
        }
    }
    // println!("{:?}", dependencies);
    // println!("{:?}", dependents);
/*
    let without_dependencies = dependents.iter().filter(|&(k, v)| v.len() == 0);
    println!("{:?}", dependents);

    for (k, v) in without_dependencies {
        println!("{:?}", k);
        // walk_pg(&dependencies, &k, &mut ps);
    }
*/

    let mut ps = PathSet::new();
    let begin = PathBuf::from("/home/vittorioromeo/OHWorkspace/scelta/include/scelta.hpp");
    let mut res = String::new();
    walk_pg(&dependencies, &begin, &mut ps, &mut res);
    println!("{}", res);
}
