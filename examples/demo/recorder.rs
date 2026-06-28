//! Recording backend: the demo "camera".
//!
//! Drives the real [`Solitaire`] widget through Saudade's offscreen
//! [`MockBackend`] and records each step as one GIF frame. A frame is the
//! rendered ARGB buffer converted to RGBA; identical consecutive frames are
//! merged (their delays accumulate), and every frame after the first encodes
//! only the rectangle of pixels that actually changed, so static stretches cost
//! almost nothing.
//!
//! Set `SOLITAIRE_DEMO_FRAMES=<dir>` to also dump every frame as a PNG for
//! debugging a script.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use gif::{DisposalMethod, Encoder, Frame, Repeat};
use saudade::mock::MockBackend;
use saudade::{Event, Font};
use solitaire::game::{DrawMode, Game};
use solitaire::ui::Solitaire;

/// Cards still moving for this many frames are abandoned, so a runaway
/// animation can't make the recorder loop forever.
const MAX_CASCADE_FRAMES: usize = 160;

/// The GIF is captured at 1/this of the window's pixel size — a 2×2 box
/// average keeps the cards crisp while quartering the byte count.
const DOWNSCALE: usize = 2;

/// Ticks advanced per captured cascade frame. The cascade runs at 60 Hz; one
/// frame every two ticks halves the frame count without looking choppy.
const TICKS_PER_CASCADE_FRAME: usize = 2;

struct RecordedFrame {
    rgba: Vec<u8>,
    /// On-screen time in centiseconds (the GIF delay unit).
    delay: u32,
}

/// Records a scripted Solitaire session into an animated GIF.
pub struct Backend {
    widget: Solitaire,
    mock: MockBackend,
    /// Full window pixel size, before downscaling.
    full_width: usize,
    full_height: usize,
    /// Downscaled GIF frame size.
    width: u16,
    height: u16,
    frames: Vec<RecordedFrame>,
    dump_dir: Option<PathBuf>,
}

impl Backend {
    /// Start a session on the near-won demo board.
    pub fn start(width: i32, height: i32) -> Self {
        let mock = MockBackend::new(width, height)
            .with_scale(1.0)
            .with_sans_font(sans_font())
            .with_mono_font(mono_font());

        let dump_dir = std::env::var_os("SOLITAIRE_DEMO_FRAMES").map(PathBuf::from);
        if let Some(dir) = &dump_dir {
            fs::create_dir_all(dir).expect("create frame dump directory");
        }

        let full_width = width as usize;
        let full_height = height as usize;
        let mut backend = Self {
            widget: Solitaire::from_game(Game::demo_deal(DrawMode::One)),
            mock,
            full_width,
            full_height,
            width: (full_width / DOWNSCALE) as u16,
            height: (full_height / DOWNSCALE) as u16,
            frames: Vec::new(),
            dump_dir,
        };
        // Warm-up render so the menu bar's cached geometry is ready, then the
        // opening frame.
        let _ = backend.mock.render(&mut backend.widget);
        backend.capture(0);
        backend
    }

    /// Dispatch one input event through the real widget and capture the frame.
    pub fn input(&mut self, event: Event, delay: u32) {
        self.mock.dispatch(&mut self.widget, &event);
        self.capture(delay);
    }

    /// Extend the time the current frame stays on screen by `cs` centiseconds.
    pub fn hold(&mut self, cs: u32) {
        if let Some(last) = self.frames.last_mut() {
            last.delay += cs;
        }
    }

    /// Run the winning cascade to completion, capturing a frame every few ticks.
    pub fn celebrate(&mut self, delay: u32) {
        let mut guard = 0;
        while self.widget_wants_ticks() && guard < MAX_CASCADE_FRAMES {
            for _ in 0..TICKS_PER_CASCADE_FRAME {
                self.mock.dispatch(&mut self.widget, &Event::Tick);
            }
            self.capture(delay);
            guard += 1;
        }
    }

    fn widget_wants_ticks(&mut self) -> bool {
        use saudade::Widget;
        self.widget.wants_ticks()
    }

    /// Encode the captured frames into a GIF at `path`.
    pub fn finish(self, path: &Path) -> std::io::Result<()> {
        encode_gif(path, &self.frames, self.width, self.height)?;
        let seconds = self.frames.iter().map(|f| f.delay).sum::<u32>() as f32 / 100.0;
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        println!(
            "{}: {} frames, {seconds:.1}s, {}K",
            path.display(),
            self.frames.len(),
            bytes.div_ceil(1024),
        );
        Ok(())
    }

    /// Render the current widget state and append it as a frame, or — when
    /// nothing visible changed — extend the previous frame's delay instead.
    fn capture(&mut self, delay: u32) {
        let snap = self.mock.render(&mut self.widget);
        let rgba = downscale(snap.pixels(), self.full_width, self.full_height, DOWNSCALE);

        if let Some(last) = self.frames.last_mut()
            && last.rgba == rgba
        {
            last.delay += delay;
            return;
        }
        if let Some(dir) = &self.dump_dir {
            dump_png(dir, self.frames.len(), &rgba, self.width, self.height);
        }
        self.frames.push(RecordedFrame { rgba, delay });
    }
}

/// Average each `factor`×`factor` block of the ARGB buffer down to one opaque
/// RGBA pixel.
fn downscale(pixels: &[u32], full_w: usize, full_h: usize, factor: usize) -> Vec<u8> {
    let (ow, oh) = (full_w / factor, full_h / factor);
    let mut out = Vec::with_capacity(ow * oh * 4);
    let area = (factor * factor) as u32;
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for dy in 0..factor {
                for dx in 0..factor {
                    let px = pixels[(oy * factor + dy) * full_w + (ox * factor + dx)];
                    r += (px >> 16) & 0xFF;
                    g += (px >> 8) & 0xFF;
                    b += px & 0xFF;
                }
            }
            out.push((r / area) as u8);
            out.push((g / area) as u8);
            out.push((b / area) as u8);
            out.push(0xFF);
        }
    }
    out
}

