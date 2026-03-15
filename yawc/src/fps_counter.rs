use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Frame, ImportAll, ImportMem, Renderer, Texture,
            element::{Element, Id, RenderElement},
            gles::{GlesRenderer, GlesTexture},
            utils::CommitCounter,
        },
    },
    utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform},
};

pub static FPS_NUMBERS_PNG: &[u8] = include_bytes!("../../resources/numbers.png");

pub fn fps_glestexture(renderer: &mut GlesRenderer) -> GlesTexture {
    use png::{Decoder, Transformations};
    use std::io::Cursor;

    let mut decoder = Decoder::new(Cursor::new(FPS_NUMBERS_PNG));
    decoder.set_transformations(Transformations::EXPAND);

    let mut reader = decoder.read_info().unwrap();

    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();

    let pixels = &buf[..info.buffer_size()];

    renderer
        .import_memory(
            pixels,
            Fourcc::Abgr8888,
            (info.width as i32, info.height as i32).into(),
            false,
        )
        .expect("Unable to upload FPS texture")
}

#[derive(Debug, Clone)]
pub struct FpsElement<T: Texture> {
    id: Id,
    value: u32,
    texture: T,
    commit_counter: CommitCounter,
}

impl<T: Texture> FpsElement<T> {
    pub fn new(texture: T) -> Self {
        FpsElement {
            id: Id::new(),
            texture,
            value: 0,
            commit_counter: CommitCounter::default(),
        }
    }

    pub fn update_fps(&mut self, fps: u32) {
        if self.value != fps {
            self.value = fps;
            self.commit_counter.increment();
        }
    }
}

impl<T> Element for FpsElement<T>
where
    T: Texture + 'static,
{
    fn id(&self) -> &Id {
        &self.id
    }

    fn location(&self, _scale: Scale<f64>) -> Point<i32, Physical> {
        (0, 0).into()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        let digits = if self.value < 10 {
            1
        } else if self.value < 100 {
            2
        } else {
            3
        };
        Rectangle::from_size((24 * digits, 35).into()).to_f64()
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        let digits = if self.value < 10 {
            1
        } else if self.value < 100 {
            2
        } else {
            3
        };
        Rectangle::from_size((24 * digits, 35).into()).to_physical_precise_round(scale)
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit_counter
    }
}

impl<R> RenderElement<R> for FpsElement<R::TextureId>
where
    R: Renderer + ImportAll,
    R::TextureId: 'static,
{
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        // FIXME: respect the src for cropping
        let scale = dst.size.to_f64() / self.src().size;
        let value_str = std::cmp::min(self.value, 999).to_string();
        let mut offset: Point<f64, Physical> = Point::from((0.0, 0.0));
        for digit in value_str.chars().map(|d| d.to_digit(10).unwrap()) {
            let digit_location = dst.loc.to_f64() + offset;
            let digit_size = Size::<i32, Logical>::from((22, 35))
                .to_f64()
                .to_physical(scale);
            let dst = Rectangle::new(
                digit_location.to_i32_round(),
                ((digit_size.to_point() + digit_location).to_i32_round()
                    - digit_location.to_i32_round())
                .to_size(),
            );
            let damage = damage
                .iter()
                .cloned()
                .flat_map(|x| x.intersection(dst))
                .map(|mut x| {
                    x.loc -= dst.loc;
                    x
                })
                .collect::<Vec<_>>();
            let texture_src: Rectangle<i32, Buffer> = match digit {
                9 => Rectangle::from_size((22, 35).into()),
                6 => Rectangle::new((22, 0).into(), (22, 35).into()),
                3 => Rectangle::new((44, 0).into(), (22, 35).into()),
                1 => Rectangle::new((66, 0).into(), (22, 35).into()),
                8 => Rectangle::new((0, 35).into(), (22, 35).into()),
                0 => Rectangle::new((22, 35).into(), (22, 35).into()),
                2 => Rectangle::new((44, 35).into(), (22, 35).into()),
                7 => Rectangle::new((0, 70).into(), (22, 35).into()),
                4 => Rectangle::new((22, 70).into(), (22, 35).into()),
                5 => Rectangle::new((44, 70).into(), (22, 35).into()),
                _ => unreachable!(),
            };

            frame.render_texture_from_to(
                &self.texture,
                texture_src.to_f64(),
                dst,
                &damage,
                &[],
                Transform::Normal,
                1.0,
            )?;
            offset += Point::from((24.0, 0.0)).to_physical(scale);
        }

        Ok(())
    }
}

use std::{
    cell::RefCell,
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Tracking frames-per-second.
#[derive(Clone, Debug)]
pub struct Fps {
    window_len: usize,
    inner: RefCell<Inner>,
}

#[derive(Clone, Debug)]
struct Inner {
    window: VecDeque<Duration>,
    last: Instant,
    avg: f64,
    min: f64,
    max: f64,
}

impl Fps {
    /// The window length used by the default constructor.
    pub const DEFAULT_WINDOW_LEN: usize = 60;

    /// Create a new `Fps` with the given window length as a number of frames.
    ///
    /// The larger the window, the "smoother" the FPS.
    pub fn with_window_len(window_len: usize) -> Self {
        let window = VecDeque::with_capacity(window_len);
        let last = Instant::now();
        let (avg, min, max) = (0.0, 0.0, 0.0);
        let inner = RefCell::new(Inner {
            window,
            last,
            avg,
            min,
            max,
        });
        Fps { window_len, inner }
    }

    /// Call this once per frame to allow the `Fps` instance to sample the rate internally.
    pub fn tick(&self) {
        let now = Instant::now();
        let mut inner = self.inner.borrow_mut();
        let delta = now.duration_since(inner.last);
        inner.last = now;
        while inner.window.len() + 1 > self.window_len {
            inner.window.pop_front();
        }
        inner.window.push_back(delta);
        inner.avg = inner.calc_avg();
        inner.min = inner.calc_min();
        inner.max = inner.calc_max();
    }

    /// Retrieve the average frames-per-second at the moment of the last call to `tick`.
    pub fn avg(&self) -> f64 {
        self.inner.borrow().avg
    }

    /// Retrieve the minimum frames-per-second that was reached within the window at the moment
    /// `tick` was last called.
    pub fn min(&self) -> f64 {
        self.inner.borrow().min
    }

    /// Retrieve the maximum frames-per-second that was reached within the window at the moment
    /// `tick` was last called.
    pub fn max(&self) -> f64 {
        self.inner.borrow().max
    }
}

impl Inner {
    /// Calculate the frames per second from the current state of the window.
    fn calc_avg(&self) -> f64 {
        let sum_secs = self.window.iter().map(|d| d.as_secs_f64()).sum::<f64>();
        1.0 / (sum_secs / self.window.len() as f64)
    }

    /// Find the minimum frames per second that occurs over the window.
    fn calc_min(&self) -> f64 {
        1.0 / self
            .window
            .iter()
            .max()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Find the minimum frames per second that occurs over the window.
    fn calc_max(&self) -> f64 {
        1.0 / self
            .window
            .iter()
            .min()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
}

impl Default for Fps {
    fn default() -> Self {
        Fps::with_window_len(Self::DEFAULT_WINDOW_LEN)
    }
}
