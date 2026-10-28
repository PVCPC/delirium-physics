use skate_core::input::gesture::Pattern;
use std::path::Path;

pub fn load(path: &Path) -> Result<Vec<Pattern>, String> {
    parse(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse(source: &str) -> Result<Vec<Pattern>, String> {
    let mut patterns: Vec<Pattern> = Vec::new();
    let mut tolerance = None;
    for (line, source) in source.lines().enumerate() {
        let words: Vec<_> = source
            .split("//")
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if words.is_empty() {
            continue;
        }
        let fail = || format!("invalid PAT directive at line {}: {source}", line + 1);
        let number = |index: usize| -> Result<f32, String> {
            let value: f32 = words
                .get(index)
                .ok_or_else(fail)?
                .parse()
                .map_err(|_| fail())?;
            if !value.is_finite() {
                return Err(fail());
            }
            Ok(value)
        };
        match words[0] {
            "global_tolerance_dist" if words.len() == 2 => {
                let v = number(1)?;
                tolerance = Some(v * v);
            }
            "global_tolerance_time"
            | "global_tolerance_speed"
            | "global_anticipation_delay"
            | "tolerance_time"
                if words.len() == 2 =>
            {
                number(1)?;
            }
            "pattern" if words.len() == 2 => patterns.push(Pattern {
                name: words[1].into(),
                points: Vec::new(),
                tolerance_squared: tolerance
                    .ok_or_else(|| "PAT lacks global_tolerance_dist".to_owned())?,
            }),
            "tolerance_dist" if words.len() == 2 => {
                let v = number(1)?;
                patterns.last_mut().ok_or_else(fail)?.tolerance_squared = v * v;
            }
            "coord" if words.len() == 3 => {
                let point = [number(1)?, number(2)?];
                patterns.last_mut().ok_or_else(fail)?.points.push(point);
            }
            _ => return Err(fail()),
        }
    }
    skate_core::input::gesture::Recognizer::new(patterns.clone())?;
    Ok(patterns)
}

