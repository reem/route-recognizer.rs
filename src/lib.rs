#[cfg(test)] extern crate test;

use nfa::NFA;
use nfa::CharacterClass;
use std::collections::TreeMap;

pub mod nfa;

#[deriving(Clone)]
struct Metadata {
  statics: int,
  dynamics: int,
  stars: int,
  param_names: Vec<String>,
}

impl Metadata {
  pub fn new() -> Metadata {
    Metadata{ statics: 0, dynamics: 0, stars: 0, param_names: Vec::new() }
  }
}

impl Ord for Metadata {
  fn cmp(&self, other: &Metadata) -> Ordering {
    if self.stars > other.stars {
      Less
    } else if self.stars < other.stars {
      Greater
    } else if self.dynamics > other.dynamics {
      Less
    } else if self.dynamics < other.dynamics {
      Greater
    } else if self.statics > other.statics {
      Less
    } else if self.statics < other.statics {
      Greater
    } else {
      Equal
    }
  }
}

impl PartialOrd for Metadata {
  fn partial_cmp(&self, other: &Metadata) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl PartialEq for Metadata {
  fn eq(&self, other: &Metadata) -> bool {
    self.statics == other.statics && self.dynamics == other.dynamics && self.stars == other.stars
  }
}

impl Eq for Metadata {}

#[deriving(PartialEq, Clone, Show)]
pub struct Params {
  map: TreeMap<String, String>
}

impl Params {
  pub fn new() -> Params {
    Params{ map: TreeMap::new() }
  }

  pub fn insert(&mut self, key: String, value: String) {
    self.map.insert(key, value);
  }
}

impl Index<&'static str, String> for Params {
  fn index<'a>(&'a self, index: &&'static str) -> &'a String {
    match self.map.find(&index.to_string()) {
      None => fail!(format!("params[{}] did not exist", index)),
      Some(s) => s,
    }
  }
}

pub struct Match<T> {
  pub handler: T,
  pub params: Params
}

impl<T> Match<T> {
  pub fn new(handler: T, params: Params) -> Match<T> {
    Match{ handler: handler, params: params }
  }
}

#[deriving(Clone)]
pub struct Router<T> {
  nfa: NFA<Metadata>,
  handlers: TreeMap<uint, T>
}

impl<T> Router<T> {
  pub fn new() -> Router<T> {
    Router{ nfa: NFA::new(), handlers: TreeMap::new() }
  }

  pub fn add(&mut self, mut route: &str, dest: T) {
    if route.char_at(0) == '/' {
      route = route.slice_from(1);
    }

    let nfa = &mut self.nfa;
    let mut state = 0;
    let mut metadata = Metadata::new();

    for (i, segment) in route.split('/').enumerate() {
      if i > 0 { state = nfa.put(state, CharacterClass::valid_char('/')); }

      if segment.len() > 0 && segment.char_at(0) == ':' {
        state = process_dynamic_segment(nfa, state);
        metadata.dynamics += 1;
        metadata.param_names.push(segment.slice_from(1).to_string());
      } else if segment.len() > 0 && segment.char_at(0) == '*' {
        state = process_star_state(nfa, state);
        metadata.stars += 1;
        metadata.param_names.push(segment.slice_from(1).to_string());
      } else {
        state = process_static_segment(segment, nfa, state);
        metadata.statics += 1;
      }
    }

    nfa.acceptance(state);
    nfa.metadata(state, metadata);
    self.handlers.insert(state, dest);
  }

  pub fn recognize<'a>(&'a self, mut path: &str) -> Result<Match<&'a T>, String> {
    if path.char_at(0) == '/' {
      path = path.slice_from(1);
    }

    let nfa = &self.nfa;
    let result = nfa.process(path, |index| nfa.get(index).metadata.get_ref());

    match result {
      Ok(nfa_match) => {
        let mut map = Params::new();
        let state = &nfa.get(nfa_match.state);
        let metadata = state.metadata.get_ref();
        let param_names = metadata.param_names.clone();

        for (i, capture) in nfa_match.captures.iter().enumerate() {
          map.insert(param_names[i].to_string(), capture.to_string());
        }

        let handler = self.handlers.find(&nfa_match.state).unwrap();
        Ok(Match::new(handler, map))
      },
      Err(str) => Err(str)
    }
  }
}

fn process_static_segment<T>(segment: &str, nfa: &mut NFA<T>, mut state: uint) -> uint {
  for char in segment.chars() {
    state = nfa.put(state, CharacterClass::valid_char(char));
  }

  state
}

