use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CHAIN: usize = 16;
pub const MAX_MOVIES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    pub file: String,
    pub name: String,
    pub character_id: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub character_id: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovieDecl {
    #[serde(default)]
    pub imports: Vec<Import>,
    #[serde(default)]
    pub exports: Vec<Export>,
    #[serde(default)]
    pub characters: BTreeSet<i32>,
}
impl MovieDecl {
    pub fn from_json(json: &serde_json::Value) -> Result<Self, String> {
        let field = |name: &str| {
            json.get(name)
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]))
        };
        Ok(MovieDecl {
            imports: serde_json::from_value(field("imports")).map_err(|e| e.to_string())?,
            exports: serde_json::from_value(field("exports")).map_err(|e| e.to_string())?,
            characters: field("characters")
                .as_array()
                .ok_or("APT characters must be a list")?
                .iter()
                .filter_map(|c| c.as_i64().or_else(|| c["id"].as_i64()))
                .map(|i| i as i32)
                .collect(),
        })
    }
}

pub fn library_key(file: &str) -> String {
    let path = file.replace('\\', "/");
    let path = path.trim_start_matches('/');
    let path = path.strip_prefix("data/fe/").unwrap_or(path);
    path.strip_suffix(".apt").unwrap_or(path).to_string()
}

pub trait MovieSource {
    fn movie(&mut self, key: &str) -> Option<MovieDecl>;
}
impl MovieSource for BTreeMap<String, MovieDecl> {
    fn movie(&mut self, key: &str) -> Option<MovieDecl> {
        self.get(key).cloned()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OverrideTable {
    entries: BTreeMap<String, Vec<(String, MovieDecl)>>,
}
impl OverrideTable {
    pub fn set(&mut self, owner: &str, file: &str, movie: MovieDecl) {
        let list = self.entries.entry(library_key(file)).or_default();
        list.retain(|(o, _)| o != owner);
        list.push((owner.to_string(), movie));
    }
    pub fn clear_owner(&mut self, owner: &str) {
        for list in self.entries.values_mut() {
            list.retain(|(o, _)| o != owner);
        }
        self.entries.retain(|_, l| !l.is_empty());
    }
    pub fn position(&self, owner: &str, key: &str) -> Option<usize> {
        self.entries.get(key)?.iter().position(|(o, _)| o == owner)
    }
    pub fn get(&self, key: &str) -> Option<&MovieDecl> {
        self.entries.get(key).and_then(|l| l.last()).map(|(_, m)| m)
    }
    pub fn over<'a>(&'a self, base: &'a mut dyn MovieSource) -> Layered<'a> {
        Layered {
            overrides: self,
            base,
        }
    }
}
pub struct Layered<'a> {
    overrides: &'a OverrideTable,
    base: &'a mut dyn MovieSource,
}
impl MovieSource for Layered<'_> {
    fn movie(&mut self, key: &str) -> Option<MovieDecl> {
        self.overrides
            .get(key)
            .cloned()
            .or_else(|| self.base.movie(key))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub movie: String,
    pub character_id: i32,
    pub source: String,
    pub source_character: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    MissingMovie(String),
    MissingExport {
        movie: String,
        name: String,
    },
    UndefinedExport {
        movie: String,
        name: String,
    },
    Cycle,
    ChainLimit,
    MovieLimit,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    pub movie: String,
    pub character_id: i32,
    pub failure: Failure,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Resolution {
    pub bindings: Vec<Binding>,
    pub unresolved: Vec<Unresolved>,
    pub movies: BTreeMap<String, MovieDecl>,
}
impl Resolution {
    pub fn binding(&self, movie: &str, character_id: i32) -> Option<&Binding> {
        self.bindings
            .iter()
            .find(|b| b.movie == movie && b.character_id == character_id)
    }
}

struct Resolver<'a> {
    source: &'a mut dyn MovieSource,
    movies: BTreeMap<String, Option<MovieDecl>>,
}
impl Resolver<'_> {
    fn load(&mut self, key: &str) -> Result<Option<&MovieDecl>, Failure> {
        if !self.movies.contains_key(key) {
            if self.movies.len() >= MAX_MOVIES {
                return Err(Failure::MovieLimit);
            }
            let movie = self.source.movie(key);
            self.movies.insert(key.to_string(), movie);
        }
        Ok(self.movies[key].as_ref())
    }
    fn follow(&mut self, import: &Import) -> Result<(String, i32), Failure> {
        let mut seen = BTreeSet::new();
        let mut current = import.clone();
        for _ in 0..MAX_CHAIN {
            let key = library_key(&current.file);
            if !seen.insert((key.clone(), current.name.clone())) {
                return Err(Failure::Cycle);
            }
            let movie = self
                .load(&key)?
                .ok_or_else(|| Failure::MissingMovie(key.clone()))?;
            let export = movie
                .exports
                .iter()
                .find(|e| e.name == current.name)
                .ok_or_else(|| Failure::MissingExport {
                    movie: key.clone(),
                    name: current.name.clone(),
                })?;
            let id = export.character_id;
            if movie.characters.contains(&id) {
                return Ok((key, id));
            }
            current = movie
                .imports
                .iter()
                .find(|i| i.character_id == id)
                .cloned()
                .ok_or_else(|| Failure::UndefinedExport {
                    movie: key.clone(),
                    name: current.name.clone(),
                })?;
        }
        Err(Failure::ChainLimit)
    }
}

pub fn resolve(root: &str, root_movie: MovieDecl, source: &mut dyn MovieSource) -> Resolution {
    let root = library_key(root);
    let mut r = Resolver {
        source,
        movies: BTreeMap::new(),
    };
    r.movies.insert(root.clone(), Some(root_movie));
    let mut out = Resolution::default();
    let mut queue = vec![root];
    let mut done = BTreeSet::new();
    while let Some(key) = queue.pop() {
        if !done.insert(key.clone()) {
            continue;
        }
        let Some(Some(movie)) = r.movies.get(&key).cloned() else {
            continue;
        };
        for import in &movie.imports {
            match r.follow(import) {
                Ok((source, source_character)) => out.bindings.push(Binding {
                    movie: key.clone(),
                    character_id: import.character_id,
                    source,
                    source_character,
                }),
                Err(failure) => out.unresolved.push(Unresolved {
                    movie: key.clone(),
                    character_id: import.character_id,
                    failure,
                }),
            }
            let next = library_key(&import.file);
            if !done.contains(&next) && r.movies.get(&next).is_some_and(|m| m.is_some()) {
                queue.push(next);
            }
        }
        queue.sort();
        queue.dedup();
        queue.reverse();
    }
    out.bindings
        .sort_by(|a, b| (&a.movie, a.character_id).cmp(&(&b.movie, b.character_id)));
    out.unresolved
        .sort_by(|a, b| (&a.movie, a.character_id).cmp(&(&b.movie, b.character_id)));
    out.movies = r
        .movies
        .into_iter()
        .filter_map(|(k, m)| m.map(|m| (k, m)))
        .collect();
    out
}

