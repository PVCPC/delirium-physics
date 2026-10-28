use super::{StoredBlock, Texture};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) fn decode(textures: &mut [Texture], blocks: &[StoredBlock<'_>], workers: usize) -> Result<usize, String> {
    let bytes = blocks.iter().fold(0usize, |sum, block| sum.saturating_add(block.expected));
    let workers = if bytes < 1024 * 1024 { 1 } else { workers.clamp(1, 8).min(blocks.len().max(1)) };
    if workers == 1 {
        for (texture, block) in textures.iter_mut().zip(blocks) { texture.rgba = block.decode()?; }
        return Ok(1);
    }
    let next = AtomicUsize::new(0);
    let decode_batch = || -> Result<Vec<(usize, Vec<u8>)>, String> {
        let mut decoded = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(block) = blocks.get(index) else { break; };
            decoded.push((index, block.decode().map_err(|e| format!("Texture {index}: {e}"))?));
        }
        Ok(decoded)
    };
    let decoded = std::thread::scope(|scope| -> Result<_, String> {
        let mut jobs = Vec::new();
        for _ in 1..workers {
            jobs.push(std::thread::Builder::new().name("map-texture-decode".into())
                .spawn_scoped(scope, &decode_batch).map_err(|e| format!("Texture worker: {e}"))?);
        }
        let local = decode_batch();
        let batches: Vec<_> = jobs.into_iter().map(|job| job.join()
            .unwrap_or_else(|_| Err("Texture decoder failed unexpectedly".into()))).collect();
        let mut decoded = local?;
        for batch in batches { decoded.extend(batch?); }
        Ok(decoded)
    })?;
    for (index, rgba) in decoded { textures[index].rgba = rgba; }
    Ok(workers)
}