fn sans_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../../tests/fonts/DejaVuSans.ttf").to_vec())
        .expect("bundled DejaVuSans.ttf failed to load")
}

fn mono_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../../tests/fonts/DejaVuSansMono.ttf").to_vec())
        .expect("bundled DejaVuSansMono.ttf failed to load")
}

fn dump_png(dir: &Path, index: usize, rgba: &[u8], width: u16, height: u16) {
    let path = dir.join(format!("frame-{index:04}.png"));
    let file = File::create(path).expect("create frame png");
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

// ----------------------------------------------------- GIF encoding internals
//
// Adapted from the pure-tui demo recorder: a lossless transparent-index path
// for frames with a small palette (the static board), falling back to
// quantization for the busy cascade frames; each non-first frame ships only its
// changed rectangle.

fn encode_gif(
    path: &Path,
    frames: &[RecordedFrame],
    width: u16,
    height: u16,
) -> std::io::Result<()> {
    if frames.is_empty() {
        return Err(std::io::Error::other("no frames recorded"));
    }
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = Encoder::new(file, width, height, &[]).map_err(std::io::Error::other)?;
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(std::io::Error::other)?;

    let mut previous: Option<&RecordedFrame> = None;
    for frame in frames {
        let region = match previous {
            None => Region {
                left: 0,
                top: 0,
                width,
                height,
            },
            Some(prev) => changed_region(&prev.rgba, &frame.rgba, width).unwrap_or(Region {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            }),
        };
        let previous_rgba = previous.map(|p| p.rgba.as_slice());
        encoder
            .write_frame(&gif_frame(
                &frame.rgba,
                previous_rgba,
                width,
                &region,
                frame.delay,
            ))
            .map_err(std::io::Error::other)?;
        previous = Some(frame);
    }
    Ok(())
}

/// A rectangle of pixels, in whole-frame coordinates.
struct Region {
    left: u16,
    top: u16,
    width: u16,
    height: u16,
}

/// The bounding box of all pixels that differ between two frames.
fn changed_region(a: &[u8], b: &[u8], width: u16) -> Option<Region> {
    let stride = width as usize * 4;
    let (mut top, mut bottom) = (None, 0);
    for (y, (row_a, row_b)) in a
        .chunks_exact(stride)
        .zip(b.chunks_exact(stride))
        .enumerate()
    {
        if row_a != row_b {
            top.get_or_insert(y);
            bottom = y;
        }
    }
    let top = top?;

    let (mut left, mut right) = (width as usize - 1, 0);
    for y in top..=bottom {
        let row_a = &a[y * stride..(y + 1) * stride];
        let row_b = &b[y * stride..(y + 1) * stride];
        for x in 0..width as usize {
            if row_a[x * 4..x * 4 + 4] != row_b[x * 4..x * 4 + 4] {
                left = left.min(x);
                right = right.max(x);
            }
        }
    }
    Some(Region {
        left: left as u16,
        top: top as u16,
        width: (right - left + 1) as u16,
        height: (bottom - top + 1) as u16,
    })
}

fn gif_frame(
    rgba: &[u8],
    previous: Option<&[u8]>,
    width: u16,
    region: &Region,
    delay: u32,
) -> Frame<'static> {
    let stride = width as usize * 4;
    let crop = |buffer: &[u8]| {
        let mut cropped = Vec::with_capacity(region.width as usize * region.height as usize * 4);
        for y in region.top..region.top + region.height {
            let start = y as usize * stride + region.left as usize * 4;
            cropped.extend_from_slice(&buffer[start..start + region.width as usize * 4]);
        }
        cropped
    };
    let mut cropped = crop(rgba);
    let cropped_previous = previous.map(crop);

    let mut frame = indexed_frame(&cropped, cropped_previous.as_deref(), region)
        .unwrap_or_else(|| Frame::from_rgba_speed(region.width, region.height, &mut cropped, 10));
    frame.left = region.left;
    frame.top = region.top;
    frame.delay = delay.min(u16::MAX as u32) as u16;
    frame.dispose = DisposalMethod::Keep;
    frame
}

/// Index the pixels against an exact palette, mapping pixels unchanged from the
/// previous frame to a shared transparent index. Lossless; returns `None` for
/// frames with more than 255 distinct colours (the caller then quantizes).
fn indexed_frame(rgba: &[u8], previous: Option<&[u8]>, region: &Region) -> Option<Frame<'static>> {
    const TRANSPARENT: u8 = 0;
    let mut lookup: HashMap<[u8; 3], u8> = HashMap::new();
    let mut palette = vec![0, 0, 0];
    let mut pixels = Vec::with_capacity(rgba.len() / 4);
    for (i, pixel) in rgba.chunks_exact(4).enumerate() {
        if let Some(previous) = previous
            && previous[i * 4..i * 4 + 4] == *pixel
        {
            pixels.push(TRANSPARENT);
            continue;
        }
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let index = match lookup.get(&rgb) {
            Some(&index) => index,
            None => {
                if lookup.len() == 255 {
                    return None;
                }
                let index = lookup.len() as u8 + 1;
                lookup.insert(rgb, index);
                palette.extend_from_slice(&rgb);
                index
            }
        };
        pixels.push(index);
    }
    Some(Frame {
        width: region.width,
        height: region.height,
        palette: Some(palette),
        transparent: previous.is_some().then_some(TRANSPARENT),
        buffer: pixels.into(),
        ..Frame::default()
    })
}
