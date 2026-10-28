use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct TaskParams(pub HashMap<String, HashMap<String, HashMap<String, Vec<f32>>>>);

impl TaskParams {
    pub fn get(&self, task: &str, set: &str, name: &str, or: f32) -> f32 {
        let t = self.0.get(task);
        let find = |s: &str| t.and_then(|t| t.get(s)).and_then(|s| s.get(name)).and_then(|v| v.first().copied());
        find(set).or_else(|| find("*")).unwrap_or(or)
    }
}

pub fn read(text: &str) -> TaskParams {
    let mut out: HashMap<String, HashMap<String, HashMap<String, Vec<f32>>>> = HashMap::new();
    let mut at: Option<(String, String)> = None;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").replace(',', " ");
        let words: Vec<&str> = line.split_whitespace().collect();
        match (words.as_slice(), &at) {
            ([task, set, "{"], None) => at = Some((task.to_string(), set.to_string())),
            (["}"], Some(_)) => at = None,
            ([name, values @ ..], Some((task, set))) if !values.is_empty() => {
                let default = || out.get(task).and_then(|t| t.get("*")).and_then(|s| s.get(*name));
                let values: Vec<f32> = if *values == ["*"] { default().cloned().unwrap_or_default() } else { values.iter().filter_map(|v| match *v {
                    "*" => default().and_then(|d| d.first().copied()),
                    v if v.eq_ignore_ascii_case("true") => Some(1.0),
                    v if v.eq_ignore_ascii_case("false") => Some(0.0),
                    v => v.parse().ok(),
                }).collect() };
                out.entry(task.clone()).or_default().entry(set.clone()).or_default().insert(name.to_string(), values);
            }
            _ => {}
        }
    }
    TaskParams(out)
}

