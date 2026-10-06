pub mod decoder;
pub mod output;
pub mod player;
pub mod resample;
pub mod source;

pub fn to_stereo(src: &[f32], channels: usize, out: &mut Vec<f32>) {
    match channels {
        0 => {}
        1 => out.extend(src.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(src),
        n => out.extend(src.chunks_exact(n).flat_map(|f| {
            let m = f.iter().sum::<f32>() / n as f32;
            [m, m]
        })),
    }
}