fn process_dynamic_segment<T>(nfa: &mut NFA<T>, mut state: uint) -> uint {
  state = nfa.put(state, CharacterClass::invalid_char('/'));
  nfa.put_state(state, state);
  nfa.start_capture(state);
  nfa.end_capture(state);

  state
}

fn process_star_state<T>(nfa: &mut NFA<T>, mut state: uint) -> uint {
  state = nfa.put(state, CharacterClass::any());
  nfa.put_state(state, state);
  nfa.start_capture(state);
  nfa.end_capture(state);

  state
}

#[test]
fn basic_router() {
  let mut router = Router::new();

  router.add("/thomas", "Thomas".to_string());
  router.add("/tom", "Tom".to_string());
  router.add("/wycats", "Yehuda".to_string());

  let m = router.recognize("/thomas").unwrap();

  assert_eq!(*m.handler, "Thomas".to_string());
  assert_eq!(m.params, Params::new());
}

#[test]
fn root_router() {
  let mut router = Router::new();
  router.add("/", 10i);
  assert_eq!(*router.recognize("/").unwrap().handler, 10)
}

#[test]
fn ambiguous_router() {
  let mut router = Router::new();

  router.add("/posts/new", "new".to_string());
  router.add("/posts/:id", "id".to_string());

  let id = router.recognize("/posts/1").unwrap();

  assert_eq!(*id.handler, "id".to_string());
  assert_eq!(id.params, params("id", "1"));

  let new = router.recognize("/posts/new").unwrap();
  assert_eq!(*new.handler, "new".to_string());
  assert_eq!(new.params, Params::new());
}


#[test]
fn ambiguous_router_b() {
  let mut router = Router::new();

  router.add("/posts/:id", "id".to_string());
  router.add("/posts/new", "new".to_string());

  let id = router.recognize("/posts/1").unwrap();

  assert_eq!(*id.handler, "id".to_string());
  assert_eq!(id.params, params("id", "1"));

  let new = router.recognize("/posts/new").unwrap();
  assert_eq!(*new.handler, "new".to_string());
  assert_eq!(new.params, Params::new());
}

#[test]
fn multiple_params() {
  let mut router = Router::new();

  router.add("/posts/:post_id/comments/:id", "comment".to_string());
  router.add("/posts/:post_id/comments", "comments".to_string());

  let com = router.recognize("/posts/12/comments/100").unwrap();
  let coms = router.recognize("/posts/12/comments").unwrap();

  assert_eq!(*com.handler, "comment".to_string());
  assert_eq!(com.params, two_params("post_id", "12", "id", "100"));

  assert_eq!(*coms.handler, "comments".to_string());
  assert_eq!(coms.params, params("post_id", "12"));
  assert_eq!(coms.params["post_id"], "12".to_string());
}

#[test]
fn star() {
  let mut router = Router::new();

  router.add("*foo", "test".to_string());
  router.add("/bar/*foo", "test2".to_string());

  let m = router.recognize("/test").unwrap();
  assert_eq!(*m.handler, "test".to_string());
  assert_eq!(m.params, params("foo", "test"));

  let m = router.recognize("/foo/bar").unwrap();
  assert_eq!(*m.handler, "test".to_string());
  assert_eq!(m.params, params("foo", "foo/bar"));

  let m = router.recognize("/bar/foo").unwrap();
  assert_eq!(*m.handler, "test".to_string());
  assert_eq!(m.params, params("foo", "bar/foo"));
}

#[bench]
fn benchmark(b: &mut test::Bencher) {
  let mut router = Router::new();
  router.add("/posts/:post_id/comments/:id", "comment".to_string());
  router.add("/posts/:post_id/comments", "comments".to_string());
  router.add("/posts/:post_id", "post".to_string());
  router.add("/posts", "posts".to_string());
  router.add("/comments", "comments2".to_string());
  router.add("/comments/:id", "comment2".to_string());

  b.iter(|| {
    router.recognize("/posts/100/comments/200")
  });
}

#[allow(dead_code)]
fn params(key: &str, val: &str) -> Params {
  let mut map = Params::new();
  map.insert(key.to_string(), val.to_string());
  map
}

#[allow(dead_code)]
fn two_params(k1: &str, v1: &str, k2: &str, v2: &str) -> Params {
  let mut map = Params::new();
  map.insert(k1.to_string(), v1.to_string());
  map.insert(k2.to_string(), v2.to_string());
  map
}
