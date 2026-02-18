use crate::screenshot::ScreenshotProvider;
use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use image::{ImageBuffer, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep};

pub const DEFAULT_SCROLL_FRAME_INTERVAL: Duration = Duration::from_millis(180);
pub const DEFAULT_SCROLL_MAX_DURATION: Duration = Duration::from_secs(180);
pub const DEFAULT_SCROLL_MAX_FRAMES: usize = 900;

const MIN_OVERLAP_RATIO: f32 = 0.20;
const MAX_OVERLAP_RATIO: f32 = 0.95;
const MAX_ALIGNMENT_SCORE: f64 = 24.0;
const ALIGNMENT_STEP_PX: usize = 2;
const SAMPLE_STEP_X: usize = 4;
const SAMPLE_STEP_Y: usize = 3;
const MIN_NEW_ROWS: u32 = 24;
const DEFAULT_FALLBACK_NEW_ROWS_RATIO: f32 = 0.33;
const MAX_OUTPUT_PIXELS: u64 = 120_000_000; // ~480 MB RGBA
const SCORE_EPSILON: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollControlCommand {
    Stop,
}

#[derive(Debug, Clone)]
pub struct ScrollCaptureConfig {
    pub output_dir: PathBuf,
    pub filename_prefix: String,
    pub frame_interval: Duration,
    pub max_duration: Duration,
    pub max_frames: usize,
}

impl ScrollCaptureConfig {
    pub fn new(output_dir: PathBuf, filename_prefix: impl Into<String>) -> Self {
        Self {
            output_dir,
            filename_prefix: filename_prefix.into(),
            frame_interval: DEFAULT_SCROLL_FRAME_INTERVAL,
            max_duration: DEFAULT_SCROLL_MAX_DURATION,
            max_frames: DEFAULT_SCROLL_MAX_FRAMES,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScrollCaptureStats {
    pub raw_frames: usize,
    pub stitched_frames: usize,
    pub duplicate_frames: usize,
    pub fallback_alignments: usize,
    pub dimension_mismatches: usize,
    pub final_width: u32,
    pub final_height: u32,
}

#[derive(Debug, Clone)]
pub struct ScrollCaptureSummary {
    pub path: PathBuf,
    pub stats: ScrollCaptureStats,
    pub finished_by_limit: bool,
}

#[derive(Debug, Clone)]
pub enum ScrollCaptureEvent {
    Started,
    FrameCaptured { raw_frames: usize },
    Stitching { raw_frames: usize },
    Completed(ScrollCaptureSummary),
}

pub async fn run_manual_scroll_capture(
    screenshot_provider: Arc<dyn ScreenshotProvider>,
    config: ScrollCaptureConfig,
    mut control_rx: mpsc::UnboundedReceiver<ScrollControlCommand>,
    event_tx: Option<mpsc::UnboundedSender<ScrollCaptureEvent>>,
) -> Result<ScrollCaptureSummary> {
    fs::create_dir_all(&config.output_dir).with_context(|| {
        format!(
            "failed to create output directory {}",
            config.output_dir.display()
        )
    })?;

    let session_stamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
    let frames_dir = config.output_dir.join(format!(
        "{}-scroll-{}-frames",
        config.filename_prefix, session_stamp
    ));
    fs::create_dir_all(&frames_dir)
        .with_context(|| format!("failed to create frame directory {}", frames_dir.display()))?;

    send_event(&event_tx, ScrollCaptureEvent::Started);

    let started = Instant::now();
    let mut frame_paths: Vec<PathBuf> = Vec::new();
    let mut finished_by_limit = false;

    'capture_loop: loop {
        while let Ok(command) = control_rx.try_recv() {
            if matches!(command, ScrollControlCommand::Stop) {
                break 'capture_loop;
            }
        }

        if frame_paths.len() >= config.max_frames || started.elapsed() >= config.max_duration {
            finished_by_limit = true;
            break;
        }

        let frame_index = frame_paths.len() + 1;
        let frame_path = frames_dir.join(format!("frame-{frame_index:06}.png"));
        screenshot_provider
            .capture(&frame_path)
            .await
            .with_context(|| format!("failed to capture frame #{frame_index}"))?;
        frame_paths.push(frame_path);
        send_event(
            &event_tx,
            ScrollCaptureEvent::FrameCaptured {
                raw_frames: frame_paths.len(),
            },
        );

        tokio::select! {
            _ = sleep(config.frame_interval) => {}
            command = control_rx.recv() => {
                if command.is_none() || matches!(command, Some(ScrollControlCommand::Stop)) {
                    break;
                }
            }
        }
    }

    if frame_paths.is_empty() {
        bail!("scroll capture did not record any frames");
    }

    send_event(
        &event_tx,
        ScrollCaptureEvent::Stitching {
            raw_frames: frame_paths.len(),
        },
    );

    let output_path = config.output_dir.join(format!(
        "{}-scroll-{}.png",
        config.filename_prefix, session_stamp
    ));
    let stats = stitch_frames(&frame_paths, &output_path)?;
    let summary = ScrollCaptureSummary {
        path: output_path,
        stats,
        finished_by_limit,
    };

    let _ = fs::remove_dir_all(&frames_dir);

    send_event(&event_tx, ScrollCaptureEvent::Completed(summary.clone()));
    Ok(summary)
}

fn send_event(
    event_tx: &Option<mpsc::UnboundedSender<ScrollCaptureEvent>>,
    event: ScrollCaptureEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(event);
    }
}

fn stitch_frames(frame_paths: &[PathBuf], output_path: &Path) -> Result<ScrollCaptureStats> {
    let first_path = frame_paths
        .first()
        .ok_or_else(|| anyhow!("no frames available for stitching"))?;
    let first = read_rgba_image(first_path)?;
    let (width, height) = first.dimensions();
    if width == 0 || height == 0 {
        bail!("captured frame has invalid zero-sized dimensions");
    }

    ensure_pixel_budget(width, height)?;

    let row_bytes = width as usize * 4;
    let mut stitched_data = first.as_raw().clone();
    let mut stats = ScrollCaptureStats {
        raw_frames: frame_paths.len(),
        stitched_frames: 1,
        duplicate_frames: 0,
        fallback_alignments: 0,
        dimension_mismatches: 0,
        final_width: width,
        final_height: height,
    };

    let mut previous = first;
    let mut last_good_new_rows: Option<u32> = None;

    for path in frame_paths.iter().skip(1) {
        let current = read_rgba_image(path)?;
        let (current_width, current_height) = current.dimensions();
        if current_width != width || current_height != height {
            stats.dimension_mismatches += 1;
            previous = current;
            continue;
        }

        let alignment = estimate_alignment(&previous, &current, last_good_new_rows);
        if alignment.used_fallback {
            stats.fallback_alignments += 1;
        }

        let new_rows = height.saturating_sub(alignment.overlap);
        if new_rows < MIN_NEW_ROWS {
            stats.duplicate_frames += 1;
            previous = current;
            continue;
        }

        let new_total_height = stats.final_height.saturating_add(new_rows);
        ensure_pixel_budget(width, new_total_height)?;

        append_rows(&mut stitched_data, &current, alignment.overlap, row_bytes);
        stats.final_height = new_total_height;
        stats.stitched_frames += 1;
        last_good_new_rows = Some(new_rows);
        previous = current;
    }

    let stitched: RgbaImage = ImageBuffer::from_raw(width, stats.final_height, stitched_data)
        .ok_or_else(|| anyhow!("failed to construct stitched image buffer"))?;
    stitched
        .save(output_path)
        .with_context(|| format!("failed to write stitched image {}", output_path.display()))?;

    Ok(stats)
}

fn read_rgba_image(path: &Path) -> Result<RgbaImage> {
    image::open(path)
        .with_context(|| format!("failed to decode frame {}", path.display()))
        .map(|image| image.to_rgba8())
}

#[derive(Debug, Clone, Copy)]
struct Alignment {
    overlap: u32,
    score: f64,
    used_fallback: bool,
}

fn estimate_alignment(
    previous: &RgbaImage,
    current: &RgbaImage,
    last_good_new_rows: Option<u32>,
) -> Alignment {
    let height = previous.height();
    let min_overlap = ((height as f32 * MIN_OVERLAP_RATIO).round() as u32).clamp(1, height - 1);
    let max_overlap =
        ((height as f32 * MAX_OVERLAP_RATIO).round() as u32).clamp(min_overlap, height - 1);

    let mut best = Alignment {
        overlap: min_overlap,
        score: f64::MAX,
        used_fallback: false,
    };

    let mut overlap = min_overlap;
    while overlap <= max_overlap {
        let score = overlap_score(previous, current, overlap);
        if score + SCORE_EPSILON < best.score
            || ((score - best.score).abs() <= SCORE_EPSILON && overlap > best.overlap)
        {
            best = Alignment {
                overlap,
                score,
                used_fallback: false,
            };
        }
        overlap = overlap.saturating_add(ALIGNMENT_STEP_PX as u32);
    }

    if best.score <= MAX_ALIGNMENT_SCORE {
        return best;
    }

    let default_new_rows = ((height as f32) * DEFAULT_FALLBACK_NEW_ROWS_RATIO).round() as u32;
    let fallback_new_rows = last_good_new_rows
        .unwrap_or(default_new_rows)
        .clamp(MIN_NEW_ROWS, height.saturating_sub(1));

    Alignment {
        overlap: height.saturating_sub(fallback_new_rows),
        score: best.score,
        used_fallback: true,
    }
}

fn overlap_score(previous: &RgbaImage, current: &RgbaImage, overlap: u32) -> f64 {
    if overlap == 0 || overlap >= previous.height() {
        return f64::MAX;
    }

    let width = previous.width();
    let height = previous.height();

    let x_start = ((width as f32) * 0.2) as u32;
    let mut x_end = ((width as f32) * 0.8) as u32;
    if x_end <= x_start {
        x_end = width;
    }

    let top_ignore = ((height as f32) * 0.08) as u32;
    let bottom_ignore = ((height as f32) * 0.06) as u32;

    let y_start = top_ignore.min(overlap.saturating_sub(1));
    let mut y_end = overlap.saturating_sub(bottom_ignore);
    if y_end <= y_start {
        y_end = overlap;
    }
    if y_end <= y_start {
        return f64::MAX;
    }

    let mut sum = 0.0_f64;
    let mut samples = 0_u64;

    let mut y = y_start;
    while y < y_end {
        let previous_y = height - overlap + y;
        let current_y = y;
        let mut x = x_start;
        while x < x_end {
            let previous_pixel = previous.get_pixel(x, previous_y).0;
            let current_pixel = current.get_pixel(x, current_y).0;
            let previous_luma = luma(previous_pixel);
            let current_luma = luma(current_pixel);
            sum += (f64::from(previous_luma) - f64::from(current_luma)).abs();
            samples += 1;
            x = x.saturating_add(SAMPLE_STEP_X as u32);
        }
        y = y.saturating_add(SAMPLE_STEP_Y as u32);
    }

    if samples == 0 {
        f64::MAX
    } else {
        sum / samples as f64
    }
}

fn luma(pixel: [u8; 4]) -> u8 {
    // ITU-R BT.601 weighted luma approximation in integer math.
    let value = u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114;
    (value / 1000) as u8
}

fn append_rows(buffer: &mut Vec<u8>, image: &RgbaImage, start_row: u32, row_bytes: usize) {
    let width = image.width() as usize;
    for y in start_row..image.height() {
        let start = y as usize * width * 4;
        let end = start + row_bytes;
        buffer.extend_from_slice(&image.as_raw()[start..end]);
    }
}

fn ensure_pixel_budget(width: u32, height: u32) -> Result<()> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_OUTPUT_PIXELS {
        bail!(
            "scroll screenshot is too large ({}x{}, {:.1} MP). Try a shorter capture or slower scrolling.",
            width,
            height,
            pixels as f64 / 1_000_000.0
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::stitch_frames;
    use image::RgbaImage;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn stitches_aligned_scroll_frames() {
        let temp = tempdir().expect("tempdir");
        let canvas = make_gradient_canvas(140, 680);
        let viewport_height = 220;
        let offsets = [0, 90, 180, 270, 360, 460];

        let frame_paths = write_viewports(&canvas, viewport_height, &offsets, temp.path());
        let output_path = temp.path().join("stitched.png");

        let stats = stitch_frames(&frame_paths, &output_path).expect("stitch succeeds");
        assert_eq!(stats.raw_frames, offsets.len());
        assert_eq!(stats.duplicate_frames, 0);
        assert_eq!(stats.stitched_frames, offsets.len());
        assert_eq!(stats.final_width, canvas.width());
        assert_eq!(stats.final_height, canvas.height());

        let stitched = image::open(&output_path).expect("read stitched").to_rgba8();
        assert_eq!(stitched, canvas);
    }

    #[test]
    fn skips_duplicate_frames_with_no_new_rows() {
        let temp = tempdir().expect("tempdir");
        let frame_a = solid_frame(128, 220, [120, 140, 160, 255]);
        let frame_b = solid_frame(128, 220, [120, 140, 160, 255]);
        let frame_c = solid_frame(128, 220, [120, 140, 160, 255]);
        let frame_d = solid_frame(128, 220, [20, 40, 60, 255]);
        let frame_paths = vec![
            write_image(&frame_a, temp.path().join("a.png")),
            write_image(&frame_b, temp.path().join("b.png")),
            write_image(&frame_c, temp.path().join("c.png")),
            write_image(&frame_d, temp.path().join("d.png")),
        ];
        let output_path = temp.path().join("stitched.png");

        let stats = stitch_frames(&frame_paths, &output_path).expect("stitch succeeds");
        assert!(stats.duplicate_frames >= 2);
        assert!(stats.stitched_frames >= 2);
    }

    #[test]
    fn uses_fallback_when_frames_do_not_overlap() {
        let temp = tempdir().expect("tempdir");
        let frame_a = solid_frame(120, 200, [255, 10, 10, 255]);
        let frame_b = solid_frame(120, 200, [10, 255, 10, 255]);
        let frame_c = solid_frame(120, 200, [10, 10, 255, 255]);

        let paths = vec![
            write_image(&frame_a, temp.path().join("a.png")),
            write_image(&frame_b, temp.path().join("b.png")),
            write_image(&frame_c, temp.path().join("c.png")),
        ];
        let output_path = temp.path().join("stitched.png");

        let stats = stitch_frames(&paths, &output_path).expect("stitch succeeds");
        assert_eq!(stats.raw_frames, 3);
        assert!(stats.fallback_alignments >= 2);
        assert!(stats.final_height > frame_a.height());
    }

    fn make_gradient_canvas(width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let r = ((x * 13 + y * 3) % 255) as u8;
                let g = ((x * 5 + y * 11) % 255) as u8;
                let b = ((x * 7 + y * 17) % 255) as u8;
                image.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }
        image
    }

    fn write_viewports(
        canvas: &RgbaImage,
        viewport_height: u32,
        offsets: &[u32],
        dir: &Path,
    ) -> Vec<PathBuf> {
        offsets
            .iter()
            .enumerate()
            .map(|(index, offset)| {
                let viewport = viewport_from_canvas(canvas, *offset, viewport_height);
                write_image(&viewport, dir.join(format!("frame-{index:03}.png")))
            })
            .collect()
    }

    fn viewport_from_canvas(canvas: &RgbaImage, offset: u32, viewport_height: u32) -> RgbaImage {
        let mut out = RgbaImage::new(canvas.width(), viewport_height);
        for y in 0..viewport_height {
            for x in 0..canvas.width() {
                let pixel = canvas.get_pixel(x, offset + y);
                out.put_pixel(x, y, *pixel);
            }
        }
        out
    }

    fn solid_frame(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Rgba(rgba));
            }
        }
        image
    }

    fn write_image(image: &RgbaImage, path: PathBuf) -> PathBuf {
        image.save(&path).expect("write image");
        path
    }
}
